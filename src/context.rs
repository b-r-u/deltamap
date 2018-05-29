use glutin;
use glutin::GlContext;
use std::mem;
use std::ffi::CStr;

pub(crate) mod gl {
    #![allow(unknown_lints)]
    #![allow(clippy)]
    pub use self::Gles2 as Gl;
    include!(concat!(env!("OUT_DIR"), "/gles_bindings.rs"));
}

#[derive(Clone)]
pub struct Context {
    pub(crate) gl: gl::Gl,
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
        let cx = Context { gl };

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
}
