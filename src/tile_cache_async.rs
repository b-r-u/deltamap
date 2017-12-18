use image::DynamicImage;
use image;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::ops::Deref;
use std::sync::{Arc, Mutex, MutexGuard};
use tile::Tile;
use tile_loader::TileLoader;
use tile_source::TileSource;


pub struct TileCache {
    loader: TileLoader,
    map: Arc<Mutex<HashMap<Tile, image::DynamicImage>>>,
}

impl TileCache {
    pub fn new<F>(new_tile_func: F) -> Self
        where F: Fn(Tile) + Sync + Send + 'static,
    {
        let map = Arc::new(Mutex::new(HashMap::new()));
        TileCache {
            loader: TileLoader::new(TileSource::new(), move |tile| {
                println!("TILECACHE NEW tile {:?}", tile);
                new_tile_func(tile);
            }),
            map: map,
        }
    }

    pub fn get_sync(&mut self, tile: Tile, source: &TileSource) -> Option<ImgRef> {
        if let Ok(mut lock) = self.map.lock() {
            let contains = lock.contains_key(&tile);

            if contains {
                Some(ImgRef {
                    guard: lock,
                    key: tile,
                })
            } else {
                if let Some(img) = self.loader.get_sync(tile, source) {
                    lock.insert(tile, img);

                    Some(ImgRef {
                        guard: lock,
                        key: tile,
                    })
                } else {
                    None
                }
            }
        } else {
            None
        }
    }

    //TODO Return ImgRef, do not clone
    pub fn get_async(&mut self, tile: Tile, source: &TileSource) -> Option<&DynamicImage> {
        if let Ok(mut lock) = self.map.lock() {
            match lock.entry(tile) {
                Entry::Occupied(entry) => Some(entry.into_mut().clone()),
                Entry::Vacant(_) => {
                    self.loader.async_request(tile);
                    None
                }
            }
        } else {
            None
        }
    }

    /*
    //TODO Return ImgRef, do not clone
    pub fn get_sync(&mut self, tile: Tile, source: &TileSource) -> Option<ImgRef> {
        if let Ok(mut lock) = self.map.lock() {
            let contains = lock.contains_key(&tile);

            if contains {
                Some(ImgRef {
                    guard: lock,
                    key: tile,
                })
            } else {
                if let Some(img) = self.loader.get_sync(tile, source) {
                    lock.insert(tile, img);

                    Some(ImgRef {
                        guard: lock,
                        key: tile,
                    })
                } else {
                    None
                }
            }
        } else {
            None
        }
    }

    //TODO Return ImgRef, do not clone
    pub fn get_async(&mut self, tile: Tile, source: &TileSource) -> Option<DynamicImage> {
        if let Ok(mut lock) = self.map.lock() {
            match lock.entry(tile) {
                Entry::Occupied(entry) => Some(entry.into_mut().clone()),
                Entry::Vacant(_) => {
                    self.loader.async_request(tile);
                    None
                }
            }
        } else {
            None
        }
    }
    */
}

impl ::std::fmt::Debug for TileCache {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        if let Ok(lock) = self.map.try_lock() {
            write!(
                f,
                "TileCache {{ tiles: {:?} }}",
                lock.keys().collect::<Vec<_>>()
            )
        } else {
            write!(
                f,
                "TileCache {{ tiles: <not accessible> }}",
            )
        }
    }
}

pub struct ImgRef<'a> {
    guard: MutexGuard<'a, HashMap<Tile, DynamicImage>>,
    key: Tile,
}

impl<'a> Deref for ImgRef<'a> {
    type Target = DynamicImage;
    fn deref(&self) -> &Self::Target {
        println!("DEREF {:?}", self.key);
        self.guard.get(&self.key).unwrap()
    }
}
