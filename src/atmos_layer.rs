use ::std::ffi::CStr;
use buffer::{Buffer, DrawMode};
use context::Context;
use map_view::MapView;
use orthografic_view::OrthograficView;
use program::{Program, UniformId};
use std::f32::consts::PI;
use vertex_attrib::VertexAttribParams;


#[derive(Debug)]
pub struct AtmosLayer {
    buffer: Buffer,
    program: Program,
    scale_uniform: UniformId,
}

impl AtmosLayer {
    pub fn new(cx: &mut Context) -> AtmosLayer {
        let vertex_data = {
            let mut vertex_data: Vec<f32> = Vec::with_capacity(17 * 4);

            let radius_a = 0.75;
            let radius_b = 1.25;
            for x in 0..17 {
                let angle = x as f32 * (PI * 2.0 / 16.0);
                vertex_data.extend(&[
                    angle.cos() * radius_a,
                    angle.sin() * radius_a,
                    angle.cos() * radius_b,
                    angle.sin() * radius_b,
                ]);
            }

            vertex_data
        };

        let buffer = Buffer::new(cx, &vertex_data, vertex_data.len() / 2);

        let mut program = Program::new(
            cx,
            include_bytes!("../shader/atmos.vert"),
            include_bytes!("../shader/atmos.frag"),
        ).unwrap();

        program.add_attribute(
            cx,
            CStr::from_bytes_with_nul(b"position\0").unwrap(),
            &VertexAttribParams::new(2, 2, 0)
        );

        let scale_uniform = program.get_uniform_id(cx, CStr::from_bytes_with_nul(b"scale\0").unwrap()).unwrap();

        check_gl_errors!(cx);

        AtmosLayer {
            buffer,
            program,
            scale_uniform,
        }
    }

    // Has to be called once before one or multiple calls to `draw`.
    pub fn prepare_draw(&mut self, cx: &mut Context) {
        self.program.enable_vertex_attribs(cx);
        self.program.set_vertex_attribs(cx, &self.buffer);
    }

    pub fn draw(
        &mut self,
        cx: &mut Context,
        map_view: &MapView,
    ) {
        let (scale_x, scale_y) = {
            let diam = OrthograficView::diameter_physical_pixels(map_view);
            ((diam / map_view.width) as f32, (diam / map_view.height) as f32)
        };

        self.program.set_uniform_2f(cx, self.scale_uniform, scale_x, scale_y);

        self.buffer.draw(cx, &self.program, DrawMode::TriangleStrip);
    }
}
