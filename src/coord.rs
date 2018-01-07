use std::f64::consts::PI;
use tile_source::TileSourceId;

/// A position in map coordinates.
/// Valid values for x and y lie in the interval [0.0, 1.0].
#[derive(Copy, Debug, PartialEq, Clone)]
pub struct MapCoord {
    pub x: f64,
    pub y: f64,
}

impl MapCoord {
    pub fn new(x: f64, y: f64) -> MapCoord {
        MapCoord {
            x: x,
            y: y,
        }
    }

    pub fn from_latlon(latitude: f64, longitude: f64) -> MapCoord {
        let x = longitude * (1.0 / 360.0) + 0.5;
        let pi_lat = latitude * (PI / 180.0);
        let y = f64::ln(f64::tan(pi_lat) + 1.0 / f64::cos(pi_lat)) * (-0.5 / PI) + 0.5;

        MapCoord {
            x: x,
            y: y,
        }
    }

    //TODO differ between normalized and not normalized tiles
    pub fn on_tile_at_zoom(&self, zoom: u32) -> TileCoord {
        let zoom_factor = f64::powi(2.0, zoom as i32);
        let ix = (self.x * zoom_factor).floor() as i32;
        let iy = (self.y * zoom_factor).floor() as i32;

        let x = ix;
        let y = iy;

        TileCoord {
            zoom: zoom,
            x: x,
            y: y,
        }
    }

    pub fn normalize_x(&mut self) {
        // Wrap around in x-direction.
        // Do not wrap around in y-direction. The poles don't touch.
        self.x = (self.x.fract() + 1.0).fract();
    }

    pub fn normalize_xy(&mut self) {
        // Wrap around in x-direction.
        // Restrict y coordinates to interval [0.0, 1.0]
        self.x = (self.x.fract() + 1.0).fract();
        self.y = 0.0f64.max(1.0f64.min(self.y));
    }

}

/// A position on the screen in pixels. Top-left corner is (0.0, 0.0).
#[derive(Copy, Clone, Debug)]
pub struct ScreenCoord {
    pub x: f64,
    pub y: f64,
}

impl ScreenCoord {
    pub fn new(x: f64, y: f64) -> Self {
        ScreenCoord {
            x: x,
            y: y,
        }
    }
    pub fn snap_to_pixel(&mut self) {
        self.x = self.x.floor();
        self.y = self.y.floor();
    }
}

/// A rectangle in screen coordinates.
#[derive(Copy, Clone, Debug)]
pub struct ScreenRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl ScreenRect {
    pub fn subdivide(&self, sub_tile: &SubTileCoord) -> ScreenRect {
        let scale = 1.0 / f64::from(sub_tile.size);
        let w = self.width * scale;
        let h = self.height * scale;
        ScreenRect {
            x: self.x + f64::from(sub_tile.x) * w,
            y: self.y + f64::from(sub_tile.y) * h,
            width: w,
            height: h,
        }
    }
}

/// A rectangle in texture coordinates.
/// Top-left corner is (0.0, 0.0).
/// Bottom-right corner is (1.0, 1.0).
#[derive(Copy, Clone, Debug)]
pub struct TextureRect {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

impl TextureRect {
    pub fn inset(self, margin_x: f64, margin_y: f64) -> TextureRect {
        TextureRect {
            x1: self.x1 + margin_x,
            y1: self.y1 + margin_y,
            x2: self.x2 - margin_x,
            y2: self.y2 - margin_y,
        }
    }

    pub fn subdivide(&self, sub_tile: &SubTileCoord) -> TextureRect {
        let scale = 1.0 / f64::from(sub_tile.size);
        let w = (self.x2 - self.x1) * scale;
        let h = (self.y2 - self.y1) * scale;
        TextureRect {
            x1: self.x1 + f64::from(sub_tile.x) * w,
            y1: self.y1 + f64::from(sub_tile.y) * h,
            x2: self.x1 + f64::from(sub_tile.x + 1) * w,
            y2: self.y1 + f64::from(sub_tile.y + 1) * h,
        }
    }
}

/// A subdivision of a tile. `x` and `y` are in the interval [0, `size` - 1].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SubTileCoord {
    pub size: u32,
    pub x: u32,
    pub y: u32,
}

impl SubTileCoord {
    pub fn subdivide(&self, other: &SubTileCoord) -> SubTileCoord {
        SubTileCoord {
            x: self.x * other.size + other.x,
            y: self.y * other.size + other.y,
            size: self.size * other.size,
        }
    }
}

/// A tile position in a tile pyramid.
/// Each zoom level has 2^zoom by 2^zoom tiles.
/// `x` and `y` are allowed to be negative or >= 2^zoom but then they will not correspond to a tile
/// and `is_on_planet` will return false.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TileCoord {
    pub zoom: u32,
    pub x: i32,
    pub y: i32,
}

impl TileCoord {
    pub fn new(zoom: u32, x: i32, y: i32) -> TileCoord {
        TileCoord {
            zoom: zoom,
            x: Self::normalize_coord(x, zoom),
            y: y,
        }
    }

    pub fn is_on_planet(&self) -> bool {
        let num_tiles = Self::get_zoom_level_tiles(self.zoom);
        self.y >= 0 && self.y < num_tiles &&
        self.x >= 0 && self.x < num_tiles
    }

    // Return the MapCoord of the top left corner of the current tile.
    pub fn map_coord_north_west(&self) -> MapCoord {
        let inv_zoom_factor = f64::powi(2.0, -(self.zoom as i32));
        MapCoord::new(f64::from(self.x) * inv_zoom_factor, f64::from(self.y) * inv_zoom_factor)
    }

    // Return the MapCoord of the center of the current tile.
    pub fn map_coord_center(&self) -> MapCoord {
        let inv_zoom_factor = f64::powi(2.0, -(self.zoom as i32));
        MapCoord::new(
            (f64::from(self.x) + 0.5) * inv_zoom_factor,
            (f64::from(self.y) + 0.5) * inv_zoom_factor,
        )
    }

    pub fn parent(&self, distance: u32) -> Option<(TileCoord, SubTileCoord)> {
        if distance > self.zoom {
            None
        } else {
            let scale = u32::pow(2, distance);

            Some((
                TileCoord {
                    zoom: self.zoom - distance,
                    x: self.x / scale as i32,
                    y: self.y / scale as i32,
                },
                SubTileCoord {
                    size: scale,
                    x: (Self::normalize_coord(self.x, self.zoom) as u32) % scale,
                    y: (Self::normalize_coord(self.y, self.zoom) as u32) % scale,
                },
            ))
        }
    }

    pub fn children(&self) -> [(TileCoord, SubTileCoord); 4] {
        [
            (
                TileCoord {
                    zoom: self.zoom + 1,
                    x: self.x * 2,
                    y: self.y * 2,
                },
                SubTileCoord {
                    size: 2,
                    x: 0,
                    y: 0,
                },
            ),
            (
                TileCoord {
                    zoom: self.zoom + 1,
                    x: self.x * 2 + 1,
                    y: self.y * 2,
                },
                SubTileCoord {
                    size: 2,
                    x: 1,
                    y: 0,
                },
            ),
            (
                TileCoord {
                    zoom: self.zoom + 1,
                    x: self.x * 2,
                    y: self.y * 2 + 1,
                },
                SubTileCoord {
                    size: 2,
                    x: 0,
                    y: 1,
                },
            ),
            (
                TileCoord {
                    zoom: self.zoom + 1,
                    x: self.x * 2 + 1,
                    y: self.y * 2 + 1,
                },
                SubTileCoord {
                    size: 2,
                    x: 1,
                    y: 1,
                },
            ),
        ]
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

    pub fn to_quadkey(&self) -> Option<String> {
        if self.zoom == 0 || self.zoom > 30 || self.x < 0 || self.y < 0 {
            return None;
        }

        let mut quadkey = String::with_capacity(self.zoom as usize);

        let len = self.zoom;

        for i in (0..len).rev() {
            let mask: u32 = 1 << i;

            match ((self.x as u32 & mask) != 0, (self.y as u32 & mask) != 0) {
                (false, false) => quadkey.push('0'),
                (true, false) => quadkey.push('1'),
                (false, true) => quadkey.push('2'),
                (true, true) => quadkey.push('3'),
            }
        }

        Some(quadkey)
    }
}

//TODO include width and height of view rect to determine visibility
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct View {
    pub source_id: TileSourceId,
    pub zoom: u32,
    pub center: MapCoord,
}

#[cfg(test)]
mod tests {
    use coord::*;

    #[test]
    fn normalize_mapcoord() {
        {
            let a = MapCoord::new(0.0, 0.0);
            let mut b = a.clone();
            assert_eq!(a, b);
            b.normalize_x();
            assert_eq!(a, b);
        }
        {
            let mut a = MapCoord::new(1.0, 1.0);
            let b = MapCoord::new(0.0, 1.0);
            a.normalize_x();
            assert_eq!(a, b);
        }
    }

    #[test]
    fn quadkey() {
        assert_eq!(TileCoord::new(0, 0, 0).to_quadkey(), None);
        assert_eq!(TileCoord::new(1, 0, 0).to_quadkey(), Some("0".to_string()));
        assert_eq!(TileCoord::new(1, 1, 0).to_quadkey(), Some("1".to_string()));
        assert_eq!(TileCoord::new(1, 0, 1).to_quadkey(), Some("2".to_string()));
        assert_eq!(TileCoord::new(1, 1, 1).to_quadkey(), Some("3".to_string()));
        assert_eq!(TileCoord::new(3, 1, 0).to_quadkey(), Some("001".to_string()));
        assert_eq!(TileCoord::new(30, 0, 1).to_quadkey(), Some("000000000000000000000000000002".to_string()));
    }
}
