use coord::{TileCoord, View};
use image::DynamicImage;
use image;
use reqwest::Client;
use std::cmp::Ordering;
use std::cmp;
use std::collections::hash_set::HashSet;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::TryRecvError;
use std::sync::mpsc;
use std::thread;
use tile::Tile;
use tile_source::TileSource;


//TODO remember failed loading attempts

#[derive(Debug)]
pub struct TileLoader {
    client: Option<Client>,
    join_handle: thread::JoinHandle<()>,
    request_tx: mpsc::Sender<LoaderMessage>,
    result_rx: mpsc::Receiver<(Tile, Option<DynamicImage>)>,
    pending: HashSet<Tile>,
}

impl TileLoader {
    pub fn new<F>(notice_func: F) -> Self
        where F: Fn(Tile) + Sync + Send + 'static,
    {
        let (request_tx, request_rx) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();

        TileLoader {
            client: None,
            join_handle: thread::spawn(move || Self::work(request_rx, result_tx, notice_func)),
            request_tx: request_tx,
            result_rx: result_rx,
            pending: HashSet::new(),
        }
    }

    fn work<F>(
        request_rx: mpsc::Receiver<LoaderMessage>,
        result_tx: mpsc::Sender<(Tile, Option<DynamicImage>)>,
        notice_func: F,
    )
        where F: Fn(Tile) + Sync + Send + 'static,
    {
        let mut client_opt = None;
        let mut queue: Vec<(Tile, String, PathBuf, bool)> = vec![];

        'outer: while let Ok(message) = request_rx.recv() {
            let mut view_opt: Option<View> = None;

            match message {
                LoaderMessage::SetViewLocation{view} => {
                    view_opt = Some(view);
                },
                LoaderMessage::GetTile{tile, url, path, write_to_file} => {
                    queue.push((tile, url, path, write_to_file));
                }
            }

            loop {
                loop {
                    match request_rx.try_recv() {
                        Ok(LoaderMessage::SetViewLocation{view}) => {
                            view_opt = Some(view);
                        },
                        Ok(LoaderMessage::GetTile{tile, url, path, write_to_file}) => {
                            queue.push((tile, url, path, write_to_file));
                        },
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => break 'outer,
                    }
                }

                if let Some(view) = view_opt {
                    //TODO sort queue
                    queue.as_mut_slice().sort_by(|&(tile_a, _, _, _), &(tile_b, _, _, _)| {
                        compare_tiles(tile_a, tile_b, view)
                    });
                }

                match queue.pop() {
                    None => break,
                    Some((tile, url, path, write_to_file)) => {
                        println!("queue {:?} {:?}", tile, path);
                        match image::open(&path) {
                            Ok(img) => {
                                result_tx.send((tile, Some(img))).unwrap();
                                notice_func(tile);
                                continue;
                            },
                            Err(_) => {
                                //TODO do not try to create a client every time when it failed before
                                if client_opt.is_none() {
                                    client_opt = Client::builder().build().ok();
                                }

                                if let Some(ref client) = client_opt {
                                    println!("use client {:?}", tile);
                                    if let Ok(mut response) = client.get(&url).send() {
                                        let mut buf: Vec<u8> = vec![];
                                        response.copy_to(&mut buf).unwrap();
                                        if let Ok(img) = image::load_from_memory(&buf) {
                                            result_tx.send((tile, Some(img))).unwrap();
                                            notice_func(tile);

                                            if write_to_file {
                                                //TODO do something on write errors
                                                let _ = Self::write_to_file(&path, &buf);
                                            }

                                            continue;
                                        }
                                    }
                                }
                            },
                        }
                        result_tx.send((tile, None)).unwrap();
                    },
                }
            }
        }
    }

    pub fn async_request(&mut self, tile_coord: TileCoord, source: &TileSource, write_to_file: bool) {
        if tile_coord.zoom > source.max_tile_zoom() {
            return;
        }

        let tile = Tile::new(tile_coord, source.id());

        if !self.pending.contains(&tile) {
            self.pending.insert(tile);
            self.request_tx.send(LoaderMessage::GetTile{
                tile: tile,
                url: source.remote_tile_url(tile_coord),
                path: source.local_tile_path(tile_coord),
                write_to_file: write_to_file,
            }).unwrap();
        }
    }

    pub fn async_result(&mut self) -> Option<(Tile, DynamicImage)> {
        match self.result_rx.try_recv() {
            Err(_) => None,
            Ok((tile, None)) => {
                self.pending.remove(&tile);
                None
            },
            Ok((tile, Some(img))) => {
                self.pending.remove(&tile);
                Some((tile, img))
            },
        }
    }

    pub fn get_sync(&mut self, tile: TileCoord, source: &TileSource, write_to_file: bool) -> Option<DynamicImage> {
        match image::open(source.local_tile_path(tile)) {
            Ok(img) => {
                Some(img)
            },
            Err(_) => {
                //TODO do not try to create a client every time when it failed before
                if self.client.is_none() {
                    self.client = Client::builder().build().ok();
                }

                if let Some(ref client) = self.client {
                    println!("use client {:?}", tile);
                    if let Ok(mut response) = client.get(&source.remote_tile_url(tile)).send() {
                        let mut buf: Vec<u8> = vec![];
                        response.copy_to(&mut buf).unwrap();
                        if let Ok(img) = image::load_from_memory(&buf) {
                            if write_to_file {
                                let path = source.local_tile_path(tile);
                                let _ = Self::write_to_file(path, &buf);
                            }
                            Some(img)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            },
        }
    }

    pub fn set_view_location(&mut self, view: View) {
        self.request_tx.send(LoaderMessage::SetViewLocation{
            view: view,
        }).unwrap();
    }

    fn write_to_file<P: AsRef<Path>>(path: P, img_data: &[u8]) -> ::std::io::Result<()> {

        if let Some(dir) = path.as_ref().parent() {
            ::std::fs::create_dir_all(dir)?;
        }

        //TODO remove
        println!("write file {:?}", path.as_ref());

        let mut file = File::create(path)?;
        file.write_all(img_data)
    }
}

enum LoaderMessage {
    GetTile{tile: Tile, url: String, path: PathBuf, write_to_file: bool},
    SetViewLocation{view: View},
}

fn compare_tiles(a: Tile, b: Tile, view: View) -> Ordering {
    let source_a = view.source_id == a.source_id;
    let source_b = view.source_id == b.source_id;

    match (source_a, source_b) {
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        _ => {
            let zoom_diff_a = cmp::max(a.coord.zoom, view.zoom) - cmp::min(a.coord.zoom, view.zoom);
            let zoom_diff_b = cmp::max(b.coord.zoom, view.zoom) - cmp::min(b.coord.zoom, view.zoom);

            if zoom_diff_a < zoom_diff_b {
                Ordering::Greater
            } else if zoom_diff_a > zoom_diff_b {
                Ordering::Less
            } else {
                let map_a = a.coord.map_coord_center();
                let map_b = b.coord.map_coord_center();
                let center_diff_a = (view.center.x - map_a.x).hypot(view.center.y - map_a.y);
                let center_diff_b = (view.center.x - map_b.x).hypot(view.center.y - map_b.y);

                center_diff_b.partial_cmp(&center_diff_a).unwrap_or(Ordering::Equal)
            }
        },
    }
}
