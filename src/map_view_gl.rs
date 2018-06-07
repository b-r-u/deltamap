use ::std::ffi::CStr;
use buffer::{Buffer, DrawMode};
use context::Context;
use coord::{ScreenCoord, View};
use map_view::MapView;
use program::Program;
use texture::{Texture, TextureFormat};
use tile_atlas::TileAtlas;
use tile_cache::TileCache;
use tile_source::TileSource;
use vertex_attrib::VertexAttribParams;


const MIN_ZOOM_LEVEL: f64 = 0.0;
const MAX_ZOOM_LEVEL: f64 = 22.0;

#[derive(Debug)]
pub struct MapViewGl {
    map_view: MapView,
    viewport_size: (u32, u32),
    tile_program: Program,
    tile_buffer: Buffer,
    tile_cache: TileCache,
    tile_atlas: TileAtlas,
}

impl MapViewGl {
    pub fn new<F>(
        cx: &mut Context,
        initial_size: (u32, u32),
        update_func: F,
        use_network: bool,
        use_async: bool,
        ) -> MapViewGl
        where F: Fn() + Sync + Send + 'static,
    {
        let tile_size = 256;

        let mut map_view = MapView::with_filling_zoom(f64::from(initial_size.0), f64::from(initial_size.1), tile_size);

        if map_view.zoom < MIN_ZOOM_LEVEL {
            map_view.zoom = MIN_ZOOM_LEVEL;
        }

        let tile_buffer = Buffer::new(cx, &[], 0);
        check_gl_errors!(cx);
        cx.bind_buffer(tile_buffer.id());

        let mut tile_program = Program::new(
            cx,
            include_bytes!("../shader/map.vert"),
            include_bytes!("../shader/map.frag"),
        ).unwrap();
        check_gl_errors!(cx);

        let atlas_size = {
            let default_size = 2048;
            let max_size = cx.max_texture_size() as u32;
            if default_size <= max_size {
                default_size
            } else {
                if tile_size * 3 > max_size {
                    error!("maximal tile size ({}) is too small", max_size);
                }

                max_size
            }
        };

        let atlas_tex = Texture::empty(cx, atlas_size, atlas_size, TextureFormat::Rgb8);
        check_gl_errors!(cx);

        tile_program.add_texture(cx, &atlas_tex, CStr::from_bytes_with_nul(b"tex_map\0").unwrap());
        check_gl_errors!(cx);

        tile_program.add_attribute(
            cx,
            CStr::from_bytes_with_nul(b"position\0").unwrap(),
            &VertexAttribParams::new(2, 8, 0)
        );
        tile_program.add_attribute(
            cx,
            CStr::from_bytes_with_nul(b"tex_coord\0").unwrap(),
            &VertexAttribParams::new(2, 8, 2)
        );
        tile_program.add_attribute(
            cx,
            CStr::from_bytes_with_nul(b"tex_minmax\0").unwrap(),
            &VertexAttribParams::new(4, 8, 4)
        );
        check_gl_errors!(cx);

        tile_program.enable_vertex_attribs(cx);
        tile_program.set_vertex_attribs(cx, &tile_buffer);

        MapViewGl {
            map_view,
            viewport_size: initial_size,
            tile_program,
            tile_buffer,
            tile_cache: TileCache::new(move |_tile| update_func(), use_network),
            tile_atlas: TileAtlas::new(cx, atlas_tex, 256, use_async),
        }
    }

    pub fn set_viewport_size(&mut self, cx: &mut Context, width: u32, height: u32) {
        self.viewport_size = (width, height);
        self.map_view.set_size(f64::from(width), f64::from(height));
        cx.set_viewport(0, 0, width, height);
    }

    pub fn viewport_in_map(&self) -> bool {
        self.map_view.viewport_in_map()
    }

    pub fn increase_atlas_size(&mut self, cx: &mut Context) -> Result<(), ()> {
        self.tile_atlas.double_texture_size(cx)
    }

    /// Returns `Err` when tile cache is too small for this view.
    /// Returns the number of OpenGL draw calls, which can be decreased to `1` by increasing the
    /// size of the tile atlas.
    pub fn draw(&mut self, cx: &mut Context, source: &TileSource) -> Result<usize, usize> {
        self.tile_cache.set_view_location(View {
            source_id: source.id(),
            zoom: self.map_view.tile_zoom(),
            center: self.map_view.center,
        });

        let visible_tiles = self.map_view.visible_tiles(true);
        let mut remainder = visible_tiles.as_slice();
        let mut num_draws = 0;
        let mut max_tiles_to_use = self.tile_cache.max_tiles();

        loop {
            let (textured_visible_tiles, remainder_opt, used_tiles) = {
                self.tile_atlas.textured_visible_tiles(
                    cx,
                    remainder,
                    max_tiles_to_use,
                    source,
                    &mut self.tile_cache,
                )
            };

            max_tiles_to_use -= used_tiles;

            let mut vertex_data: Vec<f32> = Vec::with_capacity(textured_visible_tiles.len() * (6 * 8));
            let scale_x = 2.0 / f64::from(self.viewport_size.0);
            let scale_y = -2.0 / f64::from(self.viewport_size.1);
            for tvt in &textured_visible_tiles {
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

            self.tile_buffer.set_data(cx, &vertex_data, vertex_data.len() / 4);
            self.tile_buffer.draw(cx, &self.tile_program, DrawMode::Triangles);

            num_draws += 1;

            debug!("draw #{}: tvt.len() = {}, remainder = {:?}, max_tiles = {}",
                num_draws,
                textured_visible_tiles.len(),
                remainder_opt.map(|r| r.len()),
                max_tiles_to_use);

            if max_tiles_to_use == 0 {
                warn!("tile cache is too small for this view.");
                return Err(num_draws);
            }

            match remainder_opt {
                None => return Ok(num_draws),
                Some(new_remainder) => {
                    if new_remainder.len() >= remainder.len() {
                        warn!("failed to draw all tiles. number of remaining tiles did not decrease.");
                        return Err(num_draws);
                    } else {
                        remainder = new_remainder;
                    }
                },
            }
        }
    }

    pub fn step_zoom(&mut self, steps: i32, step_size: f64) {
        let new_zoom = {
            let z = (self.map_view.zoom + f64::from(steps) * step_size) / step_size;
            if steps > 0 {
                z.ceil() * step_size
            } else {
                z.floor() * step_size
            }
        }.max(MIN_ZOOM_LEVEL).min(MAX_ZOOM_LEVEL);

        self.map_view.set_zoom(new_zoom);
    }

    pub fn zoom(&mut self, zoom_delta: f64) {
        if self.map_view.zoom + zoom_delta < MIN_ZOOM_LEVEL {
            self.map_view.set_zoom(MIN_ZOOM_LEVEL);
        } else if self.map_view.zoom + zoom_delta > MAX_ZOOM_LEVEL {
            self.map_view.set_zoom(MAX_ZOOM_LEVEL);
        } else {
            self.map_view.zoom(zoom_delta);
        }
    }

    pub fn zoom_at(&mut self, pos: ScreenCoord, zoom_delta: f64) {
        if self.map_view.zoom + zoom_delta < MIN_ZOOM_LEVEL {
            self.map_view.set_zoom_at(pos, MIN_ZOOM_LEVEL);
        } else if self.map_view.zoom + zoom_delta > MAX_ZOOM_LEVEL {
            self.map_view.set_zoom_at(pos, MAX_ZOOM_LEVEL);
        } else {
            self.map_view.zoom_at(pos, zoom_delta);
        }
        self.map_view.center.normalize_xy();
    }

    pub fn change_tile_zoom_offset(&mut self, delta_offset: f64) {
        let offset = self.map_view.tile_zoom_offset();
        self.map_view.set_tile_zoom_offset(offset + delta_offset);
    }

    pub fn move_pixel(&mut self, delta_x: f64, delta_y: f64) {
        self.map_view.move_pixel(delta_x, delta_y);
        self.map_view.center.normalize_xy();
    }
}
