use glutin;

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
            let data = ::std::ffi::CStr::from_ptr(self.gl.GetString(gl::VERSION) as *const _).to_bytes().to_vec();
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
    pub fn from_window(window: &glutin::Window) -> Context {
        let gl = gl::Gl::load_with(|ptr| window.get_proc_address(ptr) as *const _);
        let cx = Context {gl: gl};

        info!("OpenGL version: {}", cx.gl_version());
        debug!("MAX_TEXTURE_SIZE: {}", cx.max_texture_size());

        cx
    }

    pub fn gl_version(&self) -> String {
        unsafe {
            let data = ::std::ffi::CStr::from_ptr(self.gl.GetString(gl::VERSION) as *const _).to_bytes().to_vec();
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

    pub unsafe fn check_errors(&self, file: &str, line: u32) {
        loop {
            match self.gl.GetError() {
                gl::NO_ERROR => break,
                gl::INVALID_VALUE => {
                    error!("{}:{}, invalid value error", file, line);
                },
                gl::INVALID_ENUM => {
                    error!("{}:{}, invalid enum error", file, line);
                },
                gl::INVALID_OPERATION => {
                    error!("{}:{}, invalid operation error", file, line);
                },
                gl::INVALID_FRAMEBUFFER_OPERATION => {
                    error!("{}:{}, invalid framebuffer operation error", file, line);
                },
                gl::OUT_OF_MEMORY => {
                    error!("{}:{}, out of memory error", file, line);
                },
                x => {
                    error!("{}:{}, unknown error {}", file, line, x);
                },
            }
        }
    }

    pub fn clear_color(&self, color: (f32, f32, f32, f32)) {
        unsafe {
            self.gl.ClearColor(color.0, color.1, color.2, color.3);
            self.gl.Clear(gl::COLOR_BUFFER_BIT);
        }
    }
}
