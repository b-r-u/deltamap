use cgmath::{Matrix3, Point3, Transform, vec3};
use coord::TileCoord;
use map_view::MapView;
use std::f32::consts::{PI, FRAC_1_PI};
use std::f64;


#[derive(Clone, Debug)]
pub struct OrthograficView {
}

impl OrthograficView {
    /// Returns true if the rendering covers the whole viewport.
    pub fn covers_viewport(map_view: &MapView) -> bool {
        //TODO Add a little safety margin since the rendered globe is not a perfect sphere and its
        // screen area is underestimated by the tesselation.
        let sphere_diameter = 2.0f64.powf(map_view.zoom) *
            (f64::consts::FRAC_1_PI * map_view.tile_size as f64);

        return map_view.width.hypot(map_view.height) < sphere_diameter;
    }

    /// Returns the tile zoom value that is used for rendering with the current zoom.
    //TODO Insert real implementation. Add TileCoord parameter -> lower resolution at the poles
    pub fn tile_zoom(map_view: &MapView) -> u32 {
        (map_view.zoom + map_view.tile_zoom_offset).floor().max(0.0) as u32
    }

    //TODO Return the transformation matrix that is used here to avoid redundant calculation.
    /// Returns a `Vec` of all tiles that are visible in the current viewport.
    pub fn visible_tiles(map_view: &MapView) -> Vec<TileCoord> {
        let uzoom = Self::tile_zoom(map_view);

        match uzoom {
            0 => return vec![TileCoord::new(0, 0, 0)],
            1 => {
                // return every tile
                return vec![
                    TileCoord::new(1, 0, 0),
                    TileCoord::new(1, 0, 1),
                    TileCoord::new(1, 1, 0),
                    TileCoord::new(1, 1, 1),
                ]},
            _ => {},
        }

        let center_tile = map_view.center.on_tile_at_zoom(uzoom).globe_norm();

        let transform = Self::transformation_matrix(map_view);

        let add_tile_if_visible = |tc: TileCoord, vec: &mut Vec<TileCoord>| -> bool {
            let test_point = tc.latlon_rad_north_west().to_sphere_point3();
            let test_point = transform.transform_point(test_point);

            let visible = test_point.x >= -1.0 && test_point.x <= 1.0 &&
                test_point.y >= -1.0 && test_point.y <= 1.0;

            if visible {
                vec.push(tc);
                true
            } else {
                false
            }
        };

        let mut tiles = vec![];

        {
            let zoom_level_tiles = TileCoord::get_zoom_level_tiles(uzoom);

            for dx in 0..(zoom_level_tiles / 2) {
                let v = add_tile_if_visible(TileCoord::new(uzoom, center_tile.x + dx, center_tile.y), &mut tiles);
                if !v {
                    break;
                }
            }
            for dx in 1..(1 + zoom_level_tiles / 2) {
                let v = add_tile_if_visible(TileCoord::new(uzoom, center_tile.x - dx, center_tile.y), &mut tiles);
                if !v {
                    break;
                }
            }

            // move south
            for y in (center_tile.y + 1)..zoom_level_tiles {
                let mut visible = false;

                for dx in 0..(zoom_level_tiles / 2) {
                    let v = add_tile_if_visible(TileCoord::new(uzoom, center_tile.x + dx, y), &mut tiles);
                    visible = visible || v;
                    if !v {
                        break;
                    }
                }
                for dx in 1..(1 + zoom_level_tiles / 2) {
                    let v = add_tile_if_visible(TileCoord::new(uzoom, center_tile.x - dx, y), &mut tiles);
                    visible = visible || v;
                    if !v {
                        break;
                    }
                }

                if !visible {
                    break;
                }
            }

            // move north
            for y in (0..center_tile.y).rev() {
                let mut visible = false;

                for dx in 0..(zoom_level_tiles / 2) {
                    let v = add_tile_if_visible(TileCoord::new(uzoom, center_tile.x + dx, y), &mut tiles);
                    visible = visible || v;
                    if !v {
                        break;
                    }
                }
                for dx in 1..(1 + zoom_level_tiles / 2) {
                    let v = add_tile_if_visible(TileCoord::new(uzoom, center_tile.x - dx, y), &mut tiles);
                    visible = visible || v;
                    if !v {
                        break;
                    }
                }

                if !visible {
                    break;
                }
            }
        }

        tiles
    }

    pub fn transformation_matrix(map_view: &MapView) -> Matrix3<f32> {
        let (scale_x, scale_y) = {
            let factor = 2.0f32.powf(map_view.zoom as f32) *
                (FRAC_1_PI * map_view.tile_size as f32);
            (factor / map_view.width as f32, factor / map_view.height as f32)
        };

        let scale_mat: Matrix3<f32> = Matrix3::from_cols(
            vec3(scale_x, 0.0, 0.0),
            vec3(0.0, scale_y, 0.0),
            vec3(0.0, 0.0, 1.0),
        );

        let rot_mat_x: Matrix3<f32> = {
            let center_latlon = map_view.center.to_latlon_rad();
            let alpha = center_latlon.lon as f32 + (PI * 0.5);
            let cosa = alpha.cos();
            let sina = alpha.sin();
                Matrix3::from_cols(
                vec3(cosa, 0.0, -sina),
                vec3(0.0, 1.0, 0.0),
                vec3(sina, 0.0, cosa),
            )
        };

        let rot_mat_y: Matrix3<f32> = {
            let center_latlon = map_view.center.to_latlon_rad();
            let alpha = (-center_latlon.lat) as f32;
            let cosa = alpha.cos();
            let sina = alpha.sin();
                Matrix3::from_cols(
                vec3(1.0, 0.0, 0.0),
                vec3(0.0, cosa, sina),
                vec3(0.0, -sina, cosa),
            )
        };

        let transform = Transform::<Point3<f32>>::concat(&rot_mat_y, &rot_mat_x);
        let transform = Transform::<Point3<f32>>::concat(&scale_mat, &transform);
        transform
    }
}
