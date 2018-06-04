use ::context;
use context::Context;
use std::mem;


#[derive(Clone, Debug)]
pub struct Buffer {
    buffer_obj: u32,
    num_elements: usize,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum DrawMode {
    Triangles,
    TriangleStrip,
    TriangleFan,
}

impl DrawMode {
    pub fn to_gl_enum(self) -> u32 {
        match self {
            DrawMode::Triangles => context::gl::TRIANGLES,
            DrawMode::TriangleStrip => context::gl::TRIANGLE_STRIP,
            DrawMode::TriangleFan => context::gl::TRIANGLE_FAN,
        }
    }
}

impl Buffer {
    pub fn new(cx: &mut Context, vertex_data: &[f32], num_elements: usize) -> Buffer {
        let mut buffer_obj = 0_u32;

        unsafe {
            cx.gl.GenBuffers(1, &mut buffer_obj);
            cx.gl.BindBuffer(context::gl::ARRAY_BUFFER, buffer_obj);
            cx.gl.BufferData(context::gl::ARRAY_BUFFER,
                             (vertex_data.len() * mem::size_of::<f32>()) as context::gl::types::GLsizeiptr,
                             vertex_data.as_ptr() as *const _,
                             context::gl::STATIC_DRAW);
        }

        Buffer {
            buffer_obj,
            num_elements,
        }
    }

    pub fn set_data(&mut self, cx: &mut Context, vertex_data: &[f32], num_elements: usize) {
        unsafe {
            cx.gl.BufferData(context::gl::ARRAY_BUFFER,
                                  (vertex_data.len() * mem::size_of::<f32>()) as context::gl::types::GLsizeiptr,
                                  vertex_data.as_ptr() as *const _,
                                  context::gl::DYNAMIC_DRAW);
        }
        self.num_elements = num_elements;
    }

    pub fn bind(&self, cx: &mut Context) {
        unsafe {
            cx.gl.BindBuffer(context::gl::ARRAY_BUFFER, self.buffer_obj);
        }
    }

    pub fn draw(&self, cx: &mut Context, mode: DrawMode) {
        unsafe {
            cx.gl.DrawArrays(
                mode.to_gl_enum(),
                0,
                self.num_elements as context::gl::types::GLsizei);
        }
    }
}
