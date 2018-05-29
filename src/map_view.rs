use coord::{MapCoord, ScreenCoord, ScreenRect, TileCoord};


/// A view of a tiled map with a rectangular viewport and a zoom.
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

/// The position and size of a specific tile on the screen.
#[derive(Clone, Debug)]
pub struct VisibleTile {
    pub tile: TileCoord,
    pub rect: ScreenRect,
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

    /// Constructs a new `MapView` centered at Null Island with an integer zoom that fills a screen
    /// with the given dimensions.
    pub fn with_filling_zoom(width: f64, height: f64, tile_size: u32) -> MapView {
        let min_dimension = width.min(height);
        let zoom = (min_dimension / f64::from(tile_size)).log2().ceil();
        MapView {
            width,
            height,
            tile_size,
            center: MapCoord::new(0.5, 0.5),
            zoom,
            tile_zoom_offset: 0.0,
        }
    }

    /// Returns the map coordinate that corresponds to the top-left corner of the viewport.
    pub fn top_left_coord(&self) -> MapCoord {
        let scale = f64::powf(2.0, -self.zoom) / f64::from(self.tile_size);

        let x = self.center.x + -0.5 * self.width * scale;
        let y = self.center.y + -0.5 * self.height * scale;

        MapCoord::new(x, y)
    }

    /// Returns the screen coordinate that corresponds to the given map coordinate.
    pub fn map_to_screen_coord(&self, map_coord: MapCoord) -> ScreenCoord {
        let scale = f64::powf(2.0, self.zoom) * f64::from(self.tile_size);

        let delta_x = map_coord.x - self.center.x;
        let delta_y = map_coord.y - self.center.y;

        ScreenCoord {
            x: 0.5 * self.width + delta_x * scale,
            y: 0.5 * self.height + delta_y * scale,
        }
    }

    /// Returns the screen coordinate of the top-left corner of a tile.
    pub fn tile_screen_position(&self, tile: &TileCoord) -> ScreenCoord {
        self.map_to_screen_coord(tile.map_coord_north_west())
    }

    /// Returns a `Vec` of all tiles that are visible in the current viewport.
    pub fn visible_tiles(&self, snap_to_pixel: bool) -> Vec<VisibleTile> {
        let uzoom = self.tile_zoom();
        let top_left_tile = self.top_left_coord().on_tile_at_zoom(uzoom);
        let mut top_left_tile_screen_coord = self.tile_screen_position(&top_left_tile);
        let tile_screen_size = f64::powf(2.0, self.zoom - f64::from(uzoom)) * f64::from(self.tile_size);

        if snap_to_pixel {
            top_left_tile_screen_coord.snap_to_pixel();
        }

        let start_tile_x = top_left_tile.x;
        let start_tile_y = top_left_tile.y;
        let num_tiles_x = ((self.width - top_left_tile_screen_coord.x) / tile_screen_size).ceil().max(0.0) as i32;
        let num_tiles_y = ((self.height - top_left_tile_screen_coord.y) / tile_screen_size).ceil().max(0.0) as i32;

        let mut visible_tiles = Vec::with_capacity(num_tiles_x as usize * num_tiles_y as usize);

        for y in 0..num_tiles_y {
            for x in 0..num_tiles_x {
                let t = TileCoord::new(uzoom, start_tile_x + x, start_tile_y + y);
                if t.is_on_planet() {
                    visible_tiles.push(
                        VisibleTile {
                            tile: t,
                            rect: ScreenRect {
                                x: top_left_tile_screen_coord.x + tile_screen_size * f64::from(x),
                                y: top_left_tile_screen_coord.y + tile_screen_size * f64::from(y),
                                width: tile_screen_size,
                                height: tile_screen_size,
                            }
                        }
                    );
                }
            }
        }

        visible_tiles
    }

    /// Returns the tile zoom value that is used for rendering with the current zoom.
    pub fn tile_zoom(&self) -> u32 {
        (self.zoom + self.tile_zoom_offset).floor().max(0.0) as u32
    }

    /// Returns the tile zoom offset.
    pub fn tile_zoom_offset(&self) -> f64 {
        self.tile_zoom_offset
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
        self.zoom = zoom;
    }

    /// Change zoom value by `zoom_delta`.
    pub fn zoom(&mut self, zoom_delta: f64) {
        self.zoom += zoom_delta;
    }

    /// Change zoom value by `zoom_delta` and zoom to a position given in screen coordinates.
    pub fn zoom_at(&mut self, pos: ScreenCoord, zoom_delta: f64) {
        let delta_x = pos.x - self.width * 0.5;
        let delta_y = pos.y - self.height * 0.5;

        let scale =
            (f64::powf(2.0, -self.zoom) - f64::powf(2.0, -self.zoom - zoom_delta))
            / f64::from(self.tile_size);
        self.zoom += zoom_delta;

        self.center.x += delta_x * scale;
        self.center.y += delta_y * scale;
    }

    /// Set a zoom value and zoom to a `position` given in screen coordinates.
    pub fn set_zoom_at(&mut self, pos: ScreenCoord, zoom: f64) {
        let delta_x = pos.x - self.width * 0.5;
        let delta_y = pos.y - self.height * 0.5;

        let scale = (f64::powf(2.0, -self.zoom) - f64::powf(2.0, -zoom)) / f64::from(self.tile_size);
        self.zoom = zoom;

        self.center.x += delta_x * scale;
        self.center.y += delta_y * scale;
    }

    /// Move the center of the viewport by (`delta_x`, `delta_y`) in screen coordinates.
    pub fn move_pixel(&mut self, delta_x: f64, delta_y: f64) {
        let scale = f64::powf(2.0, -self.zoom) / f64::from(self.tile_size);
        self.center.x += delta_x * scale;
        self.center.y += delta_y * scale;
    }
}
