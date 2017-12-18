use coord::MapCoord;


#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Tile {
    pub zoom: u32,
    pub tile_x: i32,
    pub tile_y: i32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SubTileCoord {
    pub size: u32,
    pub x: u32,
    pub y: u32,
}

impl Tile {
    pub fn new(zoom: u32, tile_x: i32, tile_y: i32) -> Tile {
        Tile {
            zoom: zoom,
            tile_x: Self::normalize_coord(tile_x, zoom),
            tile_y: tile_y,
        }
    }

    pub fn is_on_planet(&self) -> bool {
        let num_tiles = Self::get_zoom_level_tiles(self.zoom);
        self.tile_y >= 0 && self.tile_y < num_tiles &&
        self.tile_x >= 0 && self.tile_x < num_tiles
    }

    pub fn map_coord(&self) -> MapCoord {
        let inv_zoom_factor = f64::powi(2.0, -(self.zoom as i32));
        MapCoord::new(f64::from(self.tile_x) * inv_zoom_factor, f64::from(self.tile_y) * inv_zoom_factor)
    }

    pub fn parent(&self, distance: u32) -> Option<(Tile, SubTileCoord)> {
        if distance > self.zoom {
            None
        } else {
            let scale = u32::pow(2, distance);

            Some((
                Tile {
                    zoom: self.zoom - distance,
                    tile_x: self.tile_x / scale as i32,
                    tile_y: self.tile_y / scale as i32,
                },
                SubTileCoord {
                    size: scale,
                    x: (Self::normalize_coord(self.tile_x, self.zoom) as u32) % scale,
                    y: (Self::normalize_coord(self.tile_y, self.zoom) as u32) % scale,
                },
            ))
        }
    }

    #[inline]
    fn normalize_coord(coord: i32, zoom: u32) -> i32 {
        let max = Self::get_zoom_level_tiles(zoom);
        ((coord % max) + max) % max
    }

    #[inline]
    pub fn get_zoom_level_tiles(zoom: u32) -> i32 {
        //TODO throw error when zoom too big
        i32::pow(2, zoom)
    }
}
