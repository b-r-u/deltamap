use std::ffi::CStr;
use buffer::{Buffer, DrawMode};
use cgmath::Transform;
use context::Context;
use coord::{LatLonRad, ScreenCoord, TileCoord, View};
use map_view::MapView;
use program::Program;
use tile_atlas::TileAtlas;
use tile_cache::TileCache;
use tile_source::TileSource;
use vertex_attrib::VertexAttribParams;


#[derive(Debug)]
pub struct GlobeTileLayer {
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
    fn new(view_center: LatLonRad, viewport_size: (u32, u32), globe_radius: f64, lat: f64) -> Self {
        LatScreenEllipse {
            center: ScreenCoord {
                x: viewport_size.0 as f64 * 0.5,
                y: viewport_size.1 as f64 * 0.5 * (lat - view_center.lat).sin() * globe_radius,
            },
            radius_x: lat.cos() * globe_radius,
            radius_y: lat.cos() * -view_center.lat.sin() * globe_radius,
            ref_angle: view_center.lon,
        }
    }
}


impl GlobeTileLayer {
    pub fn new(
        cx: &mut Context,
    ) -> GlobeTileLayer
    {
        let buffer = Buffer::new(cx, &[], 0);
        check_gl_errors!(cx);
        cx.bind_buffer(buffer.id());

        let mut program = Program::new(
            cx,
            include_bytes!("../shader/globe_tile.vert"),
            include_bytes!("../shader/globe_tile.frag"),
        ).unwrap();
        check_gl_errors!(cx);

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
        check_gl_errors!(cx);

        GlobeTileLayer {
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
        map_view: &MapView,
        source: &TileSource,
        cache: &mut TileCache,
        tile_atlas: &mut TileAtlas,
    ) {
        //TODO Add distance function to TileCache that takes topology of the sphere into account.
        cache.set_view_location(View {
            source_id: source.id(),
            zoom: map_view.tile_zoom(),
            center: map_view.center,
        });

        let mut vertex_data = vec![];

        let transform = map_view.globe_transformation_matrix();

        let (inset_x, inset_y) = tile_atlas.texture_margins();

        for tile_coord in map_view.visible_globe_tiles().into_iter() {
            let slot = tile_atlas.store(cx, tile_coord, source, cache, true)
                .unwrap_or_else(TileAtlas::default_slot);
            let texrect = tile_atlas.slot_to_texture_rect(slot);
            let tex_minmax = texrect.inset(inset_x, inset_y);

            let minmax = [
                tex_minmax.x1 as f32,
                tex_minmax.y1 as f32,
                tex_minmax.x2 as f32,
                tex_minmax.y2 as f32,
            ];

            let subdivision = 6u32.saturating_sub(tile_coord.zoom).max(2);

            for (tc, sub_tile) in tile_coord.children_iter(subdivision) {
                let ll_nw = tc.latlon_rad_north_west();
                let ll_se = {
                    let tc = TileCoord::new(tc.zoom, tc.x + 1, tc.y + 1);
                    tc.latlon_rad_north_west()
                };

                let ll_ne = LatLonRad::new(ll_nw.lat, ll_se.lon);
                let ll_sw = LatLonRad::new(ll_se.lat, ll_nw.lon);

                let p1 = ll_nw.to_sphere_point3();
                let p2 = ll_ne.to_sphere_point3();
                let p3 = ll_se.to_sphere_point3();
                let p4 = ll_sw.to_sphere_point3();

                let p1 = transform.transform_point(p1);
                let p2 = transform.transform_point(p2);
                let p3 = transform.transform_point(p3);
                let p4 = transform.transform_point(p4);

                // Discard tiles that are facing backwards
                if (p1.z + p3.z) * 0.5 > 0.0 {
                    continue;
                }

                let texrect = texrect.subdivide(&sub_tile);

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
    }
}
