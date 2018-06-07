use buffer::BufferId;
use glutin::GlContext;
use glutin;
use program::ProgramId;
use std::collections::HashSet;
use std::ffi::CStr;
use std::mem;
use vertex_attrib::VertexAttribLoc;

pub(crate) mod gl {
    #![allow(unknown_lints)]
    #![allow(clippy)]
    pub use self::Gles2 as Gl;
    include!(concat!(env!("OUT_DIR"), "/gles_bindings.rs"));
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextureUnit(u32);

impl TextureUnit {
    pub fn index(&self) -> u32 {
        self.0
    }
}

#[derive(Clone)]
pub struct Context {
    pub(crate) gl: gl::Gl,
    active_texture_unit: TextureUnit,
    next_free_texture_unit: TextureUnit,
    active_program: ProgramId,
    active_buffer: BufferId,
    active_attribs: HashSet<VertexAttribLoc>,
}

impl ::std::fmt::Debug for Context {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        let version = unsafe {
            let data = CStr::from_ptr(self.gl.GetString(gl::VERSION) as *const _).to_bytes().to_vec();
            String::from_utf8(data).unwrap_or_else(|_| "".into())
        };
        write!(f, "Context {{ version: {:?} }}", version)
    }
}

macro_rules! check_gl_errors {
    ($cx:expr) => (
        $cx.check_errors(file!(), line!());
    )
}

impl Context {
    pub fn from_gl_window(window: &glutin::GlWindow) -> Context {
        let gl = gl::Gl::load_with(|ptr| window.get_proc_address(ptr) as *const _);
        let cx = Context {
            gl,
            /// Initial active texture unit is supposed to be GL_TEXTURE0
            active_texture_unit: TextureUnit(0),
            next_free_texture_unit: TextureUnit(0),
            active_program: ProgramId::invalid(),
            active_buffer: BufferId::invalid(),
            active_attribs: HashSet::new(),
        };

        // Initialize a vertex array object (VAO) if the current OpenGL context supports it. VAOs are
        // not OpenGL ES 2.0 compatible, but are required for rendering with a core context.
        if cx.gl.BindVertexArray.is_loaded() {
            unsafe {
                let mut vao = mem::uninitialized();
                cx.gl.GenVertexArrays(1, &mut vao);
                cx.gl.BindVertexArray(vao);
            }
        }

        info!("OpenGL version: {}", cx.gl_version());
        debug!("MAX_TEXTURE_SIZE: {}", cx.max_texture_size());

        cx
    }

    pub fn gl_version(&self) -> String {
        unsafe {
            let data = CStr::from_ptr(self.gl.GetString(gl::VERSION) as *const _).to_bytes().to_vec();
            String::from_utf8(data).unwrap_or_else(|_| "".into())
        }
    }

    pub fn max_texture_size(&self) -> i32 {
        unsafe {
            let mut size = 0;
            self.gl.GetIntegerv(gl::MAX_TEXTURE_SIZE, &mut size as *mut _);
            size
        }
    }

    pub fn check_errors(&self, file: &str, line: u32) {
        let mut fail = false;

        loop {
            match unsafe { self.gl.GetError() } {
                gl::NO_ERROR => break,
                gl::INVALID_VALUE => {
                    error!("{}:{}, invalid value error", file, line);
                    fail = true;
                },
                gl::INVALID_ENUM => {
                    error!("{}:{}, invalid enum error", file, line);
                    fail = true;
                },
                gl::INVALID_OPERATION => {
                    error!("{}:{}, invalid operation error", file, line);
                    fail = true;
                },
                gl::INVALID_FRAMEBUFFER_OPERATION => {
                    error!("{}:{}, invalid framebuffer operation error", file, line);
                    fail = true;
                },
                gl::OUT_OF_MEMORY => {
                    error!("{}:{}, out of memory error", file, line);
                    fail = true;
                },
                x => {
                    error!("{}:{}, unknown error {}", file, line, x);
                    fail = true;
                },
            }
        }

        if fail {
            panic!("OpenGL error");
        }
    }

    pub fn clear_color(&self, color: (f32, f32, f32, f32)) {
        unsafe {
            self.gl.ClearColor(color.0, color.1, color.2, color.3);
            self.gl.Clear(gl::COLOR_BUFFER_BIT);
        }
    }

    pub fn set_viewport(&self, x: i32, y: i32, width: u32, height: u32) {
        unsafe {
            self.gl.Viewport(
                x,
                y,
                width as gl::types::GLsizei,
                height as gl::types::GLsizei,
            );
        }
    }

    pub fn set_active_texture_unit(&mut self, unit: TextureUnit) {
        if unit != self.active_texture_unit {
            unsafe {
                self.gl.ActiveTexture(gl::TEXTURE0 + unit.0);
            }
            self.active_texture_unit = unit;
        }
    }

    pub fn occupy_free_texture_unit(&mut self) -> TextureUnit {
        let tu = self.next_free_texture_unit;

        //TODO check against max number of texture units
        //TODO add a way to free texture units
        self.next_free_texture_unit = TextureUnit(self.next_free_texture_unit.0 + 1);

        tu
    }

    pub fn use_program(&mut self, prog: ProgramId) {
        if prog != self.active_program {
            unsafe {
                self.gl.UseProgram(prog.index());
            }
            self.active_program = prog;
        }
    }

    pub fn bind_buffer(&mut self, buf: BufferId) {
        if buf != self.active_buffer {
            unsafe {
                self.gl.BindBuffer(gl::ARRAY_BUFFER, buf.index());
            }
            self.active_buffer = buf;
        }
    }

    /// Enable all vertex attributes given by their location and disable all other vertex
    /// attributes.
    //TODO group attribs by program
    pub fn enable_vertex_attribs(&mut self, attribs: &[VertexAttribLoc]) {
        let new_set: HashSet<_> = attribs.iter().cloned().collect();

        unsafe {
            for old_attrib in self.active_attribs.difference(&new_set) {
                self.gl.DisableVertexAttribArray(old_attrib.index());
            }

            for new_attrib in new_set.difference(&self.active_attribs) {
                self.gl.EnableVertexAttribArray(new_attrib.index());
            }
        }

        self.active_attribs = new_set;
    }

    pub fn enable_vertex_attrib(&mut self, attrib: VertexAttribLoc) {
        if !self.active_attribs.contains(&attrib) {
            unsafe {
                self.gl.EnableVertexAttribArray(attrib.index());
            }
            self.active_attribs.insert(attrib);
        }
    }

    /// Print status of vector attributes.
    pub fn debug_attribs(&self) {
        unsafe {
            let mut max_attribs = 0i32;
            self.gl.GetIntegerv(gl::MAX_VERTEX_ATTRIBS, &mut max_attribs as *mut _);

            for index in 0..(max_attribs as u32) {
                let mut enabled = 0i32;
                self.gl.GetVertexAttribiv(index, gl::VERTEX_ATTRIB_ARRAY_ENABLED, &mut enabled as *mut _);
                let enabled: bool = enabled != 0;
                println!("attribute {} enabled: {}", index, enabled);
            }
        }
    }
}
