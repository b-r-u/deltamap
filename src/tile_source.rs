use coord::TileCoord;
use std::path::PathBuf;
use url_template::UrlTemplate;


#[derive(Debug)]
pub struct TileSource {
    id: u32,
    url_template: UrlTemplate,
    directory: PathBuf,
    extension: String,
    min_zoom: u32,
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
        extension: String,
        min_zoom: u32,
        max_zoom: u32,
    ) -> Result<Self, String> {
        Ok(TileSource {
            id,
            url_template: UrlTemplate::new(url_template)?,
            directory: directory.into(),
            extension,
            min_zoom,
            max_zoom,
        })
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
        path.push(tile_coord.y.to_string() + "." + &self.extension);

        path
    }

    pub fn remote_tile_url(&self, tile_coord: TileCoord) -> Option<String> {
        self.url_template.fill(tile_coord)
    }

    pub fn min_tile_zoom(&self) -> u32 {
        self.min_zoom
    }

    pub fn max_tile_zoom(&self) -> u32 {
        self.max_zoom
    }
}
