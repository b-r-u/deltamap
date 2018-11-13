use ::std::ffi::CStr;
use buffer::{Buffer, DrawMode};
use cgmath::{InnerSpace, Matrix3, Point2, Transform, Vector2, vec3};
use context::Context;
use coord::{MapCoord};
use mercator_view::MercatorView;
use orthografic_view::OrthograficView;
use program::{Program, UniformId};
use vertex_attrib::VertexAttribParams;


#[derive(Clone, Copy, Debug)]
pub enum PathElement {
    MoveTo(MapCoord),
    LineTo(MapCoord),
    ClosePath,
}

#[derive(Debug)]
pub struct PathLayer {
    buffer: Buffer,
    program: Program,
    scale_uniform: UniformId,
    half_width_uniform: UniformId,
    color_uniform: UniformId,
    path: Vec<PathElement>,
}

impl PathLayer {
    pub fn new(cx: &mut Context) -> PathLayer {
        let buffer = Buffer::new(cx, &[], 0);
        cx.bind_buffer(buffer.id());
        check_gl_errors!(cx);

        let mut program = Program::new(
            cx,
            include_bytes!("../shader/path.vert"),
            include_bytes!("../shader/path.frag"),
        ).unwrap();

        program.add_attribute(
            cx,
            CStr::from_bytes_with_nul(b"position\0").unwrap(),
            &VertexAttribParams::new(2, 4, 0)
        );
        program.add_attribute(
            cx,
            CStr::from_bytes_with_nul(b"extrusion\0").unwrap(),
            &VertexAttribParams::new(2, 4, 2)
        );

        let scale_uniform = program.get_uniform_id(cx, CStr::from_bytes_with_nul(b"scale\0").unwrap()).unwrap();
        let half_width_uniform = program.get_uniform_id(cx, CStr::from_bytes_with_nul(b"half_width\0").unwrap()).unwrap();
        let color_uniform = program.get_uniform_id(cx, CStr::from_bytes_with_nul(b"color\0").unwrap()).unwrap();

        PathLayer {
            buffer,
            program,
            scale_uniform,
            half_width_uniform,
            color_uniform,
            path: vec![],
        }
    }

    pub fn is_empty(&self) -> bool {
        self.path.is_empty()
    }

    pub fn add_element(&mut self, ele: PathElement) {
        self.path.push(ele);
    }

    pub fn line_to(&mut self, map_coord: MapCoord) {
        self.path.push(PathElement::LineTo(map_coord));
    }

    pub fn move_to(&mut self, map_coord: MapCoord) {
        self.path.push(PathElement::MoveTo(map_coord));
    }

    pub fn close_path(&mut self) {
        self.path.push(PathElement::ClosePath);
    }

    // Has to be called once before one or multiple calls to `draw`.
    pub fn prepare_draw(&mut self, cx: &mut Context) {
        self.program.enable_vertex_attribs(cx);
        self.program.set_vertex_attribs(cx, &self.buffer);
    }

    #[inline]
    fn vec_ortho(v: Vector2<f32>) -> Vector2<f32> {
        Vector2::new(v.y, -v.x)
    }

    fn add_line_join(
        point_a: Point2<f32>,
        normal_a: Vector2<f32>,
        normal_b: Vector2<f32>,
        vertex_data: &mut Vec<f32>
    ) {
        let dot = normal_b.dot(normal_a);
        let perp_dot = normal_b.perp_dot(normal_a);

        if dot >= 0.0 {
            // angle between segments is 90° or more

            let extrusion_normal = (normal_a + normal_b).normalize();
            let extrusion = extrusion_normal / normal_a.dot(extrusion_normal);

            if perp_dot > 0.0 {
                vertex_data.extend::<&[f32; 2]>(point_a.as_ref());
                vertex_data.extend::<&[f32; 2]>((-normal_b).as_ref());
                vertex_data.extend::<&[f32; 2]>(point_a.as_ref());
                vertex_data.extend::<&[f32; 2]>((-extrusion).as_ref());
                vertex_data.extend::<&[f32; 2]>(point_a.as_ref());
                vertex_data.extend::<&[f32; 2]>((-extrusion).as_ref());
            } else if perp_dot < 0.0 {
                vertex_data.extend::<&[f32; 2]>(point_a.as_ref());
                vertex_data.extend::<&[f32; 2]>(extrusion.as_ref());
                vertex_data.extend::<&[f32; 2]>(point_a.as_ref());
                vertex_data.extend::<&[f32; 2]>(normal_b.as_ref());
                vertex_data.extend::<&[f32; 2]>(point_a.as_ref());
                vertex_data.extend::<&[f32; 2]>(normal_b.as_ref());
            }
        } else {
            // angle between segments is less than 90°

            let tangent_a = -Self::vec_ortho(normal_a);
            let tangent_b = Self::vec_ortho(normal_b);

            if perp_dot > 0.0 {
                vertex_data.extend::<&[f32; 2]>(point_a.as_ref());
                vertex_data.extend::<&[f32; 2]>((-normal_b).as_ref());
                vertex_data.extend::<&[f32; 2]>(point_a.as_ref());
                vertex_data.extend::<&[f32; 2]>((-normal_a + tangent_a).as_ref());
                vertex_data.extend::<&[f32; 2]>(point_a.as_ref());
                vertex_data.extend::<&[f32; 2]>((-normal_b + tangent_b).as_ref());
                vertex_data.extend::<&[f32; 2]>(point_a.as_ref());
                vertex_data.extend::<&[f32; 2]>((-normal_b + tangent_b).as_ref());
            } else if perp_dot < 0.0 {
                vertex_data.extend::<&[f32; 2]>(point_a.as_ref());
                vertex_data.extend::<&[f32; 2]>((normal_a + tangent_a).as_ref());
                vertex_data.extend::<&[f32; 2]>(point_a.as_ref());
                vertex_data.extend::<&[f32; 2]>(normal_b.as_ref());
                vertex_data.extend::<&[f32; 2]>(point_a.as_ref());
                vertex_data.extend::<&[f32; 2]>((normal_b + tangent_b).as_ref());
                vertex_data.extend::<&[f32; 2]>(point_a.as_ref());
                vertex_data.extend::<&[f32; 2]>((normal_b + tangent_b).as_ref());
            }
        }
    }

    fn add_line_segment(
        point_a: Point2<f32>,
        point_b: Point2<f32>,
        normal: Vector2<f32>,
        vertex_data: &mut Vec<f32>
    ) {
        vertex_data.extend::<&[f32; 2]>(point_a.as_ref());
        vertex_data.extend::<&[f32; 2]>(normal.as_ref());
        vertex_data.extend::<&[f32; 2]>(point_a.as_ref());
        vertex_data.extend::<&[f32; 2]>(normal.as_ref());
        vertex_data.extend::<&[f32; 2]>(point_a.as_ref());
        vertex_data.extend::<&[f32; 2]>((-normal).as_ref());
        vertex_data.extend::<&[f32; 2]>(point_b.as_ref());
        vertex_data.extend::<&[f32; 2]>(normal.as_ref());
        vertex_data.extend::<&[f32; 2]>(point_b.as_ref());
        vertex_data.extend::<&[f32; 2]>((-normal).as_ref());
    }

    fn add_double_cap(
        point: Point2<f32>,
        vertex_data: &mut Vec<f32>
    ) {
        let normal = Vector2::new(1.0, 0.0);
        let tangent = -Self::vec_ortho(normal);
        vertex_data.extend::<&[f32; 2]>(point.as_ref());
        vertex_data.extend::<&[f32; 2]>((normal + tangent).as_ref());
        vertex_data.extend::<&[f32; 2]>(point.as_ref());
        vertex_data.extend::<&[f32; 2]>((normal + tangent).as_ref());
        vertex_data.extend::<&[f32; 2]>(point.as_ref());
        vertex_data.extend::<&[f32; 2]>((-normal + tangent).as_ref());
        vertex_data.extend::<&[f32; 2]>(point.as_ref());
        vertex_data.extend::<&[f32; 2]>((normal - tangent).as_ref());
        vertex_data.extend::<&[f32; 2]>(point.as_ref());
        vertex_data.extend::<&[f32; 2]>((-normal - tangent).as_ref());
        vertex_data.extend::<&[f32; 2]>(point.as_ref());
        vertex_data.extend::<&[f32; 2]>((-normal - tangent).as_ref());
    }

    fn add_cap(
        point: Point2<f32>,
        normal: Vector2<f32>,
        vertex_data: &mut Vec<f32>
    ) {
        let tangent = -Self::vec_ortho(normal);
        vertex_data.extend::<&[f32; 2]>(point.as_ref());
        vertex_data.extend::<&[f32; 2]>((normal + tangent).as_ref());
        vertex_data.extend::<&[f32; 2]>(point.as_ref());
        vertex_data.extend::<&[f32; 2]>((-normal + tangent).as_ref());
        vertex_data.extend::<&[f32; 2]>(point.as_ref());
        vertex_data.extend::<&[f32; 2]>((-normal + tangent).as_ref());
    }

    fn add_separated_cap(
        point: Point2<f32>,
        normal: Vector2<f32>,
        vertex_data: &mut Vec<f32>
    ) {
        let tangent = Self::vec_ortho(normal);
        vertex_data.extend::<&[f32; 2]>(point.as_ref());
        vertex_data.extend::<&[f32; 2]>(normal.as_ref());
        vertex_data.extend::<&[f32; 2]>(point.as_ref());
        vertex_data.extend::<&[f32; 2]>(normal.as_ref());
        vertex_data.extend::<&[f32; 2]>(point.as_ref());
        vertex_data.extend::<&[f32; 2]>((-normal).as_ref());
        vertex_data.extend::<&[f32; 2]>(point.as_ref());
        vertex_data.extend::<&[f32; 2]>((normal + tangent).as_ref());
        vertex_data.extend::<&[f32; 2]>(point.as_ref());
        vertex_data.extend::<&[f32; 2]>((-normal + tangent).as_ref());
        vertex_data.extend::<&[f32; 2]>(point.as_ref());
        vertex_data.extend::<&[f32; 2]>((-normal + tangent).as_ref());
    }

    fn add_caps<F> (
        current_point: Option<Point2<f32>>,
        current_normal: Option<Vector2<f32>>,
        start_point: Option<Point2<f32>>,
        start_normal: Option<Vector2<f32>>,
        pixel_to_screen: F,
        vertex_data: &mut Vec<f32>,
    )
        where F: Fn(Point2<f32>) -> Point2<f32>,
    {
        if let Some(point_a) = current_point {
            let point_a: Point2<f32> = pixel_to_screen(point_a);

            if let Some(normal) = current_normal {
                Self::add_cap(point_a, normal, vertex_data);

                if let (Some(point), Some(normal)) = (start_point, start_normal) {
                    let point: Point2<f32> = pixel_to_screen(point);
                    Self::add_separated_cap(point, normal, vertex_data);
                }
            } else {
                Self::add_double_cap(point_a, vertex_data);
            }
        }
    }

    pub fn draw_mercator(
        &mut self,
        cx: &mut Context,
        merc: &MercatorView,
        dpi_factor: f64,
        snap_to_pixel: bool
    ) {
        let mut vertex_data: Vec<f32> = vec![];

        let scale_x = 2.0 / merc.viewport_size.x as f32;
        let scale_y = -2.0 / merc.viewport_size.y as f32;

        let half_width = 4.0 * dpi_factor as f32;

        let screen_mat: Matrix3<f32> = Matrix3::from_cols(
            vec3(scale_x, 0.0, 0.0),
            vec3(0.0, scale_y, 0.0),
            vec3(-1.0, 1.0, 1.0),
        );

        let map_to_screen = |mc: MapCoord| -> Point2<f32> {
            let mut sp = merc.map_to_screen_coord(mc);
            if snap_to_pixel {
                let topleft = merc.map_to_screen_coord(MapCoord::new(0.0, 0.0));
                let mut snapped = topleft;
                snapped.snap_to_pixel();

                sp.x += snapped.x - topleft.x;
                sp.y += snapped.y - topleft.y;
            }
            Point2::new(sp.x as f32, sp.y as f32)
        };

        let mut current_point: Option<Point2<f32>> = None;
        let mut current_normal: Option<Vector2<f32>> = None;
        let mut start_point: Option<Point2<f32>> = None;
        let mut start_normal: Option<Vector2<f32>> = None;

        for element in &self.path {
            match element {
                PathElement::MoveTo(mc) => {
                    Self::add_caps(
                        current_point,
                        current_normal,
                        start_point,
                        start_normal,
                        |p| screen_mat.transform_point(p),
                        &mut vertex_data,
                    );

                    current_point = Some(map_to_screen(*mc));
                    current_normal = None;
                    start_point = current_point;
                    start_normal = None;
                },
                PathElement::LineTo(mc) => {
                    let point_b = map_to_screen(*mc);
                    if let Some(point_a) = current_point {
                        let normal_b = Self::vec_ortho(point_b - point_a).normalize();
                        let point_a: Point2<f32> = screen_mat.transform_point(point_a);
                        let point_b: Point2<f32> = screen_mat.transform_point(point_b);

                        if let Some(normal_a) = current_normal {
                            Self::add_line_join(point_a, normal_a, normal_b, &mut vertex_data);
                        }

                        Self::add_line_segment(point_a, point_b, normal_b, &mut vertex_data);

                        current_normal = Some(normal_b);
                        start_normal = start_normal.or(Some(normal_b));
                    } else {
                        current_normal = None;
                        start_normal = None;
                    }
                    current_point = Some(point_b);
                    start_point = start_point.or(Some(point_b));
                },
                PathElement::ClosePath => {
                    if let Some(point_a) = current_point {
                        if let (Some(normal_a), Some(point_b), Some(normal_c)) = (current_normal, start_point, start_normal) {
                            let normal_b = Self::vec_ortho(point_b - point_a).normalize();
                            let point_a: Point2<f32> = screen_mat.transform_point(point_a);
                            let point_b: Point2<f32> = screen_mat.transform_point(point_b);

                            Self::add_line_join(point_a, normal_a, normal_b, &mut vertex_data);
                            Self::add_line_segment(point_a, point_b, normal_b, &mut vertex_data);
                            Self::add_line_join(point_b, normal_b, normal_c, &mut vertex_data);
                        } else {
                            let point_a: Point2<f32> = screen_mat.transform_point(point_a);
                            Self::add_double_cap(point_a, &mut vertex_data);
                        }
                    }

                    current_point = None;
                    current_normal = None;
                    start_point = None;
                    start_normal = None;
                },
            }
        }

        Self::add_caps(
            current_point,
            current_normal,
            start_point,
            start_normal,
            |p| screen_mat.transform_point(p),
            &mut vertex_data,
        );

        self.buffer.set_data(cx, &vertex_data, vertex_data.len() / 4);

        self.program.set_uniform_2f(cx, self.scale_uniform, scale_x, scale_y);
        self.program.set_uniform_1f(cx, self.half_width_uniform, half_width);
        self.program.set_uniform_3f(cx, self.color_uniform, 0.0, 0.0, 0.0);
        self.buffer.draw(cx, &self.program, DrawMode::TriangleStrip);

        self.program.set_uniform_1f(cx, self.half_width_uniform, half_width / 3.0);
        self.program.set_uniform_3f(cx, self.color_uniform, 1.0, 1.0, 1.0);
        self.buffer.draw(cx, &self.program, DrawMode::TriangleStrip);
    }


    pub fn draw_ortho(
        &mut self,
        cx: &mut Context,
        ortho: &OrthograficView,
        dpi_factor: f64,
    ) {
        //TODO implement
    }
}
