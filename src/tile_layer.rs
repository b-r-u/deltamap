use ::std::ffi::CStr;
use buffer::{Buffer, DrawMode};
use context::Context;
use coord::View;
use map_view::MapView;
use program::Program;
use texture::{Texture, TextureFormat};
use tile::Tile;
use tile_atlas::TileAtlas;
use tile_cache::TileCache;
use tile_source::TileSource;
use vertex_attrib::VertexAttribParams;


#[derive(Debug)]
pub struct TileLayer {
    program: Program,
    buffer: Buffer,
    cache: TileCache,
    atlas: TileAtlas,
}


impl TileLayer {
    pub fn new<F>(
        cx: &mut Context,
        update_func: F,
        tile_size: u32,
        use_network: bool,
        use_async: bool,
    ) -> TileLayer
        where F: Fn(Tile) + Sync + Send + 'static,
    {
        let buffer = Buffer::new(cx, &[], 0);
        check_gl_errors!(cx);
        cx.bind_buffer(buffer.id());

        let mut program = Program::new(
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

        program.add_texture(cx, &atlas_tex, CStr::from_bytes_with_nul(b"tex_map\0").unwrap());
        check_gl_errors!(cx);

        program.add_attribute(
            cx,
            CStr::from_bytes_with_nul(b"position\0").unwrap(),
            &VertexAttribParams::new(2, 8, 0)
        );
        program.add_attribute(
            cx,
            CStr::from_bytes_with_nul(b"tex_coord\0").unwrap(),
            &VertexAttribParams::new(2, 8, 2)
        );
        program.add_attribute(
            cx,
            CStr::from_bytes_with_nul(b"tex_minmax\0").unwrap(),
            &VertexAttribParams::new(4, 8, 4)
        );
        check_gl_errors!(cx);

        TileLayer {
            program,
            buffer,
            cache: TileCache::new(update_func, use_network),
            atlas: TileAtlas::new(cx, atlas_tex, 256, use_async),
        }
    }

    pub fn double_atlas_size(&mut self, cx: &mut Context) -> Result<(), ()> {
        self.atlas.double_texture_size(cx)
    }

    // Has to be called once before one or multiple calls to `draw`.
    pub fn prepare_draw(&mut self, cx: &mut Context) {
        self.program.enable_vertex_attribs(cx);
        self.program.set_vertex_attribs(cx, &self.buffer);
        cx.set_active_texture_unit(self.atlas.texture().unit());
    }

    pub fn draw(
        &mut self,
        cx: &mut Context,
        map_view: &MapView,
        source: &TileSource,
        viewport_size: (u32, u32),
        snap_to_pixel: bool
    ) -> Result<usize, usize> {
        self.cache.set_view_location(View {
            source_id: source.id(),
            zoom: map_view.tile_zoom(),
            center: map_view.center,
        });

        let visible_tiles = map_view.visible_tiles(snap_to_pixel);
        let mut remainder = visible_tiles.as_slice();
        let mut num_draws = 0;
        let mut max_tiles_to_use = self.cache.max_tiles();

        loop {
            let (textured_visible_tiles, remainder_opt, used_tiles) = {
                self.atlas.textured_visible_tiles(
                    cx,
                    remainder,
                    max_tiles_to_use,
                    source,
                    &mut self.cache,
                )
            };

            max_tiles_to_use -= used_tiles;

            let mut vertex_data: Vec<f32> = Vec::with_capacity(textured_visible_tiles.len() * (6 * 8));
            let scale_x = 2.0 / f64::from(viewport_size.0);
            let scale_y = -2.0 / f64::from(viewport_size.1);
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

            self.buffer.set_data(cx, &vertex_data, vertex_data.len() / 4);
            self.buffer.draw(cx, &self.program, DrawMode::Triangles);

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
}
