use atmos_layer::AtmosLayer;
use cgmath::vec2;
use context::Context;
use coord::{MapCoord, ScreenCoord};
use marker_layer::MarkerLayer;
use mercator_tile_layer::MercatorTileLayer;
use mercator_view::MercatorView;
use ortho_tile_layer::OrthoTileLayer;
use orthografic_view::OrthograficView;
use projection::Projection;
use projection_view::ProjectionView;
use session::Session;
use texture::{Texture, TextureFormat};
use tile_atlas::TileAtlas;
use tile_cache::TileCache;
use tile_source::TileSource;


pub const MIN_TILE_ZOOM_OFFSET: f64 = -4.0;
pub const MAX_TILE_ZOOM_OFFSET: f64 = 4.0;

#[derive(Debug)]
pub struct MapViewGl {
    proj_view: ProjectionView,
    /// Size in physical pixels
    viewport_size: (u32, u32),
    dpi_factor: f64,
    tile_cache: TileCache,
    tile_atlas: TileAtlas,
    mercator_tile_layer: MercatorTileLayer,
    marker_layer: MarkerLayer,
    ortho_tile_layer: OrthoTileLayer,
    atmos_layer: AtmosLayer,
    show_marker: bool,
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

        let proj_view = ProjectionView::Mercator(
            MercatorView::initial_view(
                f64::from(initial_size.0),
                f64::from(initial_size.1),
                tile_size,
            )
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
            proj_view,
            viewport_size: initial_size,
            dpi_factor,
            tile_cache: TileCache::new(move |_tile| update_func(), use_network),
            tile_atlas,
            mercator_tile_layer,
            marker_layer: MarkerLayer::new(cx),
            ortho_tile_layer,
            atmos_layer,
            show_marker: true,
            show_atmos: false,
            last_draw_type: DrawType::Null,
        }
    }

    pub fn set_viewport_size(&mut self, cx: &mut Context, width: u32, height: u32) {
        self.viewport_size = (width, height);
        let vec_size = vec2(f64::from(width), f64::from(height));
        match &mut self.proj_view {
            ProjectionView::Mercator(merc) => merc.viewport_size = vec_size,
            ProjectionView::Orthografic(ortho) => ortho.viewport_size = vec_size,
        }
        cx.set_viewport(0, 0, width, height);
    }

    pub fn set_dpi_factor(&mut self, dpi_factor: f64) {
        self.dpi_factor = dpi_factor;
    }

    pub fn add_marker(&mut self, map_coord: MapCoord) {
        self.marker_layer.add_marker(map_coord);
    }

    pub fn map_covers_viewport(&self) -> bool {
        match &self.proj_view {
            ProjectionView::Mercator(ref merc) => merc.covers_viewport(),
            ProjectionView::Orthografic(ref ortho) => ortho.covers_viewport(),
        }
    }

    pub fn increase_atlas_size(&mut self, cx: &mut Context) -> Result<(), ()> {
        self.tile_atlas.double_texture_size(cx)
    }

    pub fn toggle_projection(&mut self) {
        self.proj_view = match &self.proj_view {
            ProjectionView::Orthografic(ortho) =>
                ProjectionView::Mercator(MercatorView::from_orthografic_view(ortho)),
            ProjectionView::Mercator(merc) =>
                ProjectionView::Orthografic(OrthograficView::from_mercator_view(merc)),
        };
    }

    pub fn toggle_marker(&mut self) {
        self.show_marker = !self.show_marker;
    }

    pub fn toggle_atmosphere(&mut self) {
        self.show_atmos = !self.show_atmos;
    }

    fn draw_mercator_tiles(&mut self, cx: &mut Context, merc: &MercatorView, source: &TileSource, snap_to_pixel: bool)
        -> Result<usize, usize>
    {
        if self.last_draw_type != DrawType::Tiles {
            self.last_draw_type = DrawType::Tiles;
            self.mercator_tile_layer.prepare_draw(cx, &self.tile_atlas);
        }

        self.mercator_tile_layer.draw(
            cx,
            merc,
            source,
            &mut self.tile_cache,
            &mut self.tile_atlas,
            snap_to_pixel
        )
    }

    fn draw_mercator_marker(&mut self, cx: &mut Context, merc: &MercatorView, snap_to_pixel: bool) {
        if self.last_draw_type != DrawType::Markers {
            self.last_draw_type = DrawType::Markers;
            self.marker_layer.prepare_draw(cx);
        }

        self.marker_layer.draw_mercator(
            cx,
            merc,
            self.dpi_factor,
            snap_to_pixel,
        );
    }

    fn draw_ortho_marker(&mut self, cx: &mut Context, ortho: &OrthograficView) {
        if self.last_draw_type != DrawType::Markers {
            self.last_draw_type = DrawType::Markers;
            self.marker_layer.prepare_draw(cx);
        }

        self.marker_layer.draw_ortho(
            cx,
            ortho,
            self.dpi_factor,
        );
    }

    fn draw_ortho_tiles(&mut self, cx: &mut Context, ortho: &OrthograficView, source: &TileSource) -> Result<usize, usize> {
        if self.last_draw_type != DrawType::OrthoTiles {
            self.last_draw_type = DrawType::OrthoTiles;
            self.ortho_tile_layer.prepare_draw(cx, &self.tile_atlas);
        }

        self.ortho_tile_layer.draw(
            cx,
            ortho,
            source,
            &mut self.tile_cache,
            &mut self.tile_atlas,
        )
    }

    fn draw_atmos(&mut self, cx: &mut Context, ortho: &OrthograficView) {
        if self.last_draw_type != DrawType::Atmos {
            self.last_draw_type = DrawType::Atmos;
            self.atmos_layer.prepare_draw(cx);
        }

        self.atmos_layer.draw(
            cx,
            ortho,
        )
    }

    /// Returns `Err` when tile cache is too small for this view.
    /// Returns the number of OpenGL draw calls, which can be decreased to `1` by increasing the
    /// size of the tile atlas.
    pub fn draw(&mut self, cx: &mut Context, source: &TileSource) -> Result<usize, usize> {
        match self.proj_view.clone() {
            ProjectionView::Mercator(ref merc) => {
                // only snap to pixel grid if zoom has integral value
                let snap_to_pixel = (merc.zoom - (merc.zoom + 0.5).floor()).abs() < 1e-10;

                let ret = self.draw_mercator_tiles(cx, merc, source, snap_to_pixel);
                if self.show_marker && !self.marker_layer.is_empty() {
                    self.draw_mercator_marker(cx, merc, snap_to_pixel);
                }
                ret
            },
            ProjectionView::Orthografic(ref ortho) => {
                let ret = self.draw_ortho_tiles(cx, ortho, source);
                if self.show_marker && !self.marker_layer.is_empty() {
                    self.draw_ortho_marker(cx, ortho);
                }
                if self.show_atmos {
                    self.draw_atmos(cx, ortho);
                }
                ret
            },
        }
    }

    pub fn step_zoom(&mut self, steps: i32, step_size: f64) {
        match &mut self.proj_view {
            ProjectionView::Mercator(merc) => {
                merc.step_zoom(steps, step_size);
            },
            ProjectionView::Orthografic(ortho) => {
                ortho.step_zoom(steps, step_size);
            },
        }
    }

    pub fn zoom_at(&mut self, pos: ScreenCoord, zoom_delta: f64) {
        match &mut self.proj_view {
            ProjectionView::Mercator(merc) => {
                merc.zoom_at(pos, zoom_delta)
            },
            ProjectionView::Orthografic(ortho) => {
                ortho.zoom_at(pos, zoom_delta)
            },
        }
    }

    pub fn change_tile_zoom_offset(&mut self, delta_offset: f64) {
        match &mut self.proj_view {
            ProjectionView::Mercator(merc) => {
                merc.tile_zoom_offset = (merc.tile_zoom_offset + delta_offset)
                    .max(MIN_TILE_ZOOM_OFFSET)
                    .min(MAX_TILE_ZOOM_OFFSET);
            },
            ProjectionView::Orthografic(ortho) => {
                ortho.tile_zoom_offset = (ortho.tile_zoom_offset + delta_offset)
                    .max(MIN_TILE_ZOOM_OFFSET)
                    .min(MAX_TILE_ZOOM_OFFSET);
            },
        }
    }

    //TODO Make sure to use physical pixel deltas
    pub fn move_pixel(&mut self, delta_x: f64, delta_y: f64) {
        match &mut self.proj_view {
            ProjectionView::Mercator(merc) => {
                merc.move_pixel(delta_x, delta_y);
            },
            ProjectionView::Orthografic(ortho) => {
                ortho.move_pixel(delta_x, delta_y);
            },
        }
    }

    pub fn restore_session(&mut self, session: &Session) -> Result<(), String> {
        let viewport_size = self.proj_view.viewport_size();
        let tile_size = self.proj_view.tile_size();
        match session.projection() {
            Some(Projection::Mercator) => {
                self.proj_view = ProjectionView::Mercator(
                    MercatorView::from_toml_table(&session.view, viewport_size, tile_size)?
                )
            },
            Some(Projection::Orthografic) => {
                self.proj_view = ProjectionView::Orthografic(
                    OrthograficView::from_toml_table(&session.view, viewport_size, tile_size)?
                )
            },
            None => {},
        }
        Ok(())
    }

    pub fn to_session(&self) -> Session {
        let view = match &self.proj_view {
            ProjectionView::Mercator(merc) => {
                merc.toml_table()
            },
            ProjectionView::Orthografic(ortho) => {
                ortho.toml_table()
            },
        };

        Session {
            view,
        }
    }
}
