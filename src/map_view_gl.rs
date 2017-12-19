use ::context;
use ::std::ffi::CStr;
use buffer::{Buffer, DrawMode};
use context::Context;
use coord::ScreenCoord;
use image;
use map_view::MapView;
use program::Program;
use texture::{Texture, TextureFormat};
use tile_cache::TileCache;
use tile_cache_gl::TileCacheGl;
use tile_source::TileSource;


#[derive(Debug)]
pub struct MapViewGl<'a> {
    cx: &'a Context,
    program: Program<'a>,
    buf: Buffer<'a>,
    viewport_size: (u32, u32),
    map_view: MapView,
    tile_cache: TileCache,
    tile_cache_gl: TileCacheGl<'a>,
}

impl<'a> MapViewGl<'a> {
    pub fn new<F>(cx: &Context, initial_size: (u32, u32), update_func: F) -> MapViewGl
        where F: Fn() + Sync + Send + 'static,
    {
        println!("version: {}", cx.gl_version());
        println!("max texture size: {}", cx.max_texture_size());
        unsafe {
            let mut program = Program::from_paths(cx, "shader/map.vert", "shader/map.frag");

            check_gl_errors!(cx);
            let mut tex = Texture::empty(cx, 2048, 2048, TextureFormat::Rgb8);
            check_gl_errors!(cx);
            {
                let img = image::open("no_tile.png").unwrap();
                tex.sub_image(0, 0, &img);
                check_gl_errors!(cx);
            }

            let buf = Buffer::new(cx, &[], 0);

            check_gl_errors!(cx);

            program.add_texture(&tex, CStr::from_bytes_with_nul(b"tex_map\0").unwrap());
            check_gl_errors!(cx);

            program.add_attribute(CStr::from_bytes_with_nul(b"position\0").unwrap(), 2, 8, 0);
            check_gl_errors!(cx);
            program.add_attribute(CStr::from_bytes_with_nul(b"tex_coord\0").unwrap(), 2, 8, 2);
            check_gl_errors!(cx);
            program.add_attribute(CStr::from_bytes_with_nul(b"tex_minmax\0").unwrap(), 4, 8, 4);
            check_gl_errors!(cx);

            program.before_render();

            let tile_size = 256;
            let mut map_view = MapView::new(f64::from(initial_size.0), f64::from(initial_size.1), tile_size);

            // set initial zoom
            {
                let min_dimension = f64::from(initial_size.0.min(initial_size.1));
                let zoom = (min_dimension / f64::from(tile_size)).log2().ceil();
                map_view.set_zoom(zoom);
            }

            MapViewGl {
                cx: cx,
                program: program,
                buf: buf,
                viewport_size: initial_size,
                map_view: map_view,
                tile_cache: TileCache::new(move |_tile| update_func()),
                tile_cache_gl: TileCacheGl::new(tex, 256),
            }
        }
    }

    pub fn set_viewport_size(&mut self, width: u32, height: u32) {
        self.viewport_size = (width, height);
        self.map_view.set_size(f64::from(width), f64::from(height));
        unsafe {
            self.cx.gl.Viewport(
                0,
                0,
                width as context::gl::types::GLsizei,
                height as context::gl::types::GLsizei);
        }
    }

    pub fn draw(&mut self, source: &TileSource) {
        {
            let visible_tiles = self.map_view.visible_tiles(true);
            let textured_visible_tiles = self.tile_cache_gl.textured_visible_tiles(
                &visible_tiles,
                source,
                &mut self.tile_cache,
            );

            let mut vertex_data: Vec<f32> = Vec::with_capacity(textured_visible_tiles.len() * (6 * 8));
            let scale_x = 2.0 / f64::from(self.viewport_size.0);
            let scale_y = -2.0 / f64::from(self.viewport_size.1);
            for tvt in textured_visible_tiles {
                let minmax = [
                    tvt.tex_minmax.x1 as f32,
                    tvt.tex_minmax.y1 as f32,
                    tvt.tex_minmax.x2 as f32,
                    tvt.tex_minmax.y2 as f32,
                ];
                let p1 = [
                    (tvt.screen_rect.x * scale_x - 1.0) as f32,
                    (tvt.screen_rect.y * scale_y + 1.0) as f32,
                    tvt.tex_rect.x1 as f32,
                    tvt.tex_rect.y1 as f32,
                ];
                let p2 = [
                    (tvt.screen_rect.x * scale_x - 1.0) as f32,
                    ((tvt.screen_rect.y + tvt.screen_rect.height) * scale_y + 1.0) as f32,
                    tvt.tex_rect.x1 as f32,
                    tvt.tex_rect.y2 as f32,
                ];
                let p3 = [
                    ((tvt.screen_rect.x + tvt.screen_rect.width) * scale_x - 1.0) as f32,
                    ((tvt.screen_rect.y + tvt.screen_rect.height) * scale_y + 1.0) as f32,
                    tvt.tex_rect.x2 as f32,
                    tvt.tex_rect.y2 as f32,
                ];
                let p4 = [
                    ((tvt.screen_rect.x + tvt.screen_rect.width) * scale_x - 1.0) as f32,
                    (tvt.screen_rect.y * scale_y + 1.0) as f32,
                    tvt.tex_rect.x2 as f32,
                    tvt.tex_rect.y1 as f32,
                ];
                vertex_data.extend(&p1);
                vertex_data.extend(&minmax);
                vertex_data.extend(&p2);
                vertex_data.extend(&minmax);
                vertex_data.extend(&p3);
                vertex_data.extend(&minmax);
                vertex_data.extend(&p1);
                vertex_data.extend(&minmax);
                vertex_data.extend(&p3);
                vertex_data.extend(&minmax);
                vertex_data.extend(&p4);
                vertex_data.extend(&minmax);
            }

            self.buf.set_data(&vertex_data, vertex_data.len() / 4);
        }

        self.cx.clear_color((0.9, 0.9, 0.9, 1.0));
        self.buf.draw(DrawMode::Triangles);
    }

    pub fn zoom(&mut self, zoom_delta: f64) {
        if self.map_view.zoom2 + zoom_delta < 0.0 {
            self.map_view.set_zoom(0.0);
        } else if self.map_view.zoom2 + zoom_delta > 22.0 {
            self.map_view.set_zoom(22.0);
        } else {
            self.map_view.zoom(zoom_delta);
        }
    }

    pub fn zoom_at(&mut self, pos: ScreenCoord, zoom_delta: f64) {
        if self.map_view.zoom2 + zoom_delta < 0.0 {
            self.map_view.set_zoom_at(pos, 0.0);
        } else if self.map_view.zoom2 + zoom_delta > 22.0 {
            self.map_view.set_zoom_at(pos, 22.0);
        } else {
            self.map_view.zoom_at(pos, zoom_delta);
        }
        self.map_view.center.normalize_xy();
    }

    pub fn move_pixel(&mut self, delta_x: f64, delta_y: f64) {
        self.map_view.move_pixel(delta_x, delta_y);
        self.map_view.center.normalize_xy();
    }
}
