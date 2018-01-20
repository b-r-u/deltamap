use image;
use linked_hash_map::{Entry, LinkedHashMap};
use coord::{TileCoord, View};
use tile::Tile;
use tile_loader::TileLoader;
use tile_source::TileSource;


pub struct TileCache {
    loader: TileLoader,
    map: LinkedHashMap<Tile, image::DynamicImage>,
    max_tiles: usize,
}

impl TileCache {
    pub fn new<F>(new_tile_func: F, use_network: bool) -> Self
        where F: Fn(Tile) + Sync + Send + 'static,
    {
        TileCache {
            loader: TileLoader::new(
                move |tile| {
                    new_tile_func(tile);
                },
                use_network,
            ),
            map: LinkedHashMap::new(),
            max_tiles: 512, //TODO set a reasonable value
        }
    }

    // Return the maximum number of tiles that this cache can hold at once.
    pub fn max_tiles(&self) -> usize {
        self.max_tiles
    }

    pub fn get_sync(
        &mut self,
        tile_coord: TileCoord,
        source: &TileSource,
        write_to_file: bool,
        ) -> Option<&image::DynamicImage>
    {
        let tile = Tile::new(tile_coord, source.id());

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
                self.loader.get_sync(tile_coord, source, write_to_file).map(|img| entry.insert(img) as &_)
            },
        }
    }

    pub fn get_async(
        &mut self,
        tile_coord: TileCoord,
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
        }

        let tile = Tile::new(tile_coord, source.id());

        //TODO Return the value from get_refresh with borrowck agreeing that this is OK.
        self.map.get_refresh(&tile);

        match self.map.entry(tile) {
            Entry::Occupied(entry) => Some(entry.into_mut()),
            Entry::Vacant(_) => {
                self.loader.async_request(tile_coord, source, write_to_file);
                None
            }
        }
    }

    // Return a tile from the cache but do not use TileLoader.
    pub fn lookup(&mut self, tile: Tile) -> Option<&image::DynamicImage> {
        //TODO Return the value from get_refresh with borrowck agreeing that this is OK.
        self.map.get_refresh(&tile);

        self.map.get(&tile)
    }

    pub fn set_view_location(&mut self, view: View) {
        self.loader.set_view_location(view);
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
