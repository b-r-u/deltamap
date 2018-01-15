use std::error::Error;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tile_source::TileSource;
use toml::Value;
use xdg;

static DEFAULT_CONFIG: &'static str = include_str!("../default_config.toml");


pub struct Config {
    tile_cache_dir: String,
    sources: Vec<(String, TileSource)>,
}

impl Config {
    pub fn load() -> Result<Config, String> {
        if let Ok(xdg_dirs) = xdg::BaseDirectories::with_prefix("deltamap") {
            if let Some(config_path) = xdg_dirs.find_config_file("config.toml") {
                println!("Load config from path {:?}", config_path);

                Config::from_toml_file(config_path)
            } else {
                if let Ok(path) = xdg_dirs.place_config_file("config.toml") {
                    if let Ok(mut file) = File::create(&path) {
                        println!("write default config {:?}", &path);
                        file.write_all(DEFAULT_CONFIG.as_bytes());
                    }
                }

                Config::from_toml_str(DEFAULT_CONFIG)
            }
        } else {
            Config::from_toml_str(DEFAULT_CONFIG)
        }
    }

    pub fn from_toml_str(toml_str: &str) -> Result<Config, String> {
        match toml_str.parse::<Value>() {
            Ok(Value::Table(ref table)) => {
                let tile_cache_dir = table.get("tile_cache_dir")
                    .ok_or_else(|| "missing \"tile_cache_dir\" entry".to_string())?
                    .as_str()
                    .ok_or_else(|| "tile_cache_dir has to be a string".to_string())?;

                let sources_table = table.get("sources")
                    .ok_or_else(|| "missing \"sources\" table".to_string())?
                    .as_table()
                    .ok_or_else(|| "\"sources\" has to be a table".to_string())?;

                let mut sources_vec: Vec<(String, TileSource)> = Vec::with_capacity(sources_table.len());

                for (id, (name, source)) in sources_table.iter().enumerate() {
                    let max_zoom = source.get("max_zoom")
                        .ok_or_else(|| format!("source {:?} is missing \"max_zoom\" entry", name))?
                        .as_integer()
                        .ok_or_else(|| "max_zoom has to be an integer".to_string())
                        .and_then(|m| {
                            if m <= 0 || m > 30 {
                                Err(format!("max_zoom = {} is out of bounds, has to be in interval [1, 30]", m))
                            } else {
                                Ok(m)
                            }
                        })?;

                    let url_template = source.get("url_template")
                        .ok_or_else(|| format!("source {:?} is missing \"url_template\" entry", name))?
                        .as_str()
                        .ok_or_else(|| "url_template has to be a string".to_string())?;

                    let extension = source.get("extension")
                        .ok_or_else(|| format!("source {:?} is missing \"extension\" entry", name))?
                        .as_str()
                        .ok_or_else(|| "extension has to be a string".to_string())?;

                    if name.contains("/") || name.contains("\\") {
                        return Err(format!("source name ({:?}) must not contain slashes (\"/\" or \"\\\")", name));
                    }

                    let mut path = PathBuf::from(&tile_cache_dir);
                    path.push(name);

                    sources_vec.push((
                        name.clone(),
                        TileSource::new(
                            id as u32,
                            url_template.to_string(),
                            path,
                            extension.to_string(),
                            max_zoom as u32,
                        ),
                    ));
                }

                Ok(
                    Config {
                        tile_cache_dir: tile_cache_dir.to_string(),
                        sources: sources_vec,
                    }
                )
            },
            Ok(_) => Err("TOML file has invalid structure. Expected a Table as the top-level element.".to_string()),
            Err(e) => Err(e.description().to_string()),
        }
    }

    pub fn from_toml_file<P: AsRef<Path>>(path: P) -> Result<Config, String> {
        let mut file = File::open(path).map_err(|e| e.description().to_string())?;

        let mut content = String::new();
        file.read_to_string(&mut content).map_err(|e| e.description().to_string())?;

        Config::from_toml_str(&content)
    }

    pub fn tile_sources(&self) -> &[(String, TileSource)] {
        &self.sources
    }
}
