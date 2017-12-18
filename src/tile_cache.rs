use image;
use linked_hash_map::{Entry, LinkedHashMap};
use tile::Tile;
use tile_loader::TileLoader;
use tile_source::TileSource;


pub struct TileCache {
    loader: TileLoader,
    map: LinkedHashMap<Tile, image::DynamicImage>,
    max_tiles: usize,
}

impl TileCache {
    pub fn new<F>(new_tile_func: F) -> Self
        where F: Fn(Tile) + Sync + Send + 'static,
    {
        TileCache {
            loader: TileLoader::new(move |tile| {
                new_tile_func(tile);
            }),
            map: LinkedHashMap::new(),
            max_tiles: 512, //TODO set a reasonable value
        }
    }

    pub fn get_sync(
        &mut self,
        tile: Tile,
        source: &TileSource,
        write_to_file: bool,
        ) -> Option<&image::DynamicImage>
    {
        //TODO Return the value from get_refresh with borrowck agreeing that this is OK.
        self.map.get_refresh(&tile);

        // remove old cache entries
        while self.map.len() + 1 > self.max_tiles {
            self.map.pop_front();
        }

        match self.map.entry(tile) {
            Entry::Occupied(entry) => {
                Some(entry.into_mut())
            },
            Entry::Vacant(entry) => {
                self.loader.get_sync(tile, source, write_to_file).map(|img| entry.insert(img) as &_)
            },
        }
    }

    pub fn get_async(
        &mut self,
        tile: Tile,
        source: &TileSource,
        write_to_file: bool,
        ) -> Option<&image::DynamicImage>
    {
        while let Some((t, img)) = self.loader.async_result() {
            // remove old cache entries
            while self.map.len() + 1 > self.max_tiles {
                self.map.pop_front();
            }

            self.map.insert(t, img);
            println!("CACHE SIZE: {} tiles", self.map.len());
        }

        //TODO Return the value from get_refresh with borrowck agreeing that this is OK.
        self.map.get_refresh(&tile);

        match self.map.entry(tile) {
            Entry::Occupied(entry) => Some(entry.into_mut()),
            Entry::Vacant(_) => {
                self.loader.async_request(tile, source, write_to_file);
                None
            }
        }
    }

    // Return a tile from the cache but do not use TileLoader.
    pub fn lookup(&mut self, tile: Tile, source: &TileSource) -> Option<&image::DynamicImage> {
        //TODO Return the value from get_refresh with borrowck agreeing that this is OK.
        self.map.get_refresh(&tile);

        self.map.get(&tile)
    }
}

impl ::std::fmt::Debug for TileCache {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(
            f,
            "TileCache {{ tiles: {:?} }}",
            self.map.keys().collect::<Vec<_>>()
        )
    }
}
