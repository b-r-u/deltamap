use ::std::ffi::CStr;
use buffer::{Buffer, DrawMode};
use cgmath::{Matrix3, Point2, Transform, vec2, vec3};
use context::Context;
use coord::{MapCoord, ScreenCoord, ScreenRect, View};
use image;
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
    marker_buffer: Buffer,
    marker_program: Program,
    marker_tex: Texture,
    markers: Vec<MapCoord>,
    last_draw_type: DrawType,
}

#[derive(Debug, Eq, PartialEq)]
enum DrawType {
    Null,
    Tiles,
    Markers
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


        let marker_buffer = Buffer::new(cx, &[], 0);
        cx.bind_buffer(marker_buffer.id());
        check_gl_errors!(cx);

        let mut marker_program = Program::new(
            cx,
            include_bytes!("../shader/marker.vert"),
            include_bytes!("../shader/marker.frag"),
        ).unwrap();
        check_gl_errors!(cx);

        let marker_tex = {
            let img = image::load_from_memory(
                include_bytes!("../img/marker.png"),
            ).unwrap();
            Texture::new(cx, &img).unwrap()
        };

        marker_program.add_texture(cx, &marker_tex, CStr::from_bytes_with_nul(b"tex\0").unwrap());

        marker_program.add_attribute(
            cx,
            CStr::from_bytes_with_nul(b"position\0").unwrap(),
            &VertexAttribParams::new(2, 4, 0)
        );
        marker_program.add_attribute(
            cx,
            CStr::from_bytes_with_nul(b"tex_coord\0").unwrap(),
            &VertexAttribParams::new(2, 4, 2)
        );

        MapViewGl {
            map_view,
            viewport_size: initial_size,
            tile_program,
            tile_buffer,
            tile_cache: TileCache::new(move |_tile| update_func(), use_network),
            tile_atlas: TileAtlas::new(cx, atlas_tex, 256, use_async),
            marker_buffer,
            marker_program,
            marker_tex,
            markers: vec![],
            last_draw_type: DrawType::Null,
        }
    }

    pub fn set_viewport_size(&mut self, cx: &mut Context, width: u32, height: u32) {
        self.viewport_size = (width, height);
        self.map_view.set_size(f64::from(width), f64::from(height));
        cx.set_viewport(0, 0, width, height);
    }

    pub fn add_marker(&mut self, map_coord: MapCoord) {
        self.markers.push(map_coord);
    }

    pub fn viewport_in_map(&self) -> bool {
        self.map_view.viewport_in_map()
    }

    pub fn increase_atlas_size(&mut self, cx: &mut Context) -> Result<(), ()> {
        self.tile_atlas.double_texture_size(cx)
    }

    fn draw_tiles(&mut self, cx: &mut Context, source: &TileSource, snap_to_pixel: bool) -> Result<usize, usize> {
        if self.last_draw_type != DrawType::Tiles {
            self.last_draw_type = DrawType::Tiles;
            self.tile_program.enable_vertex_attribs(cx);
            self.tile_program.set_vertex_attribs(cx, &self.tile_buffer);
            cx.set_active_texture_unit(self.tile_atlas.texture().unit());
        }

        self.tile_cache.set_view_location(View {
            source_id: source.id(),
            zoom: self.map_view.tile_zoom(),
            center: self.map_view.center,
        });

        let visible_tiles = self.map_view.visible_tiles(snap_to_pixel);
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

    fn draw_marker(&mut self, cx: &mut Context, snap_to_pixel: bool) {
        if self.last_draw_type != DrawType::Markers {
            self.last_draw_type = DrawType::Markers;
            cx.set_active_texture_unit(self.marker_tex.unit());
            self.marker_program.enable_vertex_attribs(cx);
            self.marker_program.set_vertex_attribs(cx, &self.marker_buffer);
        }

        let mut vertex_data: Vec<f32> = vec![];

        let marker_size = vec2::<f64>(40.0, 50.0);
        let marker_offset = vec2::<f64>(-20.0, -50.0);

        let scale_x = 2.0 / self.viewport_size.0 as f32;
        let scale_y = -2.0 / self.viewport_size.1 as f32;

        let tex_mat: Matrix3<f32> = Matrix3::from_cols(
            vec3(marker_size.x as f32, 0.0, 0.0),
            vec3(0.0, marker_size.y as f32, 0.0),
            vec3(marker_offset.x as f32, marker_offset.y as f32, 1.0),
        );

        let screen_mat: Matrix3<f32> = Matrix3::from_cols(
            vec3(scale_x, 0.0, 0.0),
            vec3(0.0, scale_y, 0.0),
            vec3(-1.0, 1.0, 1.0),
        );

        let t1 = Point2::new(0.0f32, 0.0);
        let t2 = Point2::new(1.0f32, 0.0);
        let t3 = Point2::new(1.0f32, 1.0);
        let t4 = Point2::new(0.0f32, 1.0);

        let visible_rect = ScreenRect {
            x: marker_offset.x,
            y: marker_offset.y,
            width: f64::from(self.viewport_size.0) + marker_size.x,
            height: f64::from(self.viewport_size.1) + marker_size.y,
        };

        for m in &self.markers {
            let screen_pos = {
                let mut sp = self.map_view.map_to_screen_coord(*m);
                if snap_to_pixel {
                    let topleft = self.map_view.map_to_screen_coord(MapCoord::new(0.0, 0.0));
                    let mut snapped = topleft;
                    snapped.snap_to_pixel();

                    sp.x += snapped.x - topleft.x;
                    sp.y += snapped.y - topleft.y;
                }
                sp
            };

            if !screen_pos.is_inside(&visible_rect) {
                continue;
            }
            let trans_mat: Matrix3<f32> = Matrix3::from_cols(
                vec3(0.0, 0.0, 0.0),
                vec3(0.0, 0.0, 0.0),
                vec3(screen_pos.x as f32, screen_pos.y as f32, 0.0),
            );
            let mat: Matrix3<f32> = screen_mat * (tex_mat + trans_mat);

            let p1: Point2<f32> = mat.transform_point(t1);
            let p2: Point2<f32> = mat.transform_point(t2);
            let p3: Point2<f32> = mat.transform_point(t3);
            let p4: Point2<f32> = mat.transform_point(t4);

            vertex_data.extend::<&[f32; 2]>(p1.as_ref());
            vertex_data.extend::<&[f32; 2]>(t1.as_ref());
            vertex_data.extend::<&[f32; 2]>(p2.as_ref());
            vertex_data.extend::<&[f32; 2]>(t2.as_ref());
            vertex_data.extend::<&[f32; 2]>(p3.as_ref());
            vertex_data.extend::<&[f32; 2]>(t3.as_ref());
            vertex_data.extend::<&[f32; 2]>(p1.as_ref());
            vertex_data.extend::<&[f32; 2]>(t1.as_ref());
            vertex_data.extend::<&[f32; 2]>(p3.as_ref());
            vertex_data.extend::<&[f32; 2]>(t3.as_ref());
            vertex_data.extend::<&[f32; 2]>(p4.as_ref());
            vertex_data.extend::<&[f32; 2]>(t4.as_ref());
        }

        self.marker_buffer.set_data(cx, &vertex_data, vertex_data.len() / 4);
        self.marker_buffer.draw(cx, &self.marker_program, DrawMode::Triangles);
    }

    /// Returns `Err` when tile cache is too small for this view.
    /// Returns the number of OpenGL draw calls, which can be decreased to `1` by increasing the
    /// size of the tile atlas.
    pub fn draw(&mut self, cx: &mut Context, source: &TileSource) -> Result<usize, usize> {
        // only snap to pixel grid if zoom has integral value
        let snap_to_pixel = (self.map_view.zoom - (self.map_view.zoom + 0.5).floor()).abs() < 1e-10;

        let ret = self.draw_tiles(cx, source, snap_to_pixel);

        if !self.markers.is_empty() {
            self.draw_marker(cx, snap_to_pixel);
        }

        ret
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
