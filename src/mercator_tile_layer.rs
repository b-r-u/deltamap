use ::std::ffi::CStr;
use buffer::{Buffer, DrawMode};
use context::Context;
use coord::View;
use map_view::MapView;
use mercator_view::MercatorView;
use program::Program;
use tile_atlas::{TileAtlas, VisibleTilesProvider};
use tile_cache::TileCache;
use tile_source::TileSource;
use vertex_attrib::VertexAttribParams;


#[derive(Debug)]
pub struct MercatorTileLayer {
    program: Program,
    buffer: Buffer,
}


impl MercatorTileLayer {
    pub fn new(
        cx: &mut Context,
        atlas: &TileAtlas,
    ) -> MercatorTileLayer
    {
        let buffer = Buffer::new(cx, &[], 0);
        check_gl_errors!(cx);
        cx.bind_buffer(buffer.id());

        let mut program = Program::new(
            cx,
            include_bytes!("../shader/map.vert"),
            include_bytes!("../shader/map.frag"),
        ).unwrap();

        program.add_texture(cx, atlas.texture(), CStr::from_bytes_with_nul(b"tex_map\0").unwrap());

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

        MercatorTileLayer {
            program,
            buffer,
        }
    }

    // Has to be called once before one or multiple calls to `draw`.
    pub fn prepare_draw(&mut self, cx: &mut Context, atlas: &TileAtlas) {
        self.program.enable_vertex_attribs(cx);
        self.program.set_vertex_attribs(cx, &self.buffer);
        cx.set_active_texture_unit(atlas.texture().unit());
    }

    pub fn draw(
        &mut self,
        cx: &mut Context,
        map_view: &MapView,
        source: &TileSource,
        cache: &mut TileCache,
        atlas: &mut TileAtlas,
        viewport_size: (u32, u32),
        snap_to_pixel: bool
    ) -> Result<usize, usize> {
        cache.set_view_location(View {
            source_id: source.id(),
            zoom: MercatorView::tile_zoom(map_view),
            center: map_view.center,
        });

        let visible_tiles = MercatorView::visible_tiles(map_view, snap_to_pixel);
        let mut remainder = visible_tiles.as_slice();
        let mut num_draws = 0;
        let mut max_tiles_to_use = cache.max_tiles();

        loop {
            let (textured_visible_tiles, remainder_opt, used_tiles) = {
                atlas.textured_visible_tiles(
                    cx,
                    remainder,
                    max_tiles_to_use,
                    source,
                    cache,
                )
            };

            max_tiles_to_use = max_tiles_to_use.saturating_sub(used_tiles);

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
