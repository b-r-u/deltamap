use coord::MapCoord;


pub const MIN_ZOOM_LEVEL: f64 = 0.0;
pub const MAX_ZOOM_LEVEL: f64 = 22.0;

/// A view of a map with a rectangular viewport and a zoom.
#[derive(Clone, Debug)]
pub struct MapView {
    /// Width of the viewport.
    pub width: f64,
    /// Height of the viewport.
    pub height: f64,
    /// Size of each square tile in the same unit as the viewport dimensions (usually pixels).
    pub tile_size: u32,
    /// The `MapCoord` that corresponds to the center of the viewport.
    pub center: MapCoord,
    /// The zoom value. The zoom factor is given by 2.0.powf(zoom);
    pub zoom: f64,
    /// Tiles only exist for integer zoom values. The tile zoom value that is used for rendering
    /// is computed by the `tile_zoom` method. Increasing `tile_zoom_offset` increases the number
    /// of visible tiles for a given zoom value.
    pub tile_zoom_offset: f64,
}

impl MapView {
    /// Constructs a new `MapView`.
    pub fn new(width: f64, height: f64, tile_size: u32, center: MapCoord, zoom: f64) -> MapView {
        MapView {
            width,
            height,
            tile_size,
            center,
            zoom,
            tile_zoom_offset: 0.0,
        }
    }

    /// Returns the tile zoom offset.
    pub fn tile_zoom_offset(map_view: &MapView) -> f64 {
        map_view.tile_zoom_offset
    }

    /// Set the tile zoom offset.
    pub fn set_tile_zoom_offset(&mut self, offset: f64) {
        self.tile_zoom_offset = offset;
    }

    /// Set the viewport size.
    pub fn set_size(&mut self, width: f64, height: f64) {
        self.width = width;
        self.height = height;
    }

    /// Set the zoom value.
    pub fn set_zoom(&mut self, zoom: f64) {
        self.zoom = zoom
            .max(MIN_ZOOM_LEVEL)
            .min(MAX_ZOOM_LEVEL);
    }

    /// Change zoom value by `zoom_delta`.
    pub fn zoom(&mut self, zoom_delta: f64) {
        self.zoom = (self.zoom + zoom_delta)
            .max(MIN_ZOOM_LEVEL)
            .min(MAX_ZOOM_LEVEL);
    }

    pub fn step_zoom(&mut self, steps: i32, step_size: f64) {
        self.zoom = {
            let z = (self.zoom + f64::from(steps) * step_size) / step_size;
            if steps > 0 {
                z.ceil() * step_size
            } else {
                z.floor() * step_size
            }
        }.max(MIN_ZOOM_LEVEL).min(MAX_ZOOM_LEVEL);
    }
}
