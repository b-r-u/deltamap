use buffer::{Buffer, DrawMode};
use cgmath::Transform;
use context::Context;
use coord::{LatLonRad, ScreenCoord, View};
use orthografic_view::OrthograficView;
use program::Program;
use std::ffi::CStr;
use tile_atlas::{TileAtlas, VisibleTilesProvider};
use tile_cache::TileCache;
use tile_source::TileSource;
use vertex_attrib::VertexAttribParams;


#[derive(Debug)]
pub struct OrthoTileLayer {
    program: Program,
    buffer: Buffer,
}

#[derive(Copy, Clone, Debug)]
pub struct LatScreenEllipse {
    pub center: ScreenCoord,
    pub radius_x: f64,
    pub radius_y: f64,
    /// longitude angle in radians at center + radius_y
    pub ref_angle: f64,
}

impl LatScreenEllipse {
    fn new(view_center: LatLonRad, viewport_size: (u32, u32), sphere_radius: f64, lat: f64) -> Self {
        LatScreenEllipse {
            center: ScreenCoord {
                x: f64::from(viewport_size.0) * 0.5,
                y: f64::from(viewport_size.1) * 0.5 * (lat - view_center.lat).sin() * sphere_radius,
            },
            radius_x: lat.cos() * sphere_radius,
            radius_y: lat.cos() * -view_center.lat.sin() * sphere_radius,
            ref_angle: view_center.lon,
        }
    }
}


impl OrthoTileLayer {
    pub fn new(
        cx: &mut Context,
        atlas: &TileAtlas,
    ) -> OrthoTileLayer
    {
        let buffer = Buffer::new(cx, &[], 0);
        cx.bind_buffer(buffer.id());

        let mut program = Program::new(
            cx,
            include_bytes!("../shader/ortho_tile.vert"),
            include_bytes!("../shader/ortho_tile.frag"),
        ).unwrap();

        program.add_texture(cx, atlas.texture(), CStr::from_bytes_with_nul(b"tex_map\0").unwrap());

        program.add_attribute(
            cx,
            CStr::from_bytes_with_nul(b"position\0").unwrap(),
            &VertexAttribParams::new(3, 9, 0)
        );
        program.add_attribute(
            cx,
            CStr::from_bytes_with_nul(b"tex_coord\0").unwrap(),
            &VertexAttribParams::new(2, 9, 3)
        );
        program.add_attribute(
            cx,
            CStr::from_bytes_with_nul(b"tex_minmax\0").unwrap(),
            &VertexAttribParams::new(4, 9, 5)
        );

        OrthoTileLayer {
            program,
            buffer,
        }
    }

    // Has to be called once before one or multiple calls to `draw`.
    pub fn prepare_draw(&mut self, cx: &mut Context, atlas: &TileAtlas) {
        self.program.enable_vertex_attribs(cx);
        self.program.set_vertex_attribs(cx, &self.buffer);
        cx.set_active_texture_unit(atlas.texture().unit());
    }

    pub fn draw(
        &mut self,
        cx: &mut Context,
        ortho: &OrthograficView,
        source: &TileSource,
        cache: &mut TileCache,
        tile_atlas: &mut TileAtlas,
    ) -> Result<usize, usize> {
        //TODO Add distance function to TileCache that takes topology of the sphere into account.
        cache.set_view_location(View {
            source_id: source.id(),
            zoom: ortho.tile_zoom(),
            center: ortho.center,
        });

        let transform = ortho.transformation_matrix();

        let visible_tiles = ortho.visible_tiles();
        let mut remainder = visible_tiles.as_slice();
        let mut num_draws = 0;
        let mut max_tiles_to_use = cache.max_tiles();

        loop {
            let (textured_visible_tiles, remainder_opt, used_tiles) = {
                tile_atlas.textured_visible_tiles(
                    cx,
                    remainder,
                    max_tiles_to_use,
                    source,
                    cache,
                )
            };

            max_tiles_to_use = max_tiles_to_use.saturating_sub(used_tiles);

            // A low guess for the capacity
            let mut vertex_data = Vec::with_capacity(textured_visible_tiles.len() * 9 * 16);

            for tvt in &textured_visible_tiles {
                let minmax = [
                    tvt.tex_minmax.x1 as f32,
                    tvt.tex_minmax.y1 as f32,
                    tvt.tex_minmax.x2 as f32,
                    tvt.tex_minmax.y2 as f32,
                ];

                let subdivision = 6u32.saturating_sub(tvt.tile_coord.zoom).max(2);

                for (tc, sub_tile) in tvt.tile_coord.children_iter(subdivision) {
                    let ll_nw = tc.latlon_rad_north_west();
                    let ll_se = tc.latlon_rad_south_east();
                    let ll_ne = LatLonRad::new(ll_nw.lat, ll_se.lon);
                    let ll_sw = LatLonRad::new(ll_se.lat, ll_nw.lon);

                    let p1 = transform.transform_point(ll_nw.to_sphere_point3());
                    let p2 = transform.transform_point(ll_ne.to_sphere_point3());
                    let p3 = transform.transform_point(ll_se.to_sphere_point3());
                    let p4 = transform.transform_point(ll_sw.to_sphere_point3());

                    // Discard tiles/subtiles that are facing backwards
                    if (p1.z + p3.z) * 0.5 > 0.0 {
                        continue;
                    }

                    let texrect = tvt.tex_rect.subdivide(&sub_tile);

                    let p1 = [p1.x as f32, p1.y as f32, p1.z as f32, texrect.x1 as f32, texrect.y1 as f32];
                    let p2 = [p2.x as f32, p2.y as f32, p2.z as f32, texrect.x2 as f32, texrect.y1 as f32];
                    let p3 = [p3.x as f32, p3.y as f32, p3.z as f32, texrect.x2 as f32, texrect.y2 as f32];
                    let p4 = [p4.x as f32, p4.y as f32, p4.z as f32, texrect.x1 as f32, texrect.y2 as f32];

                    vertex_data.extend(&p1);
                    vertex_data.extend(&minmax);
                    vertex_data.extend(&p2);
                    vertex_data.extend(&minmax);
                    vertex_data.extend(&p3);
                    vertex_data.extend(&minmax);
                    vertex_data.extend(&p1);
                    vertex_data.extend(&minmax);
                    vertex_data.extend(&p3);
                    vertex_data.extend(&minmax);
                    vertex_data.extend(&p4);
                    vertex_data.extend(&minmax);
                }
            }

            self.buffer.set_data(cx, &vertex_data, vertex_data.len() / 4);
            self.buffer.draw(cx, &self.program, DrawMode::Triangles);

            num_draws += 1;

            debug!("draw #{}: tvt.len() = {}, remainder = {:?}, max_tiles = {}",
                num_draws,
                textured_visible_tiles.len(),
                remainder_opt.map(|r| r.len()),
                max_tiles_to_use);

            if max_tiles_to_use == 0 {
                warn!("tile cache is too small for this view.");
                return Err(num_draws);
            }

            match remainder_opt {
                None => return Ok(num_draws),
                Some(new_remainder) => {
                    if new_remainder.len() >= remainder.len() {
                        warn!("failed to draw all tiles. number of remaining tiles did not decrease.");
                        return Err(num_draws);
                    } else {
                        remainder = new_remainder;
                    }
                },
            }
        }
    }
}
