use serde_derive;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use tile_source::TileSource;
use toml;


#[derive(Deserialize, Clone, Debug)]
pub struct Config {
    tile_cache_dir: String,
    sources: BTreeMap<String, Source>,
}

#[derive(Deserialize, Clone, Debug)]
struct Source {
    max_zoom: u32,
    url_template: String,
}

impl Config {
    pub fn from_toml<P: AsRef<Path>>(path: P) -> Option<Config> {
        let mut file = match File::open(path) {
            Ok(file) => file,
            Err(_) => return None,
        };

        let mut content = String::new();
        if file.read_to_string(&mut content).is_err() {
            return None;
        }

        toml::from_str(&content).ok()
    }

    pub fn tile_sources(&self) -> BTreeMap<String, TileSource> {
        let mut map = BTreeMap::new();

        for (id, (name, source)) in self.sources.iter().enumerate() {
            let mut path = PathBuf::from(&self.tile_cache_dir);
            //TODO escape name (no slashes or dots)
            path.push(name);

            map.insert(
                name.clone(),
                TileSource::new(
                    id as u32,
                    source.url_template.clone(),
                    path,
                    source.max_zoom,
                ),
            );
        }

        return map;
    }
}
