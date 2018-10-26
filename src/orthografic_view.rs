use cgmath::{Matrix3, Point3, Transform, vec3, Vector2};
use coord::{LatLonRad, MapCoord, ScreenCoord, TextureRect, TileCoord};
use mercator_view::MercatorView;
use projection::Projection;
use std::collections::HashSet;
use std::f64::consts::{PI, FRAC_1_PI};
use std::f64;
use toml::Value;
use toml::value::Table;


pub const MIN_ZOOM_LEVEL: f64 = 0.0;
pub const MAX_ZOOM_LEVEL: f64 = 22.0;

#[derive(Clone, Debug)]
pub struct VisibleTile {
    pub tile: TileCoord,
}

impl From<TileCoord> for VisibleTile {
    fn from(tc: TileCoord) -> Self {
        VisibleTile {
            tile: tc,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TexturedVisibleTile {
    pub tile_coord: TileCoord,
    pub tex_rect: TextureRect,
    pub tex_minmax: TextureRect,
}


#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum TileNeighbor {
    Coord(TileCoord),
    NorthPole,
    SouthPole,
}

/// Tile neighbors using sphere topology
pub fn tile_neighbors(origin: TileCoord, result: &mut Vec<TileNeighbor>) {
    result.clear();

    let zoom_level_tiles = TileCoord::get_zoom_level_tiles(origin.zoom);

    if origin.y < 0 || origin.y >= zoom_level_tiles {
        // Tile is out of bounds
        return;
    }

    // Normalize x coordinate
    let origin = TileCoord {
        zoom: origin.zoom,
        x: ((origin.x % zoom_level_tiles) + zoom_level_tiles) % zoom_level_tiles,
        y: origin.y,
    };

    match (origin.zoom, origin.y) {
        (0, _) => {},
        (1, _) => {
            result.extend(&[
                TileNeighbor::Coord(TileCoord::new(
                    origin.zoom,
                    (origin.x + 1) % zoom_level_tiles,
                    origin.y)
                ),
                TileNeighbor::Coord(TileCoord::new(
                    origin.zoom,
                    origin.x,
                    (origin.y + 1) % zoom_level_tiles),
                ),
            ]);
        },
        (_, 0) => {
            result.extend(&[
                TileNeighbor::NorthPole,
                TileNeighbor::Coord(TileCoord::new(
                    origin.zoom,
                    origin.x,
                    origin.y + 1,
                )),
                TileNeighbor::Coord(TileCoord::new(
                    origin.zoom,
                    (origin.x + 1) % zoom_level_tiles,
                    origin.y,
                )),
                TileNeighbor::Coord(TileCoord::new(
                    origin.zoom,
                    (origin.x + zoom_level_tiles - 1) % zoom_level_tiles,
                    origin.y,
                )),
            ]);
        },
        (_, y) if y == zoom_level_tiles - 1 => {
            result.extend(&[
                TileNeighbor::SouthPole,
                TileNeighbor::Coord(TileCoord::new(
                    origin.zoom,
                    origin.x,
                    origin.y - 1,
                )),
                TileNeighbor::Coord(TileCoord::new(
                    origin.zoom,
                    (origin.x + 1) % zoom_level_tiles,
                    origin.y,
                )),
                TileNeighbor::Coord(TileCoord::new(
                    origin.zoom,
                    (origin.x + zoom_level_tiles - 1) % zoom_level_tiles,
                    origin.y,
                )),
            ]);
        },
        _ => {
            result.extend(&[
                TileNeighbor::Coord(TileCoord::new(
                    origin.zoom,
                    origin.x,
                    origin.y + 1,
                )),
                TileNeighbor::Coord(TileCoord::new(
                    origin.zoom,
                    origin.x,
                    origin.y - 1,
                )),
                TileNeighbor::Coord(TileCoord::new(
                    origin.zoom,
                    (origin.x + 1) % zoom_level_tiles,
                    origin.y,
                )),
                TileNeighbor::Coord(TileCoord::new(
                    origin.zoom,
                    (origin.x + zoom_level_tiles - 1) % zoom_level_tiles,
                    origin.y,
                )),
            ]);
        },
    }
}


/// Orthographic projection, WGS 84 coordinates mapped to the sphere
#[derive(Clone, Debug)]
pub struct OrthograficView {
    /// Size of the viewport in physical pixels.
    pub viewport_size: Vector2<f64>,
    /// Size of each square tile in the same unit as the viewport dimensions.
    pub tile_size: u32,
    /// The `MapCoord` that corresponds to the center of the viewport.
    pub center: MapCoord,
    /// The zoom value. The zoom factor is given by 2.0.powf(zoom);
    pub zoom: f64,
    /// Tiles only exist for integer zoom values. The tile zoom value that is used for rendering
    /// is computed by the `tile_zoom` method. Increasing `tile_zoom_offset` increases the number
    /// of visible tiles for a given zoom value.
    pub tile_zoom_offset: f64,
}

impl OrthograficView {
    /// Constructs a new `OrthograficView`.
    pub fn new(
        viewport_size: Vector2<f64>,
        tile_size: u32,
        center: MapCoord,
        zoom: f64
    ) -> OrthograficView {
        OrthograficView {
            viewport_size,
            tile_size,
            center,
            zoom,
            tile_zoom_offset: 0.0,
        }
    }

    pub fn from_mercator_view(merc: &MercatorView) -> Self {
        OrthograficView {
            viewport_size: merc.viewport_size,
            tile_size: merc.tile_size,
            center: merc.center,
            zoom: merc.zoom,
            tile_zoom_offset: merc.tile_zoom_offset,
        }
    }

    pub fn from_toml_table(
        table: &Table,
        viewport_size: Vector2<f64>,
        tile_size: u32,
    ) -> Result<Self, String> {
        let x = match table.get("x") {
            Some(&Value::Float(x)) => x,
            Some(&Value::Integer(x)) => x as f64,
            Some(_) => return Err("x has to be a number.".to_string()),
            None => return Err("x position is missing.".to_string()),
        };

        let y = match table.get("y") {
            Some(&Value::Float(y)) => y,
            Some(&Value::Integer(y)) => y as f64,
            Some(_) => return Err("y has to be a number.".to_string()),
            None => return Err("y position is missing.".to_string()),
        };

        let zoom = match table.get("zoom") {
            Some(&Value::Float(z)) => z,
            Some(&Value::Integer(z)) => z as f64,
            Some(_) => return Err("zoom has to be a number.".to_string()),
            None => return Err("zoom value is missing.".to_string()),
        }.min(MAX_ZOOM_LEVEL).max(MIN_ZOOM_LEVEL);

        if let Some(&Value::String(ref s)) = table.get("projection") {
            if s != "orthografic" {
                return Err("try to deserialize wrong projection".to_string());
            }
        }

        Ok(OrthograficView {
            viewport_size,
            tile_size,
            center: MapCoord::new(x, y),
            zoom,
            tile_zoom_offset: 0.0,
        })
    }

    pub fn toml_table(&self) -> Table {
        let mut table = Table::new();
        table.insert("projection".to_string(), Value::String(Self::projection().to_str().to_string()));
        table.insert("x".to_string(), Value::Float(self.center.x));
        table.insert("y".to_string(), Value::Float(self.center.y));
        table.insert("zoom".to_string(), Value::Float(self.zoom));
        table
    }

    pub fn projection() -> Projection {
        Projection::Orthografic
    }

    /// Returns true if the rendering covers the whole viewport.
    pub fn covers_viewport(&self) -> bool {
        let sphere_diameter = 2.0f64.powf(self.zoom) *
            (f64::consts::FRAC_1_PI * f64::from(self.tile_size));

        // Add a little safety margin (the constant factor) since the rendered globe is not a
        // perfect sphere and its screen area is underestimated by the tesselation.
        self.viewport_size.x.hypot(self.viewport_size.y) < sphere_diameter * 0.9
    }

    /// Returns the tile zoom value that is used for rendering with the current zoom.
    //TODO Insert real implementation. Add TileCoord parameter -> lower resolution at the poles
    pub fn tile_zoom(&self) -> u32 {
        (self.zoom + self.tile_zoom_offset).floor().max(0.0) as u32
    }

    //TODO Return the transformation matrix that is used here to avoid redundant calculation.
    /// Returns a `Vec` of all tiles that are visible in the current viewport.
    pub fn visible_tiles(&self) -> Vec<VisibleTile> {
        let uzoom = self.tile_zoom();

        match uzoom {
            0 => return vec![TileCoord::new(0, 0, 0).into()],
            1 => {
                // return every tile
                return vec![
                    TileCoord::new(1, 0, 0).into(),
                    TileCoord::new(1, 0, 1).into(),
                    TileCoord::new(1, 1, 0).into(),
                    TileCoord::new(1, 1, 1).into(),
                ]},
            _ => {},
        }

        let center_tile = self.center.on_tile_at_zoom(uzoom).nearest_valid();

        let transform = self.transformation_matrix();

        let tile_is_visible = |tc: TileCoord| -> bool {
            let nw = tc.latlon_rad_north_west();
            let se = tc.latlon_rad_south_east();
            let vertices = [
                transform.transform_point(nw.to_sphere_point3()),
                transform.transform_point(se.to_sphere_point3()),
                transform.transform_point(LatLonRad::new(nw.lat, se.lon).to_sphere_point3()),
                transform.transform_point(LatLonRad::new(se.lat, nw.lon).to_sphere_point3()),
            ];

            if vertices.iter().all(|v| v.z > 0.0) {
                // Tile is on the backside of the sphere
                false
            } else {
                // Check bounding box of vertices against screen.
                //TODO Create true bounding box of tile that also accounts for curved borders.
                vertices.iter().fold(false, |acc, v| acc || v.x >= -1.0) &&
                vertices.iter().fold(false, |acc, v| acc || v.x <= 1.0) &&
                vertices.iter().fold(false, |acc, v| acc || v.y >= -1.0) &&
                vertices.iter().fold(false, |acc, v| acc || v.y <= 1.0)
            }
        };

        let mut tiles = vec![center_tile.into()];

        let mut stack: Vec<TileNeighbor> = vec![];
        tile_neighbors(center_tile, &mut stack);

        let mut visited: HashSet<TileNeighbor> = HashSet::new();
        visited.insert(TileNeighbor::Coord(center_tile));
        visited.extend(stack.iter());

        let mut neighbors = Vec::with_capacity(4);

        while let Some(tn) = stack.pop() {
            if let TileNeighbor::Coord(tc) = tn {
                if tile_is_visible(tc) {
                    tiles.push(tc.into());
                    tile_neighbors(tc, &mut neighbors);
                    for tn in &neighbors {
                        if !visited.contains(tn) {
                            visited.insert(*tn);
                            stack.push(*tn);
                        }
                    }
                }
            }
        }

        tiles
    }

    pub fn diameter_physical_pixels(&self) -> f64 {
        2.0f64.powf(self.zoom) * (FRAC_1_PI * f64::from(self.tile_size))
    }

    pub fn transformation_matrix(&self) -> Matrix3<f64> {
        let (scale_x, scale_y) = {
            let diam = self.diameter_physical_pixels();
            (diam / self.viewport_size.x, diam / self.viewport_size.y)
        };

        let scale_mat: Matrix3<f64> = Matrix3::from_cols(
            vec3(scale_x, 0.0, 0.0),
            vec3(0.0, scale_y, 0.0),
            vec3(0.0, 0.0, 1.0),
        );

        let center_latlon = self.center.to_latlon_rad();

        let rot_mat_x: Matrix3<f64> = {
            let alpha = center_latlon.lon + (PI * 0.5);
            let cosa = alpha.cos();
            let sina = alpha.sin();
            Matrix3::from_cols(
                vec3(cosa, 0.0, -sina),
                vec3(0.0, 1.0, 0.0),
                vec3(sina, 0.0, cosa),
            )
        };

        let rot_mat_y: Matrix3<f64> = {
            let alpha = -center_latlon.lat;
            let cosa = alpha.cos();
            let sina = alpha.sin();
            Matrix3::from_cols(
                vec3(1.0, 0.0, 0.0),
                vec3(0.0, cosa, sina),
                vec3(0.0, -sina, cosa),
            )
        };

        Transform::<Point3<f64>>::concat(
            &scale_mat,
            &Transform::<Point3<f64>>::concat(&rot_mat_y, &rot_mat_x)
        )
    }

    // Returns the inverse rotation matrix of the given view.
    pub fn inv_rotation_matrix(&self) -> Matrix3<f64> {
        let center_latlon = self.center.to_latlon_rad();

        let rot_mat_x: Matrix3<f64> = {
            let alpha = -center_latlon.lon - (PI * 0.5);
            let cosa = alpha.cos();
            let sina = alpha.sin();
            Matrix3::from_cols(
                vec3(cosa, 0.0, -sina),
                vec3(0.0, 1.0, 0.0),
                vec3(sina, 0.0, cosa),
            )
        };

        let rot_mat_y: Matrix3<f64> = {
            let alpha = center_latlon.lat;
            let cosa = alpha.cos();
            let sina = alpha.sin();
            Matrix3::from_cols(
                vec3(1.0, 0.0, 0.0),
                vec3(0.0, cosa, sina),
                vec3(0.0, -sina, cosa),
            )
        };

        Transform::<Point3<f64>>::concat(&rot_mat_x, &rot_mat_y)
    }

    // Returns the coordinates of the location that is nearest to the given `ScreenCoord`.
    pub fn screen_coord_to_sphere_point(&self, screen_coord: ScreenCoord) -> Point3<f64> {
        // Point on unit sphere
        let sphere_point = {
            let recip_radius = 2.0 * self.diameter_physical_pixels().recip();
            let sx = (screen_coord.x - self.viewport_size.x * 0.5) * recip_radius;
            let sy = (screen_coord.y - self.viewport_size.y * 0.5) * -recip_radius;
            let t = 1.0 - sx * sx - sy * sy;
            if t >= 0.0 {
                // screen_coord is on the sphere
                Point3::new(
                    sx,
                    sy,
                    -(t.sqrt()),
                )
            } else {
                // screen_coord is outside of sphere -> pick nearest.
                let scale = sx.hypot(sy).recip();
                Point3::new(
                    sx * scale,
                    sy * scale,
                    0.0,
                )
            }
        };

        // Rotate
        let inv_trans = self.inv_rotation_matrix();
        inv_trans.transform_point(sphere_point)
    }

    // Returns the coordinates of the location that is nearest to the given `ScreenCoord`.
    pub fn screen_coord_to_latlonrad(&self, screen_coord: ScreenCoord) -> LatLonRad {
        let p = self.screen_coord_to_sphere_point(screen_coord);

        // Transform to latitude, longitude
        LatLonRad::new(p.y.asin(), p.z.atan2(p.x))
    }

    /// Change zoom value by `zoom_delta` and zoom to a position given in screen coordinates.
    pub fn zoom_at(&mut self, pos: ScreenCoord, zoom_delta: f64) {
        //TODO Do something sophisticated: Increase zoom and rotate slightly so that the given
        // ScreenCoord points to the same geographical location.
        /*
        let latlon = self.screen_coord_to_latlonrad(pos);

        let delta_x = pos.x - self.viewport_size.x * 0.5;
        let delta_y = pos.y - self.viewport_size.y * 0.5;

        self.center = latlon.into();
        */

        self.zoom = (self.zoom + zoom_delta).min(MAX_ZOOM_LEVEL).max(MIN_ZOOM_LEVEL);
    }

    /// Change zoom value by `zoom_delta` and zoom to a position given in screen coordinates.
    pub fn set_zoom_at(&mut self, pos: ScreenCoord, zoom: f64) {
        //TODO Do something sophisticated
        self.zoom = zoom.min(MAX_ZOOM_LEVEL).max(MIN_ZOOM_LEVEL);
    }

    pub fn step_zoom(&mut self, steps: i32, step_size: f64) {
        self.zoom = {
            let z = (self.zoom + f64::from(steps) * step_size) / step_size;
            if steps > 0 {
                z.ceil() * step_size
            } else {
                z.floor() * step_size
            }
        }.max(MIN_ZOOM_LEVEL).min(MAX_ZOOM_LEVEL);
    }

    /// Move the center of the viewport by approx. (`delta_x`, `delta_y`) in screen coordinates.
    pub fn move_pixel(&mut self, delta_x: f64, delta_y: f64) {
        //TODO Do something more sophisticated
        let scale = f64::powf(2.0, -self.zoom) / f64::from(self.tile_size);
        self.center.x += delta_x * scale;
        self.center.y += delta_y * scale;
        self.center.normalize_xy();
    }
}

#[cfg(test)]
mod tests {
    use orthografic_view::*;

    #[test]
    fn tilecoord_neighbors() {
        let mut result = vec![];

        tile_neighbors(TileCoord::new(0, 0, 0), &mut result);
        assert!(result.is_empty());

        tile_neighbors(TileCoord::new(0, 0, -1), &mut result);
        assert!(result.is_empty());

        tile_neighbors(TileCoord::new(3, 0, -1), &mut result);
        assert!(result.is_empty());

        tile_neighbors(TileCoord::new(1, 0, 0), &mut result);
        assert_eq!(result.len(), 2);
        assert!(result.iter().find(|&&x| x == TileNeighbor::Coord(TileCoord::new(1, 1, 0))).is_some());
        assert!(result.iter().find(|&&x| x == TileNeighbor::Coord(TileCoord::new(1, 0, 1))).is_some());
        assert!(result.iter().find(|&&x| x == TileNeighbor::Coord(TileCoord::new(1, 1, 1))).is_none());

        tile_neighbors(TileCoord::new(2, 0, 0), &mut result);
        assert_eq!(result.len(), 4);
        assert!(result.iter().find(|&&x| x == TileNeighbor::NorthPole).is_some());

        tile_neighbors(TileCoord::new(2, 0, 3), &mut result);
        assert_eq!(result.len(), 4);
        assert!(result.iter().find(|&&x| x == TileNeighbor::SouthPole).is_some());

        tile_neighbors(TileCoord::new(2, 3, 1), &mut result);
        assert_eq!(result.len(), 4);
        assert!(result.iter().find(|&&x| x == TileNeighbor::Coord(TileCoord::new(2, 2, 1))).is_some());
        assert!(result.iter().find(|&&x| x == TileNeighbor::Coord(TileCoord::new(2, 0, 1))).is_some());
        assert!(result.iter().find(|&&x| x == TileNeighbor::Coord(TileCoord::new(2, 3, 0))).is_some());
        assert!(result.iter().find(|&&x| x == TileNeighbor::Coord(TileCoord::new(2, 3, 2))).is_some());
    }
}
