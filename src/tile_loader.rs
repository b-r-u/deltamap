use coord::TileCoord;
use image::DynamicImage;
use image;
use reqwest::Client;
use std::collections::hash_set::HashSet;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use tile::Tile;
use tile_source::TileSource;


//TODO remember failed loading attempts

#[derive(Debug)]
pub struct TileLoader {
    client: Option<Client>,
    join_handle: thread::JoinHandle<()>,
    request_tx: mpsc::Sender<(Tile, String, PathBuf, bool)>,
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
        request_rx: mpsc::Receiver<(Tile, String, PathBuf, bool)>,
        result_tx: mpsc::Sender<(Tile, Option<DynamicImage>)>,
        notice_func: F,
    )
        where F: Fn(Tile) + Sync + Send + 'static,
    {
        let mut client_opt = None;
        while let Ok((tile, url, path, write_to_file)) = request_rx.recv() {
            println!("work {:?}", tile);
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
        }
    }

    pub fn async_request(&mut self, tile_coord: TileCoord, source: &TileSource, write_to_file: bool) {
        if tile_coord.zoom > source.max_tile_zoom() {
            return;
        }

        let tile = Tile::new(tile_coord, source.id());

        if !self.pending.contains(&tile) {
            self.pending.insert(tile);
            self.request_tx.send((
                tile,
                source.remote_tile_url(tile_coord),
                source.local_tile_path(tile_coord),
                write_to_file
            )).unwrap();
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
