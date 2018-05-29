use ::context;
use ::image;
use context::Context;
use image::GenericImage;
use std::os::raw::c_void;

#[derive(Clone, Debug)]
pub struct Texture<'a> {
    cx: &'a Context,
    texture_obj: u32,
    width: u32,
    height: u32,
    format: TextureFormat,
}

#[derive(Clone, Debug)]
pub struct TextureId {
    pub(crate) id: u32,
}

impl<'a> Texture<'a> {
    pub fn new(cx: &'a Context, img: &image::DynamicImage) -> Result<Texture<'a>, ()> {
        let format = match *img {
            image::ImageRgb8(_) => TextureFormat::Rgb8,
            image::ImageRgba8(_) => TextureFormat::Rgba8,
            _ => return Err(()),
        };

        Ok(Self::from_bytes(cx, img.width(), img.height(), format, &img.raw_pixels()))
    }

    pub fn empty(cx: &'a Context, width: u32, height: u32, format: TextureFormat) -> Texture<'a> {
        Self::from_ptr(cx, width, height, format, ::std::ptr::null() as *const _)
    }

    pub fn from_bytes(cx: &'a Context, width: u32, height: u32, format: TextureFormat, data: &[u8]) -> Texture<'a> {
        Self::from_ptr(cx, width, height, format, data.as_ptr() as *const _)
    }

    fn from_ptr(cx: &'a Context, width: u32, height: u32, format: TextureFormat, data_ptr: *const c_void) -> Texture<'a> {
        let mut texture_obj = 0_u32;
        unsafe {
            cx.gl.GenTextures(1, &mut texture_obj);
            cx.gl.BindTexture(context::gl::TEXTURE_2D, texture_obj);

            cx.gl.TexParameteri(context::gl::TEXTURE_2D, context::gl::TEXTURE_MIN_FILTER, context::gl::LINEAR as i32);
            cx.gl.TexParameteri(context::gl::TEXTURE_2D, context::gl::TEXTURE_MAG_FILTER, context::gl::LINEAR as i32);
            cx.gl.TexParameteri(context::gl::TEXTURE_2D, context::gl::TEXTURE_WRAP_S, context::gl::CLAMP_TO_EDGE as i32);
            cx.gl.TexParameteri(context::gl::TEXTURE_2D, context::gl::TEXTURE_WRAP_T, context::gl::CLAMP_TO_EDGE as i32);

            cx.gl.TexImage2D(
                context::gl::TEXTURE_2D,
                0, // level
                format.to_gl_enum() as i32,
                width as i32,
                height as i32,
                0, // border (must be zero)
                format.to_gl_enum(),
                context::gl::UNSIGNED_BYTE,
                data_ptr);
        }

        Texture {
            cx,
            texture_obj,
            width,
            height,
            format,
        }
    }

    pub fn sub_image(&mut self, x: i32, y: i32, img: &image::DynamicImage) {
        let format = match *img {
            image::ImageRgb8(_) => TextureFormat::Rgb8,
            image::ImageRgba8(_) => TextureFormat::Rgba8,
            _ => return,
        };

        unsafe {
            self.cx.gl.BindTexture(context::gl::TEXTURE_2D, self.texture_obj);
            self.cx.gl.TexSubImage2D(
                context::gl::TEXTURE_2D,
                0, // level
                x, // x offset
                y, // y offset
                img.width() as i32,
                img.height() as i32,
                format.to_gl_enum(),
                context::gl::UNSIGNED_BYTE,
                img.raw_pixels().as_ptr() as *const _,
            );
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        unsafe {
            self.cx.gl.BindTexture(context::gl::TEXTURE_2D, self.texture_obj);
            self.cx.gl.TexImage2D(
                context::gl::TEXTURE_2D,
                0, // level
                self.format.to_gl_enum() as i32,
                width as i32,
                height as i32,
                0, // border (must be zero)
                self.format.to_gl_enum(),
                context::gl::UNSIGNED_BYTE,
                ::std::ptr::null() as *const _);

            self.width = width;
            self.height = height;
        }
    }

    pub fn id(&self) -> TextureId {
        TextureId {
            id: self.texture_obj,
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn context(&self) -> &Context {
        self.cx
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum TextureFormat {
    Rgb8,
    Rgba8,
}

impl TextureFormat {
    pub fn to_gl_enum(self) -> u32 {
        match self {
            TextureFormat::Rgb8 => context::gl::RGB,
            TextureFormat::Rgba8 => context::gl::RGBA,
        }
    }
}
