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

#[test]
fn test_normalize() {
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

#[derive(Copy, Clone, Debug)]
pub struct ScreenRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TileCoord {
    pub zoom: u32,
    pub x: i32,
    pub y: i32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SubTileCoord {
    pub size: u32,
    pub x: u32,
    pub y: u32,
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

//TODO include width and height of view rect to determine visibility
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct View {
    pub source_id: TileSourceId,
    pub zoom: u32,
    pub center: MapCoord,
}
