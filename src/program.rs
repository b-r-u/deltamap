use ::context;
use buffer::Buffer;
use context::Context;
use std::ffi::CStr;
use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::mem;
use std::path::Path;
use texture::Texture;
use vertex_attrib::{VertexAttribLoc, VertexAttribParams};


#[derive(Clone, Debug)]
pub struct Program {
    vert_obj: u32,
    frag_obj: u32,
    program_id: ProgramId,
    attrib_locs: Vec<VertexAttribLoc>,
    attrib_params: Vec<VertexAttribParams>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ProgramId {
    id: u32,
}

impl ProgramId {
    /// Returns an invalid `ProgramId`.
    pub fn invalid() -> Self {
        ProgramId{ id: 0 }
    }

    pub fn index(&self) -> u32 {
        self.id
    }
}

impl Program {
    pub fn from_paths<P: AsRef<Path>>(
        cx: &mut Context,
        vert_path: P,
        frag_path: P,
    ) -> Result<Program, String> {
        let vert_src = {
            let file = File::open(&vert_path)
                .map_err(|e| format!("{}", e))?;
            let mut reader = BufReader::new(file);
            let mut buf: Vec<u8> = vec![];
            reader.read_to_end(&mut buf)
                .map_err(|e| format!("{}", e))?;
            buf
        };

        let frag_src = {
            let file = File::open(&frag_path)
                .map_err(|e| format!("{}", e))?;
            let mut reader = BufReader::new(file);
            let mut buf: Vec<u8> = vec![];
            reader.read_to_end(&mut buf)
                .map_err(|e| format!("{}", e))?;
            buf
        };

        Self::new(cx, &vert_src, &frag_src)
    }

    pub fn new(
        cx: &mut Context,
        vert_src: &[u8],
        frag_src: &[u8],
    ) -> Result<Program, String> {
        unsafe {
            let vert_obj = {
                let vert_obj = cx.gl.CreateShader(context::gl::VERTEX_SHADER);
                let vert_len = vert_src.len() as i32;
                cx.gl.ShaderSource(
                    vert_obj,
                    1,
                    [vert_src.as_ptr() as *const _].as_ptr(),
                    &vert_len as *const _);
                cx.gl.CompileShader(vert_obj);
                check_compile_errors(cx, vert_obj)?;
                check_gl_errors!(cx);
                vert_obj
            };

            let frag_obj = {
                let frag_obj = cx.gl.CreateShader(context::gl::FRAGMENT_SHADER);
                let frag_len = frag_src.len() as i32;
                cx.gl.ShaderSource(
                    frag_obj,
                    1,
                    [frag_src.as_ptr() as *const _].as_ptr(),
                    &frag_len as *const _);
                cx.gl.CompileShader(frag_obj);
                check_compile_errors(cx, frag_obj)?;
                check_gl_errors!(cx);
                frag_obj
            };

            let program_id = {
                let prog = cx.gl.CreateProgram();
                cx.gl.AttachShader(prog, vert_obj);
                cx.gl.AttachShader(prog, frag_obj);
                cx.gl.LinkProgram(prog);
                check_link_errors(cx, prog)?;

                ProgramId { id: prog }
            };

            cx.use_program(program_id);
            check_gl_errors!(cx);

            Ok(
                Program {
                    vert_obj,
                    frag_obj,
                    program_id,
                    attrib_locs: vec![],
                    attrib_params: vec![],
                }
            )
        }
    }

    pub fn add_texture(&mut self, cx: &mut Context, texture: &Texture, uniform_name: &CStr) {
        //TODO store reference to texture
        cx.use_program(self.program_id);
        unsafe {
            let tex_loc = cx.gl.GetUniformLocation(self.program_id.index(), uniform_name.as_ptr() as *const _);
            check_gl_errors!(cx);

            cx.gl.Uniform1i(tex_loc, texture.unit().index() as i32);
        }
    }

    //TODO rename function or integrate into new()
    pub fn add_attribute(
        &mut self,
        cx: &mut Context,
        name: &CStr,
        params: &VertexAttribParams,
    ) {
        cx.use_program(self.program_id);

        let attrib_loc = unsafe {
            cx.gl.GetAttribLocation(self.program_id.index(), name.as_ptr() as *const _)
        };
        if attrib_loc < 0 {
            panic!("Attribute location not found: {:?}", name);
        }
        let attrib_loc = VertexAttribLoc::new(attrib_loc as u32);
        check_gl_errors!(cx);

        self.attrib_locs.push(attrib_loc);
        self.attrib_params.push(params.clone());
    }

    pub fn enable_vertex_attribs(&self, cx: &mut Context) {
        cx.enable_vertex_attribs(&self.attrib_locs)
    }

    //TODO use separate buffer for each attribute
    pub fn set_vertex_attribs(
        &self,
        cx: &mut Context,
        buffer: &Buffer,
    ) {
        cx.bind_buffer(buffer.id());
        for (params, loc) in self.attrib_params.iter().zip(self.attrib_locs.iter()) {
            params.set(cx, *loc);
        }
    }

    pub fn id(&self) -> ProgramId {
        self.program_id
    }
}

fn check_link_errors(cx: &Context, program_obj: u32) -> Result<(), String> {
    unsafe {
        let mut link_success: i32 = mem::uninitialized();

        cx.gl.GetProgramiv(program_obj, context::gl::LINK_STATUS, &mut link_success);

        if link_success == 0 {
            let mut error_log_size: i32 = mem::uninitialized();
            cx.gl.GetProgramiv(program_obj, context::gl::INFO_LOG_LENGTH, &mut error_log_size);

            let mut error_log: Vec<u8> = Vec::with_capacity(error_log_size as usize);
            cx.gl.GetProgramInfoLog(program_obj, error_log_size, &mut error_log_size,
                                    error_log.as_mut_ptr() as *mut context::gl::types::GLchar);

            error_log.set_len(error_log_size as usize);

            Err(String::from_utf8_lossy(&error_log).into())
        } else {
            Ok(())
        }
    }
}

fn check_compile_errors(cx: &Context, shader_obj: u32) -> Result<(), String> {
    unsafe {
        // checking compilation success by reading a flag on the shader
        let compilation_success = {
            let mut compilation_success: i32 = mem::uninitialized();
            cx.gl.GetShaderiv(shader_obj, context::gl::COMPILE_STATUS, &mut compilation_success);
            compilation_success
        };

        if compilation_success != 1 {
            // compilation error
            let mut error_log_size: i32 = mem::uninitialized();
            cx.gl.GetShaderiv(shader_obj, context::gl::INFO_LOG_LENGTH, &mut error_log_size);
            let mut error_log: Vec<u8> = Vec::with_capacity(error_log_size as usize);

            cx.gl.GetShaderInfoLog(shader_obj, error_log_size, &mut error_log_size,
                                     error_log.as_mut_ptr() as *mut _);
            error_log.set_len(error_log_size as usize);

            Err(String::from_utf8_lossy(&error_log).into())
        } else {
            Ok(())
        }
    }
}
