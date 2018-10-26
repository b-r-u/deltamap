use ::std::ffi::CStr;
use buffer::{Buffer, DrawMode};
use cgmath::{Matrix3, Point2, Transform, vec2, vec3};
use context::Context;
use coord::{MapCoord, ScreenCoord, ScreenRect};
use image;
use mercator_view::MercatorView;
use orthografic_view::OrthograficView;
use program::Program;
use texture::Texture;
use vertex_attrib::VertexAttribParams;


#[derive(Debug)]
pub struct MarkerLayer {
    buffer: Buffer,
    program: Program,
    texture: Texture,
    positions: Vec<MapCoord>,
}

impl MarkerLayer {
    pub fn new(cx: &mut Context) -> MarkerLayer {
        let buffer = Buffer::new(cx, &[], 0);
        cx.bind_buffer(buffer.id());
        check_gl_errors!(cx);

        let mut program = Program::new(
            cx,
            include_bytes!("../shader/marker.vert"),
            include_bytes!("../shader/marker.frag"),
        ).unwrap();

        //TODO Create textures for higher DPI factors / use mipmaps
        let texture = {
            let img = image::load_from_memory(
                include_bytes!("../img/marker.png"),
            ).unwrap();
            Texture::new(cx, &img).unwrap()
        };

        program.add_texture(cx, &texture, CStr::from_bytes_with_nul(b"tex\0").unwrap());

        program.add_attribute(
            cx,
            CStr::from_bytes_with_nul(b"position\0").unwrap(),
            &VertexAttribParams::new(2, 4, 0)
        );
        program.add_attribute(
            cx,
            CStr::from_bytes_with_nul(b"tex_coord\0").unwrap(),
            &VertexAttribParams::new(2, 4, 2)
        );

        MarkerLayer {
            buffer,
            program,
            texture,
            positions: vec![],
        }
    }

    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    pub fn add_marker(&mut self, map_coord: MapCoord) {
        self.positions.push(map_coord);
    }

    // Has to be called once before one or multiple calls to `draw`.
    pub fn prepare_draw(&mut self, cx: &mut Context) {
        cx.set_active_texture_unit(self.texture.unit());
        self.program.enable_vertex_attribs(cx);
        self.program.set_vertex_attribs(cx, &self.buffer);
    }

    pub fn draw_mercator(
        &mut self,
        cx: &mut Context,
        merc: &MercatorView,
        dpi_factor: f64,
        snap_to_pixel: bool
    ) {
        let mut vertex_data: Vec<f32> = vec![];

        let marker_size = vec2::<f64>(40.0, 50.0) * dpi_factor;
        let marker_offset = vec2::<f64>(-20.0, -50.0) * dpi_factor;

        let scale_x = 2.0 / merc.viewport_size.x as f32;
        let scale_y = -2.0 / merc.viewport_size.y as f32;

        let tex_mat: Matrix3<f32> = Matrix3::from_cols(
            vec3(marker_size.x as f32, 0.0, 0.0),
            vec3(0.0, marker_size.y as f32, 0.0),
            vec3(marker_offset.x as f32, marker_offset.y as f32, 1.0),
        );

        let screen_mat: Matrix3<f32> = Matrix3::from_cols(
            vec3(scale_x, 0.0, 0.0),
            vec3(0.0, scale_y, 0.0),
            vec3(-1.0, 1.0, 1.0),
        );

        let t1 = Point2::new(0.0f32, 0.0);
        let t2 = Point2::new(1.0f32, 0.0);
        let t3 = Point2::new(1.0f32, 1.0);
        let t4 = Point2::new(0.0f32, 1.0);

        let visible_rect = ScreenRect {
            x: -(marker_offset.x + marker_size.x),
            y: -(marker_offset.y + marker_size.y),
            width: merc.viewport_size.x + marker_size.x,
            height: merc.viewport_size.y + marker_size.y,
        };

        for map_pos in &self.positions {
            let screen_pos = {
                let mut sp = merc.map_to_screen_coord(*map_pos);
                if snap_to_pixel {
                    let topleft = merc.map_to_screen_coord(MapCoord::new(0.0, 0.0));
                    let mut snapped = topleft;
                    snapped.snap_to_pixel();

                    sp.x += snapped.x - topleft.x;
                    sp.y += snapped.y - topleft.y;
                }
                sp
            };

            if !screen_pos.is_inside(&visible_rect) {
                continue;
            }
            let trans_mat: Matrix3<f32> = Matrix3::from_cols(
                vec3(0.0, 0.0, 0.0),
                vec3(0.0, 0.0, 0.0),
                vec3(screen_pos.x as f32, screen_pos.y as f32, 0.0),
            );
            let mat: Matrix3<f32> = screen_mat * (tex_mat + trans_mat);

            let p1: Point2<f32> = mat.transform_point(t1);
            let p2: Point2<f32> = mat.transform_point(t2);
            let p3: Point2<f32> = mat.transform_point(t3);
            let p4: Point2<f32> = mat.transform_point(t4);

            vertex_data.extend::<&[f32; 2]>(p1.as_ref());
            vertex_data.extend::<&[f32; 2]>(t1.as_ref());
            vertex_data.extend::<&[f32; 2]>(p2.as_ref());
            vertex_data.extend::<&[f32; 2]>(t2.as_ref());
            vertex_data.extend::<&[f32; 2]>(p3.as_ref());
            vertex_data.extend::<&[f32; 2]>(t3.as_ref());
            vertex_data.extend::<&[f32; 2]>(p1.as_ref());
            vertex_data.extend::<&[f32; 2]>(t1.as_ref());
            vertex_data.extend::<&[f32; 2]>(p3.as_ref());
            vertex_data.extend::<&[f32; 2]>(t3.as_ref());
            vertex_data.extend::<&[f32; 2]>(p4.as_ref());
            vertex_data.extend::<&[f32; 2]>(t4.as_ref());
        }

        self.buffer.set_data(cx, &vertex_data, vertex_data.len() / 4);
        self.buffer.draw(cx, &self.program, DrawMode::Triangles);
    }


    pub fn draw_ortho(
        &mut self,
        cx: &mut Context,
        ortho: &OrthograficView,
        dpi_factor: f64,
    ) {
        let mut vertex_data: Vec<f32> = vec![];

        let marker_size = vec2::<f64>(40.0, 50.0) * dpi_factor;
        let marker_offset = vec2::<f64>(-20.0, -50.0) * dpi_factor;

        let scale_x = 2.0 / ortho.viewport_size.x as f32;
        let scale_y = -2.0 / ortho.viewport_size.y as f32;

        let tex_mat: Matrix3<f32> = Matrix3::from_cols(
            vec3(marker_size.x as f32, 0.0, 0.0),
            vec3(0.0, marker_size.y as f32, 0.0),
            vec3(marker_offset.x as f32, marker_offset.y as f32, 1.0),
        );

        let screen_mat: Matrix3<f32> = Matrix3::from_cols(
            vec3(scale_x, 0.0, 0.0),
            vec3(0.0, scale_y, 0.0),
            vec3(-1.0, 1.0, 1.0),
        );

        let t1 = Point2::new(0.0f32, 0.0);
        let t2 = Point2::new(1.0f32, 0.0);
        let t3 = Point2::new(1.0f32, 1.0);
        let t4 = Point2::new(0.0f32, 1.0);

        let visible_rect = ScreenRect {
            x: -(marker_offset.x + marker_size.x),
            y: -(marker_offset.y + marker_size.y),
            width: ortho.viewport_size.x + marker_size.x,
            height: ortho.viewport_size.y + marker_size.y,
        };

        let transform = ortho.transformation_matrix();

        for map_pos in &self.positions {
            let screen_pos = {
                let norm_pos = transform.transform_point(map_pos.to_latlon_rad().to_sphere_point3());

                if norm_pos.z > 0.0 {
                    continue;
                }

                ScreenCoord::new(
                    (norm_pos.x + 1.0) * (0.5 * ortho.viewport_size.x),
                    (-norm_pos.y + 1.0) * (0.5 * ortho.viewport_size.y),
                )
            };

            if !screen_pos.is_inside(&visible_rect) {
                continue;
            }
            let trans_mat: Matrix3<f32> = Matrix3::from_cols(
                vec3(0.0, 0.0, 0.0),
                vec3(0.0, 0.0, 0.0),
                vec3(screen_pos.x as f32, screen_pos.y as f32, 0.0),
            );
            let mat: Matrix3<f32> = screen_mat * (tex_mat + trans_mat);

            let p1: Point2<f32> = mat.transform_point(t1);
            let p2: Point2<f32> = mat.transform_point(t2);
            let p3: Point2<f32> = mat.transform_point(t3);
            let p4: Point2<f32> = mat.transform_point(t4);

            vertex_data.extend::<&[f32; 2]>(p1.as_ref());
            vertex_data.extend::<&[f32; 2]>(t1.as_ref());
            vertex_data.extend::<&[f32; 2]>(p2.as_ref());
            vertex_data.extend::<&[f32; 2]>(t2.as_ref());
            vertex_data.extend::<&[f32; 2]>(p3.as_ref());
            vertex_data.extend::<&[f32; 2]>(t3.as_ref());
            vertex_data.extend::<&[f32; 2]>(p1.as_ref());
            vertex_data.extend::<&[f32; 2]>(t1.as_ref());
            vertex_data.extend::<&[f32; 2]>(p3.as_ref());
            vertex_data.extend::<&[f32; 2]>(t3.as_ref());
            vertex_data.extend::<&[f32; 2]>(p4.as_ref());
            vertex_data.extend::<&[f32; 2]>(t4.as_ref());
        }

        self.buffer.set_data(cx, &vertex_data, vertex_data.len() / 4);
        self.buffer.draw(cx, &self.program, DrawMode::Triangles);
    }
}
