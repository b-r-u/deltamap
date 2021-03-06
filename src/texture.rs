use ::context;
use ::image;
use context::{Context, TextureUnit};
use image::GenericImageView;
use std::os::raw::c_void;

#[derive(Clone, Debug)]
pub struct Texture {
    texture_obj: u32,
    texture_unit: TextureUnit,
    width: u32,
    height: u32,
    format: TextureFormat,
}

#[derive(Clone, Debug)]
pub struct TextureId {
    pub(crate) id: u32,
}

impl Texture {
    pub fn new(cx: &mut Context, img: &image::DynamicImage) -> Result<Texture, ()> {
        let format = match *img {
            image::ImageRgb8(_) => TextureFormat::Rgb8,
            image::ImageRgba8(_) => TextureFormat::Rgba8,
            _ => return Err(()),
        };

        Ok(Self::from_bytes(cx, img.width(), img.height(), format, &img.raw_pixels()))
    }

    pub fn empty(cx: &mut Context, width: u32, height: u32, format: TextureFormat) -> Texture {
        Self::from_ptr(cx, width, height, format, ::std::ptr::null() as *const _)
    }

    pub fn from_bytes(cx: &mut Context, width: u32, height: u32, format: TextureFormat, data: &[u8]) -> Texture {
        Self::from_ptr(cx, width, height, format, data.as_ptr() as *const _)
    }

    fn from_ptr(cx: &mut Context, width: u32, height: u32, format: TextureFormat, data_ptr: *const c_void) -> Texture {
        let mut texture_obj = 0_u32;

        let texture_unit = cx.occupy_free_texture_unit();
        cx.set_active_texture_unit(texture_unit);

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
            texture_obj,
            texture_unit,
            width,
            height,
            format,
        }
    }

    pub fn sub_image(&mut self, cx: &mut Context, x: i32, y: i32, img: &image::DynamicImage) {
        let format = match *img {
            image::ImageRgb8(_) => TextureFormat::Rgb8,
            image::ImageRgba8(_) => TextureFormat::Rgba8,
            _ => return,
        };

        cx.set_active_texture_unit(self.texture_unit);
        unsafe {
            cx.gl.TexSubImage2D(
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

    pub fn resize(&mut self, cx: &mut Context, width: u32, height: u32) {
        cx.set_active_texture_unit(self.texture_unit);
        unsafe {
            cx.gl.TexImage2D(
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

    pub fn unit(&self) -> TextureUnit {
        self.texture_unit
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
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
