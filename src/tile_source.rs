use coord::TileCoord;
use std::path::PathBuf;


#[derive(Clone, Debug)]
pub struct TileSource {
    id: u32,
    url_template: String,
    directory: PathBuf,
    max_zoom: u32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
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

    pub fn local_tile_path(&self, tile_coord: TileCoord) -> PathBuf {
        let mut path = PathBuf::from(&self.directory);
        path.push(tile_coord.zoom.to_string());
        path.push(tile_coord.x.to_string());
        path.push(tile_coord.y.to_string() + ".png");

        path
    }

    pub fn remote_tile_url(&self, tile_coord: TileCoord) -> String {
        Self::fill_template(&self.url_template, tile_coord)
    }

    pub fn max_tile_zoom(&self) -> u32 {
        self.max_zoom
    }

    fn fill_template(template: &str, tile_coord: TileCoord) -> String {
        let x_str = tile_coord.x.to_string();
        let y_str = tile_coord.y.to_string();
        let z_str = tile_coord.zoom.to_string();

        //TODO use the regex crate for templates or some other more elegant method
        template.replacen("{x}", &x_str, 1)
                .replacen("{y}", &y_str, 1)
                .replacen("{z}", &z_str, 1)
    }
}
