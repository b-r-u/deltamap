use std::f64::consts::PI;
use tile::Tile;

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
    pub fn on_tile_at_zoom(&self, zoom: u32) -> Tile {
        let zoom_factor = f64::powi(2.0, zoom as i32);
        let ix = (self.x * zoom_factor).floor() as i32;
        let iy = (self.y * zoom_factor).floor() as i32;

        let x = ix;
        let y = iy;

        Tile {
            zoom: zoom,
            tile_x: x,
            tile_y: y,
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
