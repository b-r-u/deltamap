use atmos_layer::AtmosLayer;
use context::Context;
use coord::{MapCoord, ScreenCoord};
use map_view::{MapView, MIN_ZOOM_LEVEL, MAX_ZOOM_LEVEL};
use marker_layer::MarkerLayer;
use mercator_tile_layer::MercatorTileLayer;
use mercator_view::MercatorView;
use ortho_tile_layer::OrthoTileLayer;
use orthografic_view::OrthograficView;
use projection::Projection;
use session::Session;
use texture::{Texture, TextureFormat};
use tile_atlas::TileAtlas;
use tile_cache::TileCache;
use tile_source::TileSource;


#[derive(Debug)]
pub struct MapViewGl {
    map_view: MapView,
    /// Size in physical pixels
    viewport_size: (u32, u32),
    dpi_factor: f64,
    tile_cache: TileCache,
    tile_atlas: TileAtlas,
    mercator_tile_layer: MercatorTileLayer,
    marker_layer: MarkerLayer,
    ortho_tile_layer: OrthoTileLayer,
    atmos_layer: AtmosLayer,
    projection: Projection,
    show_atmos: bool,
    last_draw_type: DrawType,
}

#[derive(Debug, Eq, PartialEq)]
enum DrawType {
    Null,
    Tiles,
    Markers,
    OrthoTiles,
    Atmos,
}

impl MapViewGl {
    pub fn new<F>(
        cx: &mut Context,
        initial_size: (u32, u32),
        dpi_factor: f64,
        update_func: F,
        use_network: bool,
        use_async: bool,
        ) -> MapViewGl
        where F: Fn() + Sync + Send + 'static,
    {
        let tile_size = 256;

        let map_view = MercatorView::initial_map_view(
            f64::from(initial_size.0),
            f64::from(initial_size.1),
            tile_size,
        );

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

        let tile_atlas = TileAtlas::new(cx, atlas_tex, tile_size, use_async);

        let mercator_tile_layer = MercatorTileLayer::new(cx, &tile_atlas);
        let ortho_tile_layer = OrthoTileLayer::new(cx, &tile_atlas);
        let atmos_layer = AtmosLayer::new(cx);

        MapViewGl {
            map_view,
            viewport_size: initial_size,
            dpi_factor,
            tile_cache: TileCache::new(move |_tile| update_func(), use_network),
            tile_atlas,
            mercator_tile_layer,
            marker_layer: MarkerLayer::new(cx),
            ortho_tile_layer,
            atmos_layer,
            projection: Projection::Mercator,
            show_atmos: false,
            last_draw_type: DrawType::Null,
        }
    }

    pub fn set_viewport_size(&mut self, cx: &mut Context, width: u32, height: u32) {
        self.viewport_size = (width, height);
        self.map_view.set_size(f64::from(width), f64::from(height));
        cx.set_viewport(0, 0, width, height);
    }

    pub fn set_dpi_factor(&mut self, dpi_factor: f64) {
        self.dpi_factor = dpi_factor;
    }

    pub fn add_marker(&mut self, map_coord: MapCoord) {
        self.marker_layer.add_marker(map_coord);
    }

    pub fn map_covers_viewport(&self) -> bool {
        match self.projection {
            Projection::Mercator => MercatorView::covers_viewport(&self.map_view),
            Projection::Orthografic => OrthograficView::covers_viewport(&self.map_view),
        }
    }

    pub fn increase_atlas_size(&mut self, cx: &mut Context) -> Result<(), ()> {
        self.tile_atlas.double_texture_size(cx)
    }

    pub fn toggle_projection(&mut self) {
        self.projection = match self.projection {
            Projection::Mercator => Projection::Orthografic,
            Projection::Orthografic => Projection::Mercator,
        };
    }

    pub fn toggle_atmosphere(&mut self) {
        self.show_atmos = !self.show_atmos;
    }

    fn draw_mercator_tiles(&mut self, cx: &mut Context, source: &TileSource, snap_to_pixel: bool)
        -> Result<usize, usize>
    {
        if self.last_draw_type != DrawType::Tiles {
            self.last_draw_type = DrawType::Tiles;
            self.mercator_tile_layer.prepare_draw(cx, &self.tile_atlas);
        }

        self.mercator_tile_layer.draw(
            cx,
            &self.map_view,
            source,
            &mut self.tile_cache,
            &mut self.tile_atlas,
            snap_to_pixel
        )
    }

    fn draw_mercator_marker(&mut self, cx: &mut Context, snap_to_pixel: bool) {
        if self.last_draw_type != DrawType::Markers {
            self.last_draw_type = DrawType::Markers;
            self.marker_layer.prepare_draw(cx);
        }

        self.marker_layer.draw_mercator(
            cx,
            &self.map_view,
            self.dpi_factor,
            snap_to_pixel,
        );
    }

    fn draw_ortho_marker(&mut self, cx: &mut Context) {
        if self.last_draw_type != DrawType::Markers {
            self.last_draw_type = DrawType::Markers;
            self.marker_layer.prepare_draw(cx);
        }

        self.marker_layer.draw_ortho(
            cx,
            &self.map_view,
            self.dpi_factor,
        );
    }

    fn draw_ortho_tiles(&mut self, cx: &mut Context, source: &TileSource) -> Result<usize, usize> {
        if self.last_draw_type != DrawType::OrthoTiles {
            self.last_draw_type = DrawType::OrthoTiles;
            self.ortho_tile_layer.prepare_draw(cx, &self.tile_atlas);
        }

        self.ortho_tile_layer.draw(
            cx,
            &self.map_view,
            source,
            &mut self.tile_cache,
            &mut self.tile_atlas,
        )
    }

    fn draw_atmos(&mut self, cx: &mut Context) {
        if self.last_draw_type != DrawType::Atmos {
            self.last_draw_type = DrawType::Atmos;
            self.atmos_layer.prepare_draw(cx);
        }

        self.atmos_layer.draw(
            cx,
            &self.map_view,
        )
    }

    /// Returns `Err` when tile cache is too small for this view.
    /// Returns the number of OpenGL draw calls, which can be decreased to `1` by increasing the
    /// size of the tile atlas.
    pub fn draw(&mut self, cx: &mut Context, source: &TileSource) -> Result<usize, usize> {
        // only snap to pixel grid if zoom has integral value
        let snap_to_pixel = (self.map_view.zoom - (self.map_view.zoom + 0.5).floor()).abs() < 1e-10;

        match self.projection {
            Projection::Mercator => {
                let ret = self.draw_mercator_tiles(cx, source, snap_to_pixel);
                if !self.marker_layer.is_empty() {
                    self.draw_mercator_marker(cx, snap_to_pixel);
                }
                ret
            },
            Projection::Orthografic => {
                let ret = self.draw_ortho_tiles(cx, source);
                if !self.marker_layer.is_empty() {
                    self.draw_ortho_marker(cx);
                }
                if self.show_atmos {
                    self.draw_atmos(cx);
                }
                ret
            },
        }
    }

    pub fn zoom(&mut self, zoom_delta: f64) {
        self.map_view.zoom(zoom_delta);
    }

    pub fn step_zoom(&mut self, steps: i32, step_size: f64) {
        self.map_view.step_zoom(steps, step_size);
    }

    pub fn zoom_at(&mut self, pos: ScreenCoord, zoom_delta: f64) {
        match self.projection {
            Projection::Mercator => {
                if self.map_view.zoom + zoom_delta < MIN_ZOOM_LEVEL {
                    MercatorView::set_zoom_at(&mut self.map_view, pos, MIN_ZOOM_LEVEL);
                } else if self.map_view.zoom + zoom_delta > MAX_ZOOM_LEVEL {
                    MercatorView::set_zoom_at(&mut self.map_view, pos, MAX_ZOOM_LEVEL);
                } else {
                    MercatorView::zoom_at(&mut self.map_view, pos, zoom_delta);
                }
                self.map_view.center.normalize_xy();
            },
            Projection::Orthografic => {
                if self.map_view.zoom + zoom_delta < MIN_ZOOM_LEVEL {
                    OrthograficView::set_zoom_at(&mut self.map_view, pos, MIN_ZOOM_LEVEL);
                } else if self.map_view.zoom + zoom_delta > MAX_ZOOM_LEVEL {
                    OrthograficView::set_zoom_at(&mut self.map_view, pos, MAX_ZOOM_LEVEL);
                } else {
                    OrthograficView::zoom_at(&mut self.map_view, pos, zoom_delta);
                }
            },
        }
    }

    pub fn change_tile_zoom_offset(&mut self, delta_offset: f64) {
        let offset = self.map_view.tile_zoom_offset;
        self.map_view.set_tile_zoom_offset(offset + delta_offset);
    }

    //TODO Make sure to use physical pixel deltas
    pub fn move_pixel(&mut self, delta_x: f64, delta_y: f64) {
        //TODO implement for OrthograficView
        MercatorView::move_pixel(&mut self.map_view, delta_x, delta_y);
        self.map_view.center.normalize_xy();
    }

    pub fn restore_session(&mut self, session: &Session) {
        self.map_view.center = session.view_center;
        self.map_view.center.normalize_xy();
        self.map_view.zoom = session.zoom
            .max(MIN_ZOOM_LEVEL)
            .min(MAX_ZOOM_LEVEL);
        self.projection = session.projection;
    }

    pub fn to_session(&self) -> Session {
        Session {
            view_center: self.map_view.center,
            zoom: self.map_view.zoom,
            tile_source: None,
            projection: self.projection,
        }
    }
}
