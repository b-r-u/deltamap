use std::f64::consts::{PI, FRAC_1_PI};
use tile_source::TileSourceId;
use cgmath::{Point3};


/// A position in latitude, longitude.
/// Values are in degrees and usually in these intervals:
/// latitude: [-90.0, 90.0]
/// longitude: [-180, 180.0]
#[derive(Copy, Debug, PartialEq, Clone)]
pub struct LatLonDeg {
    pub lat: f64,
    pub lon: f64,
}

impl LatLonDeg {
    pub fn new(lat: f64, lon: f64) -> Self {
        LatLonDeg { lat, lon }
    }

    pub fn to_radians(&self) -> LatLonRad {
        let f = PI / 180.0;
        LatLonRad {
            lat: self.lat * f,
            lon: self.lon * f,
        }
    }
}

/// A position in latitude, longitude.
/// Values are in radians and usually in these intervals:
/// latitude: [-0.5 * π, 0.5 * π]
/// longitude: [-π, π]
#[derive(Copy, Debug, PartialEq, Clone)]
pub struct LatLonRad {
    pub lat: f64,
    pub lon: f64,
}

impl LatLonRad {
    pub fn new(lat: f64, lon: f64) -> Self {
        LatLonRad { lat, lon }
    }

    pub fn to_degrees(&self) -> LatLonDeg {
        let f = 180.0 * FRAC_1_PI;
        LatLonDeg {
            lat: self.lat * f,
            lon: self.lon * f,
        }
    }

    /// Convert to 3D Point on unit sphere.
    pub fn to_sphere_xyz(&self) -> SphereXYZ {
        SphereXYZ {
            x: self.lat.cos() * self.lon.cos(),
            y: self.lat.sin(),
            z: self.lat.cos() * self.lon.sin(),
        }
    }

    /// Convert to 3D Point on unit sphere.
    pub fn to_sphere_point3(&self) -> Point3<f64> {
        let p = self.to_sphere_xyz();
        Point3::new(p.x, p.y, p.z)
    }
}

#[derive(Copy, Debug, PartialEq, Clone)]
pub struct SphereXYZ {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl SphereXYZ {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        SphereXYZ { x, y, z }
    }
}

/// A position in map coordinates.
/// Valid values for x and y lie in the interval [0.0, 1.0].
#[derive(Copy, Debug, PartialEq, Clone)]
pub struct MapCoord {
    pub x: f64,
    pub y: f64,
}

impl From<LatLonDeg> for MapCoord
{
    fn from(pos: LatLonDeg) -> MapCoord {
        let x = pos.lon * (1.0 / 360.0) + 0.5;
        let pi_lat = pos.lat * (PI / 180.0);
        let y = f64::ln(f64::tan(pi_lat) + 1.0 / f64::cos(pi_lat)) * (-0.5 * FRAC_1_PI) + 0.5;
        debug_assert!(y.is_finite());

        MapCoord { x, y }
    }
}

impl From<LatLonRad> for MapCoord
{
    fn from(pos: LatLonRad) -> MapCoord {
        let x = pos.lon * (0.5 * FRAC_1_PI) + 0.5;
        let y = f64::ln(f64::tan(pos.lat) + 1.0 / f64::cos(pos.lat)) * (-0.5 * FRAC_1_PI) + 0.5;
        debug_assert!(y.is_finite());

        MapCoord { x, y }
    }
}

impl MapCoord {
    pub fn new(x: f64, y: f64) -> MapCoord {
        MapCoord { x, y }
    }

    //TODO differ between normalized and not normalized tiles
    pub fn on_tile_at_zoom(&self, zoom: u32) -> TileCoord {
        let zoom_factor = f64::powi(2.0, zoom as i32);
        let x = (self.x * zoom_factor).floor() as i32;
        let y = (self.y * zoom_factor).floor() as i32;

        TileCoord { zoom, x, y }
    }

    /// Wrap around in x-direction.
    /// Do not wrap around in y-direction. The poles don't touch.
    pub fn normalize_x(&mut self) {
        self.x = (self.x.fract() + 1.0).fract();
    }

    /// Wrap around in x-direction.
    /// Restrict y coordinates to interval [0.0, 1.0]
    pub fn normalize_xy(&mut self) {
        self.x = (self.x.fract() + 1.0).fract();
        self.y = 0.0f64.max(1.0f64.min(self.y));
    }

    pub fn to_latlon_rad(&self) -> LatLonRad {
        LatLonRad {
            lat: (PI - self.y * (2.0 * PI)).sinh().atan(),
            lon: self.x * (2.0 * PI) - PI,
        }
    }

    pub fn to_latlon_deg(&self) -> LatLonDeg {
        LatLonDeg {
            lat: (PI - self.y * (2.0 * PI)).sinh().atan() * (180.0 * FRAC_1_PI),
            lon: self.x * 360.0 - 180.0,
        }
    }
}

/// A position on the screen in physical pixels. Top-left corner is (0.0, 0.0).
#[derive(Copy, Clone, Debug)]
pub struct ScreenCoord {
    pub x: f64,
    pub y: f64,
}

impl ScreenCoord {
    pub fn new(x: f64, y: f64) -> Self {
        ScreenCoord { x, y }
    }

    pub fn snap_to_pixel(&mut self) {
        self.x = self.x.floor();
        self.y = self.y.floor();
    }

    pub fn is_inside(&self, rect: &ScreenRect) -> bool {
        self.x >= rect.x &&
        self.y >= rect.y &&
        self.x < rect.x + rect.width &&
        self.y < rect.y + rect.height
    }
}

/// A rectangle in screen coordinates. Position and size are in physical pixels.
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
/// Each zoom level has 2<sup>zoom</sup> by 2<sup>zoom</sup> tiles.
/// `x` and `y` are allowed to be negative or >= 2<sup>zoom</sup> but then they will not correspond to a tile
/// and `is_valid` will return false.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TileCoord {
    pub zoom: u32,
    pub x: i32,
    pub y: i32,
}

impl TileCoord {
    pub fn new(zoom: u32, x: i32, y: i32) -> TileCoord {
        TileCoord {
            zoom,
            x: Self::normalize_coord(x, zoom),
            y,
        }
    }

    pub fn is_valid(&self) -> bool {
        let num_tiles = Self::get_zoom_level_tiles(self.zoom);
        self.y >= 0 && self.y < num_tiles &&
        self.x >= 0 && self.x < num_tiles
    }

    /// Returns the nearest valid `TileCoord`. This wraps the x coordinate and clamps the y
    /// coordinate.
    pub fn nearest_valid(&self) -> Self {
        let num_tiles = Self::get_zoom_level_tiles(self.zoom);
        TileCoord {
            zoom: self.zoom,
            x: ((self.x % num_tiles) + num_tiles) % num_tiles,
            y: self.y.min(num_tiles - 1).max(0),
        }
    }

    // Return the MapCoord of the top left corner of the current tile.
    pub fn map_coord_north_west(&self) -> MapCoord {
        let inv_zoom_factor = f64::powi(2.0, -(self.zoom as i32));
        MapCoord::new(f64::from(self.x) * inv_zoom_factor, f64::from(self.y) * inv_zoom_factor)
    }

    // Return the LatLonRad coordinate of the top left corner of the current tile.
    pub fn latlon_rad_north_west(&self) -> LatLonRad {
        let factor = f64::powi(2.0, -(self.zoom as i32)) * (2.0 * PI);

        LatLonRad::new(
            (PI - f64::from(self.y) * factor).sinh().atan(),
            f64::from(self.x) * factor - PI,
        )
    }

    // Return the LatLonRad coordinate of the bottom right corner of the current tile.
    pub fn latlon_rad_south_east(&self) -> LatLonRad {
        TileCoord { zoom: self.zoom, x: self.x + 1, y: self.y + 1 }.latlon_rad_north_west()
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

    pub fn children_iter(&self, zoom_delta: u32) -> TileChildrenIter {
        let zoom_level_tiles = Self::get_zoom_level_tiles(zoom_delta);
        TileChildrenIter {
            zoom: self.zoom + zoom_delta,
            tile_base_x: self.x * zoom_level_tiles,
            tile_base_y: self.y * zoom_level_tiles,
            child_x: -1,
            child_y: 0,
            zoom_level_tiles,
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

pub struct TileChildrenIter {
    zoom: u32,
    tile_base_x: i32,
    tile_base_y: i32,
    child_x: i32,
    child_y: i32,
    zoom_level_tiles: i32,
}

impl Iterator for TileChildrenIter {
    type Item = (TileCoord, SubTileCoord);

    fn next(&mut self) -> Option<Self::Item> {
        self.child_x += 1;

        if self.child_x >= self.zoom_level_tiles {
            self.child_x = 0;
            self.child_y += 1;
        }
        if self.child_y >= self.zoom_level_tiles {
            return None;
        }

        Some((
            TileCoord {
                zoom: self.zoom,
                x: self.tile_base_x + self.child_x,
                y: self.tile_base_y + self.child_y,
            },
            SubTileCoord {
                size: self.zoom_level_tiles as u32,
                x: self.child_x as u32,
                y: self.child_y as u32,
            },
        ))
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

    fn tc(zoom: u32, x: i32, y: i32) -> TileCoord {
        TileCoord { zoom, x, y }
    }

    #[test]
    fn nearest_valid() {
        assert_eq!(tc(0, 0, 0).nearest_valid(), tc(0, 0, 0));
        assert_eq!(tc(0, -1, 0).nearest_valid(), tc(0, 0, 0));
        assert_eq!(tc(0, 1, 0).nearest_valid(), tc(0, 0, 0));
        assert_eq!(tc(0, 0, -1).nearest_valid(), tc(0, 0, 0));
        assert_eq!(tc(0, 0, 1).nearest_valid(), tc(0, 0, 0));

        assert_eq!(tc(1, 0, 0).nearest_valid(), tc(1, 0, 0));
        assert_eq!(tc(1, -1, 0).nearest_valid(), tc(1, 1, 0));
        assert_eq!(tc(1, 1, 0).nearest_valid(), tc(1, 1, 0));
        assert_eq!(tc(1, 0, -1).nearest_valid(), tc(1, 0, 0));
        assert_eq!(tc(1, 0, 1).nearest_valid(), tc(1, 0, 1));
        assert_eq!(tc(1, 0, 2).nearest_valid(), tc(1, 0, 1));

        assert_eq!(tc(2, 0, 0).nearest_valid(), tc(2, 0, 0));
        assert_eq!(tc(2, 4, 0).nearest_valid(), tc(2, 0, 0));
        assert_eq!(tc(2, 5, 0).nearest_valid(), tc(2, 1, 0));
        assert_eq!(tc(2, -1, 0).nearest_valid(), tc(2, 3, 0));
        assert_eq!(tc(2, 0, 4).nearest_valid(), tc(2, 0, 3));
        assert_eq!(tc(2, 0, -1).nearest_valid(), tc(2, 0, 0));
    }

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-10
    }

    #[test]
    fn approx_eq_test() {
        assert!(approx_eq(1.0, 1.0));
        assert!(approx_eq(0.0, 0.0));
        assert!(approx_eq(0.0, -0.0));
        assert!(approx_eq(1e20, 1e20 + 1.0));
        assert!(approx_eq(1e20, 1e20 - 1.0));
        assert!(!approx_eq(1000.0, 1000.1));
    }

    #[test]
    fn degree_radians() {
        {
            let rad = LatLonDeg::new(0.0, 0.0).to_radians();
            assert!(approx_eq(rad.lat, 0.0));
            assert!(approx_eq(rad.lon, 0.0));
            let deg = rad.to_degrees();
            assert!(approx_eq(deg.lat, 0.0));
            assert!(approx_eq(deg.lon, 0.0));
        }
        {
            let rad = LatLonDeg::new(-45.0, 180.0).to_radians();
            assert!(approx_eq(rad.lat, -PI / 4.0));
            assert!(approx_eq(rad.lon, PI));
            let deg = rad.to_degrees();
            assert!(approx_eq(deg.lat, -45.0));
            assert!(approx_eq(deg.lon, 180.0));
        }

        {
            let mc = MapCoord::from(LatLonDeg::new(23.45, 123.45));
            let deg = mc.to_latlon_rad().to_degrees();
            assert!(approx_eq(deg.lat, 23.45));
            assert!(approx_eq(deg.lon, 123.45));
        }

        {
            let mc = MapCoord::from(LatLonRad::new(-0.345 * PI, -0.987 * PI));
            let rad = mc.to_latlon_deg().to_radians();
            assert!(approx_eq(rad.lat, -0.345 * PI));
            assert!(approx_eq(rad.lon, -0.987 * PI));
        }
    }

    #[test]
    fn tile_children() {
        let t = TileCoord::new(2, 1, 2);
        for (&(a1, a2), (b1, b2)) in t.children().iter().zip(t.children_iter(1)) {
            assert_eq!(a1, b1);
            assert_eq!(a2, b2);
        }
        assert_eq!(t.children_iter(0).count(), 1);
        assert_eq!(t.children_iter(0).next(), Some((t, SubTileCoord{ size: 1, x: 0, y: 0 })));
        assert_eq!(t.children_iter(1).count(), 4);
        assert_eq!(t.children_iter(2).count(), 16);
    }
}
