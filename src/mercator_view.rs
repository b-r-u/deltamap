use cgmath::{vec2, Vector2};
use coord::{MapCoord, ScreenCoord, ScreenRect, TextureRect, TileCoord};
use orthografic_view::OrthograficView;
use projection::Projection;
use toml::Value;
use toml::value::Table;


pub const MIN_ZOOM_LEVEL: f64 = 0.0;
pub const MAX_ZOOM_LEVEL: f64 = 22.0;

/// A view of a tiled map with a rectangular viewport and a zoom.
/// Projection: EPSG:3857: WGS 84 / Pseudo-Mercator
#[derive(Clone, Debug)]
pub struct MercatorView {
    /// Size of the viewport in physical pixels.
    pub viewport_size: Vector2<f64>,
    /// Size of each square tile in the same unit as the viewport dimensions.
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

#[derive(Clone, Debug)]
pub struct TexturedVisibleTile {
    pub screen_rect: ScreenRect,
    pub tex_rect: TextureRect,
    pub tex_minmax: TextureRect,
}


impl MercatorView {
    /// Constructs a `MapView` centered at Null Island with an integer zoom that fills a screen
    /// with the given dimensions.
    pub fn initial_view(width: f64, height: f64, tile_size: u32) -> MercatorView {
        let min_dimension = width.min(height);
        let zoom = (min_dimension / f64::from(tile_size)).log2().ceil()
            .max(MIN_ZOOM_LEVEL)
            .min(MAX_ZOOM_LEVEL);
        MercatorView {
            viewport_size: vec2(width, height),
            tile_size,
            center: MapCoord::new(0.5, 0.5),
            zoom,
            tile_zoom_offset: 0.0,
        }
    }

    pub fn from_orthografic_view(ortho: &OrthograficView) -> Self {
        let latlon = ortho.center.to_latlon_rad();
        let zoom_delta = (1.0 / latlon.lat.cos()).log2();

        MercatorView {
            viewport_size: ortho.viewport_size,
            tile_size: ortho.tile_size,
            center: ortho.center,
            zoom: ortho.zoom - zoom_delta,
            tile_zoom_offset: ortho.tile_zoom_offset,
        }
    }

    pub fn from_toml_table(
        table: &Table,
        viewport_size: Vector2<f64>,
        tile_size: u32,
    ) -> Result<Self, String> {
        let x = match table.get("x") {
            Some(&Value::Float(x)) => x,
            Some(&Value::Integer(x)) => x as f64,
            Some(_) => return Err("x has to be a number.".to_string()),
            None => return Err("x position is missing.".to_string()),
        };

        let y = match table.get("y") {
            Some(&Value::Float(y)) => y,
            Some(&Value::Integer(y)) => y as f64,
            Some(_) => return Err("y has to be a number.".to_string()),
            None => return Err("y position is missing.".to_string()),
        };

        let zoom = match table.get("zoom") {
            Some(&Value::Float(z)) => z,
            Some(&Value::Integer(z)) => z as f64,
            Some(_) => return Err("zoom has to be a number.".to_string()),
            None => return Err("zoom value is missing.".to_string()),
        }.min(MAX_ZOOM_LEVEL).max(MIN_ZOOM_LEVEL);

        if let Some(&Value::String(ref s)) = table.get("projection") {
            if s != "mercator" {
                return Err("try to deserialize wrong projection".to_string());
            }
        }

        Ok(MercatorView {
            viewport_size,
            tile_size,
            center: MapCoord::new(x, y),
            zoom,
            tile_zoom_offset: 0.0,
        })
    }

    pub fn toml_table(&self) -> Table {
        let mut table = Table::new();
        table.insert("projection".to_string(), Value::String(Self::projection().to_str().to_string()));
        table.insert("x".to_string(), Value::Float(self.center.x));
        table.insert("y".to_string(), Value::Float(self.center.y));
        table.insert("zoom".to_string(), Value::Float(self.zoom));
        table
    }

    pub fn projection() -> Projection {
        Projection::Mercator
    }

    /// Returns the map coordinate that corresponds to the top-left corner of the viewport.
    pub fn top_left_coord(&self) -> MapCoord {
        let scale = f64::powf(2.0, -self.zoom) / f64::from(self.tile_size);

        let x = self.center.x + -0.5 * self.viewport_size.x * scale;
        let y = self.center.y + -0.5 * self.viewport_size.y * scale;

        MapCoord::new(x, y)
    }

    /// Returns the screen coordinate that corresponds to the given map coordinate.
    pub fn map_to_screen_coord(&self, map_coord: MapCoord) -> ScreenCoord {
        let scale = f64::powf(2.0, self.zoom) * f64::from(self.tile_size);

        let delta_x = map_coord.x - self.center.x;
        let delta_y = map_coord.y - self.center.y;

        ScreenCoord {
            x: 0.5 * self.viewport_size.x + delta_x * scale,
            y: 0.5 * self.viewport_size.y + delta_y * scale,
        }
    }

    /// Returns the map coordinate that corresponds to the given screen coordinate.
    pub fn screen_to_map_coord(&self, screen_coord: ScreenCoord) -> MapCoord {
        let scale = f64::powf(2.0, -self.zoom) / f64::from(self.tile_size);

        let delta_x = screen_coord.x - self.viewport_size.x * 0.5;
        let delta_y = screen_coord.y - self.viewport_size.y * 0.5;

        let mut m = MapCoord {
            x: self.center.x + delta_x * scale,
            y: self.center.y + delta_y * scale,
        };

        m.normalize_x();
        m
    }

    /// Returns true if the viewport rectangle is fully inside the map.
    pub fn covers_viewport(&self) -> bool {
        let scale = f64::powf(2.0, -self.zoom) / f64::from(self.tile_size);

        let y_top = self.center.y + -0.5 * self.viewport_size.x * scale;
        let y_bottom = self.center.y + 0.5 * self.viewport_size.y * scale;

        y_top >= 0.0 && y_bottom <= 1.0
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
        let tile_screen_size = f64::powf(2.0, self.zoom - f64::from(uzoom)) *
            f64::from(self.tile_size);

        if snap_to_pixel {
            top_left_tile_screen_coord.snap_to_pixel();
        }

        let start_tile_x = top_left_tile.x;
        let start_tile_y = top_left_tile.y;
        let num_tiles_x = ((self.viewport_size.x - top_left_tile_screen_coord.x) /
                           tile_screen_size).ceil().max(0.0) as i32;
        let num_tiles_y = ((self.viewport_size.y - top_left_tile_screen_coord.y) /
                           tile_screen_size).ceil().max(0.0) as i32;

        let mut visible_tiles = Vec::with_capacity(num_tiles_x as usize * num_tiles_y as usize);

        for y in 0..num_tiles_y {
            for x in 0..num_tiles_x {
                let t = TileCoord::new(uzoom, start_tile_x + x, start_tile_y + y);
                if t.is_valid() {
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

    /// Change zoom value by `zoom_delta` and zoom to a position given in screen coordinates.
    pub fn zoom_at(&mut self, pos: ScreenCoord, zoom_delta: f64) {
        let new_zoom = self.zoom + zoom_delta;
        self.set_zoom_at(pos, new_zoom);
    }

    /// Set a zoom value and zoom to a `position` given in screen coordinates.
    pub fn set_zoom_at(&mut self, pos: ScreenCoord, new_zoom: f64) {
        let new_zoom = new_zoom.min(MAX_ZOOM_LEVEL).max(MIN_ZOOM_LEVEL);

        let delta_x = pos.x - self.viewport_size.x * 0.5;
        let delta_y = pos.y - self.viewport_size.y * 0.5;

        let scale = (f64::powf(2.0, -self.zoom) - f64::powf(2.0, -new_zoom)) /
            f64::from(self.tile_size);

        self.zoom = new_zoom;
        self.center.x += delta_x * scale;
        self.center.y += delta_y * scale;
        self.center.normalize_xy();
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

    /// Move the center of the viewport by (`delta_x`, `delta_y`) in screen coordinates.
    pub fn move_pixel(&mut self, delta_x: f64, delta_y: f64) {
        let scale = f64::powf(2.0, -self.zoom) / f64::from(self.tile_size);
        self.center.x += delta_x * scale;
        self.center.y += delta_y * scale;
        self.center.normalize_xy();
    }
}
