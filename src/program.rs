use ::context;
use context::Context;
use std::ffi::CStr;
use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::mem;
use std::path::Path;
use texture::{Texture, TextureId};

#[derive(Clone, Debug)]
pub struct Program<'a> {
    cx: &'a ::context::Context,
    vert_obj: u32,
    frag_obj: u32,
    program_obj: u32,
    tex_ids: Vec<TextureId>,
    tex_locations: Vec<i32>,
}

#[derive(Clone, Debug)]
pub struct ProgramId {
    id: u32,
}

impl<'a> Program<'a> {
    pub unsafe fn from_paths<P: AsRef<Path>>(cx: &'a Context, vert_path: P, frag_path: P) -> Program<'a> {
        let vert_src = {
            let file = File::open(&vert_path).unwrap();
            let mut reader = BufReader::new(file);
            let mut buf: Vec<u8> = vec![];
            reader.read_to_end(&mut buf).unwrap();
            buf
        };

        let frag_src = {
            let file = File::open(&frag_path).unwrap();
            let mut reader = BufReader::new(file);
            let mut buf: Vec<u8> = vec![];
            reader.read_to_end(&mut buf).unwrap();
            buf
        };

        Self::new(cx, &vert_src, &frag_src)
    }

    pub unsafe fn new(cx: &'a Context, vert_src: &[u8], frag_src: &[u8]) -> Program<'a> {
        let vert_obj = {
            let vert_obj = cx.gl.CreateShader(context::gl::VERTEX_SHADER);
            let vert_len = vert_src.len() as i32;
            cx.gl.ShaderSource(
                vert_obj,
                1,
                [vert_src.as_ptr() as *const _].as_ptr(),
                &vert_len as *const _);
            cx.gl.CompileShader(vert_obj);
            check_compile_errors(cx, vert_obj);
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
            check_compile_errors(cx, frag_obj);
            check_gl_errors!(cx);
            frag_obj
        };

        let program_obj = {
            let prog = cx.gl.CreateProgram();
            cx.gl.AttachShader(prog, vert_obj);
            cx.gl.AttachShader(prog, frag_obj);
            cx.gl.LinkProgram(prog);
            check_link_errors(cx, prog);

            cx.gl.UseProgram(prog);
            check_gl_errors!(cx);
            prog
        };

        Program {
            cx: cx,
            vert_obj: vert_obj,
            frag_obj: frag_obj,
            program_obj: program_obj,
            tex_ids: vec![],
            tex_locations: vec![],
        }
    }

    pub unsafe fn add_texture(&mut self, texture: &Texture, uniform_name: &CStr) {
        //TODO store reference to texture
        let tex_loc = self.cx.gl.GetUniformLocation(self.program_obj, uniform_name.as_ptr() as *const _);
        check_gl_errors!(self.cx);

        self.tex_ids.push(texture.id());
        self.tex_locations.push(tex_loc);

    }

    pub unsafe fn add_attribute(&mut self, name: &CStr, number_components: u32, stride: usize, offset: usize) {
        let attrib_id = self.cx.gl.GetAttribLocation(self.program_obj, name.as_ptr() as *const _);
        check_gl_errors!(self.cx);
        self.cx.gl.VertexAttribPointer(
            attrib_id as u32,
            number_components as i32, // size
            context::gl::FLOAT, // type
            0, // normalized
            (stride * mem::size_of::<f32>()) as context::gl::types::GLsizei,
            (offset * mem::size_of::<f32>()) as *const () as *const _);
        check_gl_errors!(self.cx);
        self.cx.gl.EnableVertexAttribArray(attrib_id as u32);
        check_gl_errors!(self.cx);
    }

    pub unsafe fn before_render(&self) {
        check_gl_errors!(self.cx);
        //self.cx.gl.UseProgram(self.program_obj);
        //TODO check max texture number
        for (i, (tex_id, &tex_loc)) in self.tex_ids.iter().zip(&self.tex_locations).enumerate() {
            self.cx.gl.ActiveTexture(context::gl::TEXTURE0 + i as u32);
            self.cx.gl.BindTexture(context::gl::TEXTURE_2D, tex_id.id);
            self.cx.gl.Uniform1i(tex_loc, i as i32);
        }
    }

    pub fn id(&self) -> ProgramId {
        ProgramId {
            id: self.program_obj,
        }
    }
}

unsafe fn check_link_errors(cx: &Context, program_obj: u32)
{
    let mut link_success: i32 = mem::uninitialized();

    cx.gl.GetProgramiv(program_obj, context::gl::LINK_STATUS, &mut link_success);

    if link_success == 0 {

        match cx.gl.GetError() {
            context::gl::NO_ERROR => (),
            context::gl::INVALID_VALUE => {
                println!("invalid value");
                return;
            },
            context::gl::INVALID_OPERATION => {
                println!("invalid operation");
                return;
            },
            _ => {
                println!("unknown error");
                return;
            }
        };

        let mut error_log_size: i32 = mem::uninitialized();

        cx.gl.GetProgramiv(program_obj, context::gl::INFO_LOG_LENGTH, &mut error_log_size);

        let mut error_log: Vec<u8> = Vec::with_capacity(error_log_size as usize);

        cx.gl.GetProgramInfoLog(program_obj, error_log_size, &mut error_log_size,
                             error_log.as_mut_ptr() as *mut context::gl::types::GLchar);

        error_log.set_len(error_log_size as usize);

        let msg = String::from_utf8(error_log).unwrap();
        println!("{}", msg);
    }
}

unsafe fn check_compile_errors(cx: &Context, shader_obj: u32) {
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

        if let Ok(msg) = String::from_utf8(error_log) {
            println!("{}", msg);
        }
    }
}
