use std::f32::consts::{PI, FRAC_1_PI};
use std::ffi::CStr;
use buffer::{Buffer, DrawMode};
use cgmath::{Matrix3, Point3, Transform, vec3};
use context::Context;
use coord::{LatLonRad, TileCoord, View};
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
        viewport_size: (u32, u32),
    ) {
        cache.set_view_location(View {
            source_id: source.id(),
            zoom: map_view.tile_zoom(),
            center: map_view.center,
        });

        let mut vertex_data = vec![];

        let (scale_x, scale_y) = {
            let factor = 2.0f32.powf(map_view.zoom as f32) *
                (FRAC_1_PI * map_view.tile_size as f32);
            (factor / viewport_size.0 as f32, factor / viewport_size.1 as f32)
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

        let (inset_x, inset_y) = tile_atlas.texture_margins();

        for tile_y in 0..8 {
            for tile_x in 0..8 {
                let tc = TileCoord::new(3, tile_x, tile_y);
                let slot = tile_atlas.store(cx, tc, source, cache, true)
                    .unwrap_or_else(TileAtlas::default_slot);
                let texrect = tile_atlas.slot_to_texture_rect(slot);
                let tex_minmax = texrect.inset(inset_x, inset_y);

                let minmax = [
                    tex_minmax.x1 as f32,
                    tex_minmax.y1 as f32,
                    tex_minmax.x2 as f32,
                    tex_minmax.y2 as f32,
                ];

                for (tc, sub_tile) in tc.children_iter(3) {
                    let ll_nw = tc.latlon_rad_north_west();
                    let ll_se = {
                        let tc = TileCoord::new(tc.zoom, tc.x + 1, tc.y + 1);
                        tc.latlon_rad_north_west()
                    };

                    let ll_ne = LatLonRad::new(ll_nw.lat, ll_se.lon);
                    let ll_sw = LatLonRad::new(ll_se.lat, ll_nw.lon);

                    let p1 = ll_nw.to_sphere_point3(1.0);
                    let p2 = ll_ne.to_sphere_point3(1.0);
                    let p3 = ll_se.to_sphere_point3(1.0);
                    let p4 = ll_sw.to_sphere_point3(1.0);

                    let p1 = transform.transform_point(p1);
                    let p2 = transform.transform_point(p2);
                    let p3 = transform.transform_point(p3);
                    let p4 = transform.transform_point(p4);

                    if p1.z > 0.0 && p2.z > 0.0 && p3.z > 0.0 && p4.z > 0.0 {
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
        }

        self.buffer.set_data(cx, &vertex_data, vertex_data.len() / 4);
        self.buffer.draw(cx, &self.program, DrawMode::Triangles);
    }
}
