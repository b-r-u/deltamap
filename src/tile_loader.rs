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
use std::sync::{Arc, mpsc, Mutex};
use std::sync::mpsc::TryRecvError;
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
        let mut queue: Vec<TileRequest> = vec![];
        let remote_queue: Arc<Mutex<Vec<TileRequest>>> = Arc::new(Mutex::new(vec![]));
        let mut view_opt: Option<View> = None;

        let arc_notice_func = Arc::new(notice_func);

        let (remote_request_tx, remote_request_rx) = mpsc::channel();
        {
            let arc_request_rx = Arc::new(Mutex::new(remote_request_rx));
            for id in 0..2 {
                let remote_queue = Arc::clone(&remote_queue);
                let arc_request_rx = Arc::clone(&arc_request_rx);
                let result_tx = result_tx.clone();
                let arc_notice_func = Arc::clone(&arc_notice_func);
                thread::spawn(move || Self::work_remote(id, &remote_queue, &arc_request_rx, &result_tx, &arc_notice_func));
            }
        }

        'outer: while let Ok(message) = request_rx.recv() {
            let mut need_to_sort = true;

            match message {
                LoaderMessage::SetView(view) => {
                    view_opt = Some(view);
                },
                LoaderMessage::GetTile(request) => {
                    queue.push(request);
                }
            }

            loop {
                loop {
                    let message = request_rx.try_recv();

                    match message {
                        Ok(LoaderMessage::SetView(view)) => {
                            view_opt = Some(view);
                            need_to_sort = true;
                        },
                        Ok(LoaderMessage::GetTile(request)) => {
                            queue.push(request);
                            need_to_sort = true;
                        },
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => break 'outer,
                    }
                }

                if need_to_sort {
                    if let Some(view) = view_opt {
                        need_to_sort = false;

                        queue.as_mut_slice().sort_by(|a, b| {
                            compare_tiles(a.tile, b.tile, view)
                        });

                        if let Ok(mut remote_queue) = remote_queue.lock() {
                            remote_queue.as_mut_slice().sort_by(|a, b| {
                                compare_tiles(a.tile, b.tile, view)
                            });
                        }
                    }
                }

                match queue.pop() {
                    None => break,
                    Some(request) => {
                        match image::open(&request.path) {
                            Ok(img) => {
                                if let Err(_) = result_tx.send((request.tile, Some(img))) {
                                    break 'outer;
                                }
                                arc_notice_func(request.tile);
                                continue;
                            },
                            Err(_) => {
                                if let Ok(mut remote_queue) = remote_queue.lock() {
                                    //TODO restrict size of remote_queue
                                    remote_queue.push(request);
                                    if let Some(view) = view_opt {
                                        remote_queue.as_mut_slice().sort_by(|a, b| {
                                            compare_tiles(a.tile, b.tile, view)
                                        });
                                    }
                                    if remote_request_tx.send(RemoteLoaderMessage::PopQueue).is_err() {
                                        //TODO remote worker terminated
                                    }
                                }
                            },
                        }
                    },
                }
            }
        }
    }

    fn work_remote<F>(
        thread_id: u32,
        queue: &Arc<Mutex<Vec<TileRequest>>>,
        request_rx: &Arc<Mutex<mpsc::Receiver<RemoteLoaderMessage>>>,
        result_tx: &mpsc::Sender<(Tile, Option<DynamicImage>)>,
        notice_func: &Arc<F>,
    )
        where F: Fn(Tile) + Sync + Send + 'static,
    {
        let mut client_opt = None;

        loop {
            let message = request_rx.lock().ok().and_then(|r| r.recv().ok());
            match message {
                None | Some(RemoteLoaderMessage::Terminate) => break,
                Some(RemoteLoaderMessage::PopQueue) => {
                    let ele: Option<TileRequest> = queue.lock().ok().and_then(|mut q| q.pop());

                    if let Some(request) = ele {
                        if client_opt.is_none() {
                            client_opt = Client::builder().build().ok();
                        }

                        if let Some(Ok(mut response)) = client_opt.as_ref().map(|c| c.get(&request.url).send()) {
                            let mut buf: Vec<u8> = vec![];
                            if response.copy_to(&mut buf).is_ok() {
                                if let Ok(img) = image::load_from_memory(&buf) {
                                    // successfully loaded tile

                                    if result_tx.send((request.tile, Some(img))).is_err() {
                                        break;
                                    }

                                    notice_func(request.tile);

                                    if request.write_to_file {
                                        //TODO do something on write errors
                                        let _ = Self::write_to_file(&request.path, &buf);
                                    }

                                    continue;
                                }
                            }
                        }

                        // failed not load tile
                        if result_tx.send((request.tile, None)).is_err() {
                            break;
                        }
                    }
                },
            }
        }
    }

    pub fn async_request(&mut self, tile_coord: TileCoord, source: &TileSource, write_to_file: bool) {
        if tile_coord.zoom > source.max_tile_zoom() {
            return;
        }

        let tile = Tile::new(tile_coord, source.id());

        if !self.pending.contains(&tile) {
            if self.request_tx.send(LoaderMessage::GetTile(
                    TileRequest {
                        tile: tile,
                        url: source.remote_tile_url(tile_coord),
                        path: source.local_tile_path(tile_coord),
                        write_to_file: write_to_file,
                    }
                )).is_ok()
            {
                self.pending.insert(tile);
            }
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
                    if let Ok(mut response) = client.get(&source.remote_tile_url(tile)).send() {
                        let mut buf: Vec<u8> = vec![];
                        if response.copy_to(&mut buf).is_ok() {
                            if let Ok(img) = image::load_from_memory(&buf) {
                                if write_to_file {
                                    let path = source.local_tile_path(tile);
                                    let _ = Self::write_to_file(path, &buf);
                                }
                                return Some(img);
                            }
                        }
                    }
                }
                None
            },
        }
    }

    pub fn set_view_location(&mut self, view: View) {
        let _ = self.request_tx.send(LoaderMessage::SetView(view));
    }

    fn write_to_file<P: AsRef<Path>>(path: P, img_data: &[u8]) -> ::std::io::Result<()> {
        if let Some(dir) = path.as_ref().parent() {
            ::std::fs::create_dir_all(dir)?;
        }

        let mut file = File::create(path)?;
        file.write_all(img_data)
    }
}

#[derive(Debug)]
struct TileRequest {
    pub tile: Tile,
    pub url: String,
    pub path: PathBuf,
    pub write_to_file: bool,
}

#[derive(Debug)]
enum LoaderMessage {
    GetTile(TileRequest),
    SetView(View),
}

#[derive(Debug)]
enum RemoteLoaderMessage {
    PopQueue,
    Terminate,
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
