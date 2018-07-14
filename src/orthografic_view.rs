use cgmath::{Matrix3, Point3, Transform, vec3};
use coord::{LatLonRad, ScreenCoord, TextureRect, TileCoord};
use map_view::MapView;
use std::collections::HashSet;
use std::f64::consts::{PI, FRAC_1_PI};
use std::f64;


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


#[derive(Clone, Debug)]
pub struct OrthograficView {
}

impl OrthograficView {
    /// Returns true if the rendering covers the whole viewport.
    pub fn covers_viewport(map_view: &MapView) -> bool {
        //TODO Add a little safety margin since the rendered globe is not a perfect sphere and its
        // screen area is underestimated by the tesselation.
        let sphere_diameter = 2.0f64.powf(map_view.zoom) *
            (f64::consts::FRAC_1_PI * f64::from(map_view.tile_size));

        map_view.width.hypot(map_view.height) < sphere_diameter
    }

    /// Returns the tile zoom value that is used for rendering with the current zoom.
    //TODO Insert real implementation. Add TileCoord parameter -> lower resolution at the poles
    pub fn tile_zoom(map_view: &MapView) -> u32 {
        (map_view.zoom + map_view.tile_zoom_offset).floor().max(0.0) as u32
    }

    //TODO Return the transformation matrix that is used here to avoid redundant calculation.
    /// Returns a `Vec` of all tiles that are visible in the current viewport.
    pub fn visible_tiles(map_view: &MapView) -> Vec<VisibleTile> {
        let uzoom = Self::tile_zoom(map_view);

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

        let center_tile = map_view.center.on_tile_at_zoom(uzoom).globe_norm();

        let transform = Self::transformation_matrix(map_view);

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

    pub fn diameter_physical_pixels(map_view: &MapView) -> f64 {
        2.0f64.powf(map_view.zoom) * (FRAC_1_PI * f64::from(map_view.tile_size))
    }

    pub fn transformation_matrix(map_view: &MapView) -> Matrix3<f64> {
        let (scale_x, scale_y) = {
            let diam = Self::diameter_physical_pixels(map_view);
            (diam / map_view.width, diam / map_view.height)
        };

        let scale_mat: Matrix3<f64> = Matrix3::from_cols(
            vec3(scale_x, 0.0, 0.0),
            vec3(0.0, scale_y, 0.0),
            vec3(0.0, 0.0, 1.0),
        );

        let center_latlon = map_view.center.to_latlon_rad();

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
    pub fn inv_rotation_matrix(map_view: &MapView) -> Matrix3<f64> {
        let center_latlon = map_view.center.to_latlon_rad();

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
    pub fn screen_coord_to_latlonrad(map_view: &MapView, screen_coord: ScreenCoord) -> LatLonRad {
        // Point on unit sphere
        let sphere_point = {
            let recip_radius = 2.0 * Self::diameter_physical_pixels(map_view).recip();
            let sx = (screen_coord.x - map_view.width * 0.5) * recip_radius;
            let sy = (screen_coord.y - map_view.height * 0.5) * -recip_radius;
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
        let inv_trans = Self::inv_rotation_matrix(map_view);
        let p = inv_trans.transform_point(sphere_point);

        // Transform to latitude, longitude
        LatLonRad::new(p.y.asin(), p.z.atan2(p.x))
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
        assert_eq!(result.len(), 2);
        assert!(result.iter().find(|&&x| x == TileNeighbor::NorthPole).is_some());

        tile_neighbors(TileCoord::new(2, 0, 3), &mut result);
        assert_eq!(result.len(), 2);
        assert!(result.iter().find(|&&x| x == TileNeighbor::SouthPole).is_some());

        tile_neighbors(TileCoord::new(2, 3, 1), &mut result);
        assert_eq!(result.len(), 4);
        assert!(result.iter().find(|&&x| x == TileNeighbor::Coord(TileCoord::new(2, 2, 1))).is_some());
        assert!(result.iter().find(|&&x| x == TileNeighbor::Coord(TileCoord::new(2, 0, 1))).is_some());
        assert!(result.iter().find(|&&x| x == TileNeighbor::Coord(TileCoord::new(2, 3, 0))).is_some());
        assert!(result.iter().find(|&&x| x == TileNeighbor::Coord(TileCoord::new(2, 3, 2))).is_some());
    }
}
