use coord::{MapCoord, ScreenCoord, ScreenRect, TextureRect, TileCoord};
use map_view::MapView;


/// A view of a tiled map with a rectangular viewport and a zoom.
#[derive(Clone, Debug)]
pub struct MercatorView {
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
    pub fn initial_map_view(width: f64, height: f64, tile_size: u32) -> MapView {
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
    pub fn top_left_coord(map_view: &MapView) -> MapCoord {
        let scale = f64::powf(2.0, -map_view.zoom) / f64::from(map_view.tile_size);

        let x = map_view.center.x + -0.5 * map_view.width * scale;
        let y = map_view.center.y + -0.5 * map_view.height * scale;

        MapCoord::new(x, y)
    }

    /// Returns the screen coordinate that corresponds to the given map coordinate.
    pub fn map_to_screen_coord(map_view: &MapView, map_coord: MapCoord) -> ScreenCoord {
        let scale = f64::powf(2.0, map_view.zoom) * f64::from(map_view.tile_size);

        let delta_x = map_coord.x - map_view.center.x;
        let delta_y = map_coord.y - map_view.center.y;

        ScreenCoord {
            x: 0.5 * map_view.width + delta_x * scale,
            y: 0.5 * map_view.height + delta_y * scale,
        }
    }

    /// Returns the map coordinate that corresponds to the given screen coordinate.
    pub fn screen_to_map_coord(map_view: &MapView, screen_coord: ScreenCoord) -> MapCoord {
        let scale = f64::powf(2.0, -map_view.zoom) / f64::from(map_view.tile_size);

        let delta_x = screen_coord.x - map_view.width * 0.5;
        let delta_y = screen_coord.y - map_view.height * 0.5;

        let mut m = MapCoord {
            x: map_view.center.x + delta_x * scale,
            y: map_view.center.y + delta_y * scale,
        };

        m.normalize_x();
        m
    }

    /// Returns true if the viewport rectangle is fully inside the map.
    pub fn covers_viewport(map_view: &MapView) -> bool {
        let scale = f64::powf(2.0, -map_view.zoom) / f64::from(map_view.tile_size);

        let y_top = map_view.center.y + -0.5 * map_view.height * scale;
        let y_bottom = map_view.center.y + 0.5 * map_view.height * scale;

        y_top >= 0.0 && y_bottom <= 1.0
    }

    /// Returns the screen coordinate of the top-left corner of a tile.
    pub fn tile_screen_position(map_view: &MapView, tile: &TileCoord) -> ScreenCoord {
        Self::map_to_screen_coord(map_view, tile.map_coord_north_west())
    }

    /// Returns a `Vec` of all tiles that are visible in the current viewport.
    pub fn visible_tiles(map_view: &MapView, snap_to_pixel: bool) -> Vec<VisibleTile> {
        let uzoom = Self::tile_zoom(map_view);
        let top_left_tile = Self::top_left_coord(map_view).on_tile_at_zoom(uzoom);
        let mut top_left_tile_screen_coord = Self::tile_screen_position(map_view, &top_left_tile);
        let tile_screen_size = f64::powf(2.0, map_view.zoom - f64::from(uzoom)) *
            f64::from(map_view.tile_size);

        if snap_to_pixel {
            top_left_tile_screen_coord.snap_to_pixel();
        }

        let start_tile_x = top_left_tile.x;
        let start_tile_y = top_left_tile.y;
        let num_tiles_x = ((map_view.width - top_left_tile_screen_coord.x) /
                           tile_screen_size).ceil().max(0.0) as i32;
        let num_tiles_y = ((map_view.height - top_left_tile_screen_coord.y) /
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
    pub fn tile_zoom(map_view: &MapView) -> u32 {
        (map_view.zoom + map_view.tile_zoom_offset).floor().max(0.0) as u32
    }

    /// Change zoom value by `zoom_delta` and zoom to a position given in screen coordinates.
    pub fn zoom_at(map_view: &mut MapView, pos: ScreenCoord, zoom_delta: f64) {
        let delta_x = pos.x - map_view.width * 0.5;
        let delta_y = pos.y - map_view.height * 0.5;

        let scale = (f64::powf(2.0, -map_view.zoom) - f64::powf(2.0, -map_view.zoom - zoom_delta))
            / f64::from(map_view.tile_size);

        map_view.zoom += zoom_delta;
        map_view.center.x += delta_x * scale;
        map_view.center.y += delta_y * scale;
    }

    /// Set a zoom value and zoom to a `position` given in screen coordinates.
    pub fn set_zoom_at(map_view: &mut MapView, pos: ScreenCoord, zoom: f64) {
        let delta_x = pos.x - map_view.width * 0.5;
        let delta_y = pos.y - map_view.height * 0.5;

        let scale = (f64::powf(2.0, -map_view.zoom) - f64::powf(2.0, -zoom)) /
            f64::from(map_view.tile_size);

        map_view.zoom = zoom;
        map_view.center.x += delta_x * scale;
        map_view.center.y += delta_y * scale;
    }

    /// Move the center of the viewport by (`delta_x`, `delta_y`) in screen coordinates.
    pub fn move_pixel(map_view: &mut MapView, delta_x: f64, delta_y: f64) {
        let scale = f64::powf(2.0, -map_view.zoom) / f64::from(map_view.tile_size);
        map_view.center.x += delta_x * scale;
        map_view.center.y += delta_y * scale;
    }
}
