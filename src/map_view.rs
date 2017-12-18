use coord::{MapCoord, ScreenCoord, ScreenRect};
use tile::Tile;


#[derive(Clone, Debug)]
pub struct MapView {
    pub width: f64,
    pub height: f64,
    pub tile_size: u32,
    pub center: MapCoord,
    pub zoom2: f64,
}

#[derive(Clone, Debug)]
pub struct VisibleTile {
    pub tile: Tile,
    pub rect: ScreenRect,
}

impl MapView {
    pub fn new(width: f64, height: f64, tile_size: u32) -> MapView {
        MapView {
            width: width,
            height: height,
            tile_size: tile_size,
            center: MapCoord::new(0.5, 0.5),
            zoom2: 0.0,
        }
    }

    pub fn top_left_coord(&self) -> MapCoord {
        let scale = f64::powf(2.0, -self.zoom2) / f64::from(self.tile_size);

        let x = self.center.x + -0.5 * self.width * scale;
        let y = self.center.y + -0.5 * self.height * scale;

        MapCoord::new(x, y)
    }

    pub fn map_to_screen_coord(&self, map_coord: MapCoord) -> ScreenCoord {
        let scale = f64::powf(2.0, self.zoom2) * f64::from(self.tile_size);

        let delta_x = map_coord.x - self.center.x;
        let delta_y = map_coord.y - self.center.y;

        ScreenCoord {
            x: 0.5 * self.width + delta_x * scale,
            y: 0.5 * self.height + delta_y * scale,
        }
    }

    pub fn tile_screen_position(&self, tile: &Tile) -> ScreenCoord {
        self.map_to_screen_coord(tile.map_coord())
    }

    pub fn visible_tiles(&self, snap_to_pixel: bool) -> Vec<VisibleTile> {
        let uzoom = self.zoom2.floor().max(0.0) as u32;
        let top_left_tile = self.top_left_coord().on_tile_at_zoom(uzoom);
        let mut top_left_tile_screen_coord = self.tile_screen_position(&top_left_tile);
        let tile_screen_size = f64::powf(2.0, self.zoom2 - f64::from(uzoom)) * f64::from(self.tile_size);

        if snap_to_pixel {
            top_left_tile_screen_coord.snap_to_pixel();
        }

        let start_tile_x = top_left_tile.tile_x;
        let start_tile_y = top_left_tile.tile_y;
        let num_tiles_x = ((self.width - top_left_tile_screen_coord.x) / tile_screen_size).ceil().max(0.0) as i32;
        let num_tiles_y = ((self.height - top_left_tile_screen_coord.y) / tile_screen_size).ceil().max(0.0) as i32;

        let mut visible_tiles = Vec::with_capacity(num_tiles_x as usize * num_tiles_y as usize);

        for y in 0..num_tiles_y {
            for x in 0..num_tiles_x {
                let t = Tile::new(uzoom, start_tile_x + x, start_tile_y + y);
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

    pub fn set_size(&mut self, width: f64, height: f64) {
        self.width = width;
        self.height = height;
    }

    pub fn set_zoom(&mut self, zoom: f64) {
        self.zoom2 = zoom;
    }

    pub fn zoom(&mut self, zoom_delta: f64) {
        self.zoom2 += zoom_delta;
    }

    pub fn zoom_at(&mut self, pos: ScreenCoord, zoom_delta: f64) {
        let delta_x = pos.x - self.width * 0.5;
        let delta_y = pos.y - self.height * 0.5;

        let scale =
            (f64::powf(2.0, -self.zoom2) - f64::powf(2.0, -self.zoom2 - zoom_delta))
            / f64::from(self.tile_size);
        self.zoom2 += zoom_delta;

        self.center.x += delta_x * scale;
        self.center.y += delta_y * scale;
    }

    pub fn set_zoom_at(&mut self, pos: ScreenCoord, zoom: f64) {
        let delta_x = pos.x - self.width * 0.5;
        let delta_y = pos.y - self.height * 0.5;

        let scale = (f64::powf(2.0, -self.zoom2) - f64::powf(2.0, -zoom)) / f64::from(self.tile_size);
        self.zoom2 = zoom;

        self.center.x += delta_x * scale;
        self.center.y += delta_y * scale;
    }

    pub fn move_pixel(&mut self, delta_x: f64, delta_y: f64) {
        let scale = f64::powf(2.0, -self.zoom2) / f64::from(self.tile_size);
        self.center.x += delta_x * scale;
        self.center.y += delta_y * scale;
    }
}
