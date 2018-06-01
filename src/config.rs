use clap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tile_source::TileSource;
use toml::Value;
use xdg;

static DEFAULT_CONFIG: &'static str = "";
static DEFAULT_TILE_SOURCES: &'static str = include_str!("../default_tile_sources.toml");


#[derive(Debug)]
pub struct Config {
    tile_cache_dir: PathBuf,
    sources: Vec<(String, TileSource)>,
    fps: f64,
    use_network: bool,
    async: bool,
}

impl Config {
    //TODO use builder pattern to create config

    pub fn from_arg_matches<'a>(matches: &clap::ArgMatches<'a>) -> Result<Config, String> {
        let mut config = if let Some(config_path) = matches.value_of_os("config") {
            Config::from_toml_file(config_path)?
        } else {
            Config::find_or_create()?
        };

        if let Some(tile_sources_path) = matches.value_of_os("tile-sources") {
            config.add_tile_sources_from_file(tile_sources_path)?;
        } else {
            config.add_tile_sources_from_default_or_create()?;
        };

        config.merge_arg_matches(matches);

        Ok(config)
    }

    fn merge_arg_matches<'a>(&mut self, matches: &clap::ArgMatches<'a>) {
        if let Some(Ok(fps)) = matches.value_of("fps").map(|s| s.parse()) {
            self.fps = fps;
        }

        if matches.is_present("offline") {
            self.use_network = false;
        }

        if matches.is_present("sync") {
            self.async = false;
        }
    }

    fn find_or_create() -> Result<Config, String> {
        if let Ok(xdg_dirs) = xdg::BaseDirectories::with_prefix("deltamap") {
            if let Some(config_path) = xdg_dirs.find_config_file("config.toml") {
                info!("load config from path {:?}", config_path);

                Config::from_toml_file(config_path)
            } else {
                // try to write a default config file
                if let Ok(path) = xdg_dirs.place_config_file("config.toml") {
                    if let Ok(mut file) = File::create(&path) {
                        if file.write_all(DEFAULT_CONFIG.as_bytes()).is_ok() {
                            info!("write default config to {:?}", &path);
                        }
                    }
                }

                Config::from_toml_str(DEFAULT_CONFIG)
            }
        } else {
            info!("load default config");
            Config::from_toml_str(DEFAULT_CONFIG)
        }
    }

    fn add_tile_sources_from_default_or_create(&mut self) -> Result<(), String> {
        if let Ok(xdg_dirs) = xdg::BaseDirectories::with_prefix("deltamap") {
            if let Some(sources_path) = xdg_dirs.find_config_file("tile_sources.toml") {
                info!("load tile sources from path {:?}", sources_path);

                self.add_tile_sources_from_file(sources_path)
            } else {
                // try to write a default tile sources file
                if let Ok(path) = xdg_dirs.place_config_file("tile_sources.toml") {
                    if let Ok(mut file) = File::create(&path) {
                        if file.write_all(DEFAULT_TILE_SOURCES.as_bytes()).is_ok() {
                            info!("write default tile sources to {:?}", &path);
                        }
                    }
                }

                self.add_tile_sources_from_str(DEFAULT_TILE_SOURCES)
            }
        } else {
            info!("load default config");
            self.add_tile_sources_from_str(DEFAULT_TILE_SOURCES)
        }
    }

    /// Returns a tile cache directory path at a standard XDG cache location. The returned path may
    /// not exist.
    fn default_tile_cache_dir() -> Result<PathBuf, String> {
        let xdg_dirs = xdg::BaseDirectories::with_prefix("deltamap")
            .map_err(|e| format!("{}", e))?;

        match xdg_dirs.find_cache_file("tiles") {
            Some(dir) => Ok(dir),
            None => Ok(xdg_dirs.get_cache_home().join("tiles")),
        }
    }

    fn from_toml_str(toml_str: &str) -> Result<Config, String> {
        match toml_str.parse::<Value>() {
            Ok(Value::Table(ref table)) => {
                let tile_cache_dir = {
                    match table.get("tile_cache_dir") {
                        Some(dir) => {
                            PathBuf::from(
                                dir.as_str()
                                   .ok_or_else(|| "tile_cache_dir has to be a string".to_string())?
                            )
                        },
                        None => Config::default_tile_cache_dir()?,
                    }
                };

                let fps = {
                    match table.get("fps") {
                        Some(&Value::Float(fps)) => fps,
                        Some(&Value::Integer(fps)) => fps as f64,
                        Some(_) => return Err("fps has to be an integer or a float.".to_string()),
                        None => 60.0,
                    }
                };

                let use_network = {
                    match table.get("use_network") {
                        Some(&Value::Boolean(x)) => x,
                        Some(_) => return Err("use_network has to be a boolean.".to_string()),
                        None => true,
                    }
                };

                let async = {
                    match table.get("async") {
                        Some(&Value::Boolean(x)) => x,
                        Some(_) => return Err("async has to be a boolean.".to_string()),
                        None => true,
                    }
                };

                Ok(
                    Config {
                        tile_cache_dir,
                        sources: vec![],
                        fps,
                        use_network,
                        async,
                    }
                )
            },
            Ok(_) => Err("TOML file has invalid structure. Expected a Table as the top-level element.".to_string()),
            Err(e) => Err(format!("{}", e)),
        }
    }

    fn from_toml_file<P: AsRef<Path>>(path: P) -> Result<Config, String> {
        let mut file = File::open(path).map_err(|e| format!("{}", e))?;

        let mut content = String::new();
        file.read_to_string(&mut content).map_err(|e| format!("{}", e))?;

        Config::from_toml_str(&content)
    }

    fn add_tile_sources_from_str(&mut self, toml_str: &str) -> Result<(), String> {
        match toml_str.parse::<Value>() {
            Ok(Value::Table(ref table)) => {
                let sources_array = table.get("tile_sources")
                    .ok_or_else(|| "missing \"tile_sources\" table".to_string())?
                    .as_array()
                    .ok_or_else(|| "\"tile_sources\" has to be an array.".to_string())?;

                for (id, source) in sources_array.iter().enumerate() {
                    let name = source.get("name")
                        .ok_or_else(|| "tile_source is missing \"name\" entry.".to_string())?
                        .as_str()
                        .ok_or_else(|| "\"name\" has to be a string".to_string())?;

                    let min_zoom = source.get("min_zoom")
                        .unwrap_or_else(|| &Value::Integer(0))
                        .as_integer()
                        .ok_or_else(|| "min_zoom has to be an integer".to_string())
                        .and_then(|m| {
                            if m < 0 || m > 30 {
                                Err(format!("min_zoom = {} is out of bounds, has to be in interval [0, 30]", m))
                            } else {
                                Ok(m)
                            }
                        })?;

                    let max_zoom = source.get("max_zoom")
                        .ok_or_else(|| format!("source {:?} is missing \"max_zoom\" entry", name))?
                        .as_integer()
                        .ok_or_else(|| "max_zoom has to be an integer".to_string())
                        .and_then(|m| {
                            if m < 0 || m > 30 {
                                Err(format!("max_zoom = {} is out of bounds, has to be in interval [0, 30]", m))
                            } else {
                                Ok(m)
                            }
                        })?;

                    if min_zoom > max_zoom {
                        warn!("min_zoom ({}) and max_zoom ({}) allow no valid tiles", min_zoom, max_zoom);
                    } else if min_zoom == max_zoom {
                        warn!("min_zoom ({}) and max_zoom ({}) allow only one zoom level", min_zoom, max_zoom);
                    }

                    let url_template = source.get("url_template")
                        .ok_or_else(|| format!("source {:?} is missing \"url_template\" entry", name))?
                        .as_str()
                        .ok_or_else(|| "url_template has to be a string".to_string())?;

                    let extension = source.get("extension")
                        .ok_or_else(|| format!("source {:?} is missing \"extension\" entry", name))?
                        .as_str()
                        .ok_or_else(|| "extension has to be a string".to_string())?;

                    //TODO reduce allowed strings to a reasonable subset of valid UTF-8 strings
                    // that can also be used as a directory name or introduce a dir_name key with
                    // more restrictions.
                    if name.contains('/') || name.contains('\\') {
                        return Err(format!("source name ({:?}) must not contain slashes (\"/\" or \"\\\")", name));
                    }

                    let mut path = PathBuf::from(&self.tile_cache_dir);
                    path.push(name);

                    self.sources.push((
                        name.to_string(),
                        TileSource::new(
                            id as u32,
                            url_template.to_string(),
                            path,
                            extension.to_string(),
                            min_zoom as u32,
                            max_zoom as u32,
                        )?,
                    ));
                }
                Ok(())
            },
            Ok(_) => Err("TOML file has invalid structure. Expected a Table as the top-level element.".to_string()),
            Err(e) => Err(format!("{}", e)),
        }
    }

    fn add_tile_sources_from_file<P: AsRef<Path>>(&mut self, path: P) -> Result<(), String> {
        let mut file = File::open(path).map_err(|e| format!("{}", e))?;

        let mut content = String::new();
        file.read_to_string(&mut content).map_err(|e| format!("{}", e))?;

        self.add_tile_sources_from_str(&content)
    }

    pub fn tile_sources(&self) -> &[(String, TileSource)] {
        &self.sources
    }

    pub fn fps(&self) -> f64 {
        self.fps
    }

    pub fn use_network(&self) -> bool {
        self.use_network
    }

    pub fn async(&self) -> bool {
        self.async
    }
}

#[cfg(test)]
mod tests {
    use config::*;

    #[test]
    fn default_config() {
        let mut config = Config::from_toml_str(DEFAULT_CONFIG).unwrap();
        config.add_tile_sources_from_str(DEFAULT_TILE_SOURCES).unwrap();
    }
}
