use ::context;
use context::Context;
use std::mem;


#[derive(Clone, Debug)]
pub struct Buffer<'a> {
    cx: &'a Context,
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

impl<'a> Buffer<'a> {
    pub fn new(cx: &'a Context, vertex_data: &[f32], num_elements: usize) -> Buffer<'a> {
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
            cx,
            buffer_obj,
            num_elements,
        }
    }

    pub fn set_data(&mut self, vertex_data: &[f32], num_elements: usize) {
        unsafe {
            self.cx.gl.BufferData(context::gl::ARRAY_BUFFER,
                                  (vertex_data.len() * mem::size_of::<f32>()) as context::gl::types::GLsizeiptr,
                                  vertex_data.as_ptr() as *const _,
                                  context::gl::DYNAMIC_DRAW);
        }
        self.num_elements = num_elements;
    }

    pub fn bind(&self) {
        unsafe {
            self.cx.gl.BindBuffer(context::gl::ARRAY_BUFFER, self.buffer_obj);
        }
    }

    pub fn draw(&self, mode: DrawMode) {
        unsafe {
            self.cx.gl.DrawArrays(
                mode.to_gl_enum(),
                0,
                self.num_elements as context::gl::types::GLsizei);
        }
    }
}
