use ::context;
use context::Context;
use std::mem;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct VertexAttribLoc(u32);

impl VertexAttribLoc {
    pub fn new(location: u32) -> VertexAttribLoc {
        VertexAttribLoc(location)
    }

    pub fn index(&self) -> u32 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct VertexAttribParams {
    pub number_components: u32,
    pub stride: usize,
    pub offset: usize,
}

impl VertexAttribParams {
    pub fn new(
        number_components: u32,
        stride: usize,
        offset: usize,
    ) -> VertexAttribParams {
        VertexAttribParams {
            number_components,
            stride,
            offset,
        }
    }

    pub fn set(&self, cx: &mut Context, loc: VertexAttribLoc) {
        unsafe {
            cx.gl.VertexAttribPointer(
                loc.0,
                self.number_components as i32, // size
                context::gl::FLOAT, // type
                0, // normalized
                (self.stride * mem::size_of::<f32>()) as context::gl::types::GLsizei,
                (self.offset * mem::size_of::<f32>()) as *const () as *const _,
            );
        }
    }
}
