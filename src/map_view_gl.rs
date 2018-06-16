use context::Context;
use coord::{MapCoord, ScreenCoord};
use globe_tile_layer::GlobeTileLayer;
use map_view::MapView;
use marker_layer::MarkerLayer;
use session::Session;
use texture::{Texture, TextureFormat};
use tile_atlas::TileAtlas;
use tile_cache::TileCache;
use tile_layer::TileLayer;
use tile_source::TileSource;


const MIN_ZOOM_LEVEL: f64 = 0.0;
const MAX_ZOOM_LEVEL: f64 = 22.0;

#[derive(Debug)]
pub struct MapViewGl {
    map_view: MapView,
    viewport_size: (u32, u32),
    tile_cache: TileCache,
    tile_atlas: TileAtlas,
    tile_layer: TileLayer,
    marker_layer: MarkerLayer,
    globe_tile_layer: GlobeTileLayer,
    last_draw_type: DrawType,
}

#[derive(Debug, Eq, PartialEq)]
enum DrawType {
    Null,
    Tiles,
    Markers,
    Globe,
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

        let mut tile_atlas = TileAtlas::new(cx, atlas_tex, tile_size, use_async);
        //TODO remove this
        tile_atlas.double_texture_size(cx);
        tile_atlas.double_texture_size(cx);
        tile_atlas.double_texture_size(cx);

        let tile_layer = TileLayer::new(cx, &tile_atlas);

        MapViewGl {
            map_view,
            viewport_size: initial_size,
            tile_cache: TileCache::new(move |_tile| update_func(), use_network),
            tile_atlas,
            tile_layer,
            marker_layer: MarkerLayer::new(cx),
            globe_tile_layer: GlobeTileLayer::new(cx),
            last_draw_type: DrawType::Null,
        }
    }

    pub fn set_viewport_size(&mut self, cx: &mut Context, width: u32, height: u32) {
        self.viewport_size = (width, height);
        self.map_view.set_size(f64::from(width), f64::from(height));
        cx.set_viewport(0, 0, width, height);
    }

    pub fn add_marker(&mut self, map_coord: MapCoord) {
        self.marker_layer.add_marker(map_coord);
    }

    pub fn viewport_in_map(&self) -> bool {
        self.map_view.viewport_in_map()
    }

    pub fn increase_atlas_size(&mut self, cx: &mut Context) -> Result<(), ()> {
        self.tile_atlas.double_texture_size(cx)
    }

    fn draw_tiles(&mut self, cx: &mut Context, source: &TileSource, snap_to_pixel: bool)
        -> Result<usize, usize>
    {
        if self.last_draw_type != DrawType::Tiles {
            self.last_draw_type = DrawType::Tiles;
            self.tile_layer.prepare_draw(cx, &self.tile_atlas);
        }

        self.tile_layer.draw(
            cx,
            &self.map_view,
            source,
            &mut self.tile_cache,
            &mut self.tile_atlas,
            self.viewport_size,
            snap_to_pixel
        )
    }

    fn draw_marker(&mut self, cx: &mut Context, snap_to_pixel: bool) {
        if self.last_draw_type != DrawType::Markers {
            self.last_draw_type = DrawType::Markers;
            self.marker_layer.prepare_draw(cx);
        }

        self.marker_layer.draw(cx, &self.map_view, self.viewport_size, snap_to_pixel);
    }

    fn draw_globe(&mut self, cx: &mut Context, source: &TileSource) {
        if self.last_draw_type != DrawType::Globe {
            self.last_draw_type = DrawType::Globe;
            self.globe_tile_layer.prepare_draw(cx, &self.tile_atlas);
        }

        self.globe_tile_layer.draw(
            cx,
            &self.map_view,
            source,
            &mut self.tile_cache,
            &mut self.tile_atlas,
            self.viewport_size,
        );
    }

    /// Returns `Err` when tile cache is too small for this view.
    /// Returns the number of OpenGL draw calls, which can be decreased to `1` by increasing the
    /// size of the tile atlas.
    pub fn draw(&mut self, cx: &mut Context, source: &TileSource) -> Result<usize, usize> {
        // only snap to pixel grid if zoom has integral value
        let snap_to_pixel = (self.map_view.zoom - (self.map_view.zoom + 0.5).floor()).abs() < 1e-10;

        let ret = self.draw_tiles(cx, source, snap_to_pixel);

        if !self.marker_layer.is_empty() {
            self.draw_marker(cx, snap_to_pixel);
        }

        self.draw_globe(cx, source);

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

    pub fn restore_session(&mut self, session: &Session) {
        self.map_view.center = session.view_center;
        self.map_view.center.normalize_xy();
        self.map_view.zoom = MIN_ZOOM_LEVEL.max(MAX_ZOOM_LEVEL.min(session.zoom));
    }

    pub fn to_session(&self) -> Session {
        Session {
            view_center: self.map_view.center,
            zoom: self.map_view.zoom,
            tile_source: None,
        }
    }
}
