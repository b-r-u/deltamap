use tile::Tile;
use std::path::{Path, PathBuf};


#[derive(Clone, Debug)]
pub struct TileSource {
    id: u32,
    url_template: String,
    directory: PathBuf,
    max_zoom: u32,
}

#[derive(Copy, Clone, Debug)]
pub struct TileSourceId {
    id: u32,
}

impl TileSource {
    pub fn new<S: Into<String>, P: Into<PathBuf>>(
        id: u32,
        url_template: S,
        directory: P,
        max_zoom: u32,
    ) -> Self {
        TileSource {
            id: id,
            url_template: url_template.into(),
            directory: directory.into(),
            max_zoom: max_zoom,
        }
    }

    pub fn id(&self) -> TileSourceId {
        TileSourceId {
            id: self.id,
        }
    }

    pub fn local_tile_path(&self, tile: Tile) -> PathBuf {

        let mut path = PathBuf::from(&self.directory);
        path.push(tile.zoom.to_string());
        path.push(tile.tile_x.to_string());
        path.push(tile.tile_y.to_string() + ".png");

        path
    }

    pub fn remote_tile_url(&self, tile: Tile) -> String {
        Self::fill_template(&self.url_template, tile)
    }

    pub fn max_tile_zoom(&self) -> u32 {
        self.max_zoom
    }

    fn fill_template(template: &str, tile: Tile) -> String {
        let x_str = tile.tile_x.to_string();
        let y_str = tile.tile_y.to_string();
        let z_str = tile.zoom.to_string();

        //let len = (template.len() + x_str.len() + y_str.len() + z_str.len()).saturating_sub(9);

        //TODO use the regex crate for templates or some other more elegant method
        let string = template.replacen("{x}", &x_str, 1);
        let string = string.replacen("{y}", &y_str, 1);
        let string = string.replacen("{z}", &z_str, 1);

        return string;
    }
}
