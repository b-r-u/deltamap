use clap;
use directories::ProjectDirs;
use query::QueryArgs;
use session::Session;
use std::fmt::Debug;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tile_source::TileSource;
use toml::Value;

static DEFAULT_CONFIG: &'static str = "";
static DEFAULT_TILE_SOURCES: &'static str = include_str!("../default_tile_sources.toml");

lazy_static! {
    static ref PROJ_DIRS: Option<ProjectDirs> = ProjectDirs::from("", "", "DeltaMap");
}

fn proj_dirs_result() -> Result<&'static ProjectDirs, String> {
    PROJ_DIRS.as_ref().ok_or_else(|| "could not retrieve project directories".to_string())
}


#[derive(Debug)]
pub struct Config {
    config_file_path: Option<PathBuf>,
    tile_sources_file_path: Option<PathBuf>,
    tile_cache_dir: PathBuf,
    sources: Vec<(String, TileSource)>,
    pbf_path: Option<PathBuf>,
    search_pattern: Option<String>,
    keyval: Vec<(String, String)>,
    keyvalregex: Vec<(String, String)>,
    fps: f64,
    use_network: bool,
    async: bool,
    open_last_session: bool,
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

        if let Some(os_path) = matches.value_of_os("pbf") {
            let path = PathBuf::from(os_path);
            if path.is_file() {
                config.pbf_path = Some(path);
            } else {
                return Err(format!("PBF file does not exist: {:?}", os_path));
            }
        }

        config.merge_arg_matches(matches);

        Ok(config)
    }

    fn merge_arg_matches<'a>(&mut self, matches: &clap::ArgMatches<'a>) {
        self.search_pattern = matches.value_of("search").map(|s| s.to_string());

        self.keyval = matches.values_of("keyval").map_or_else(
            || vec![],
            |mut kv| {
                let mut vec = vec![];
                loop {
                    if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
                        vec.push((k.to_string(), v.to_string()));
                    } else {
                        break;
                    }
                }
                vec
            },
        );

        self.keyvalregex = matches.values_of("keyvalregex").map_or_else(
            || vec![],
            |mut kv| {
                let mut vec = vec![];
                loop {
                    if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
                        vec.push((k.to_string(), v.to_string()));
                    } else {
                        break;
                    }
                }
                vec
            },
        );

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
        let config_dir = proj_dirs_result()?.config_dir();
        let config_file = {
            let mut path = PathBuf::from(config_dir);
            path.push("config.toml");
            path
        };

        if config_file.is_file() {
            info!("load config from path {:?}", config_file);

            Config::from_toml_file(config_file)
        } else {
            // try to write a default config file

            match create_config_file(
                config_dir,
                &config_file,
                DEFAULT_CONFIG.as_bytes()
            ) {
                Err(err) => {
                    warn!("{}", err);
                    Config::from_toml_str::<&str>(DEFAULT_CONFIG, None)
                },
                Ok(()) => {
                    info!("create default config file {:?}", config_file);
                    Config::from_toml_str(DEFAULT_CONFIG, Some(config_file))
                },
            }

        }
    }

    fn add_tile_sources_from_default_or_create(&mut self) -> Result<(), String> {
        let config_dir = proj_dirs_result()?.config_dir();
        let sources_file = {
            let mut path = PathBuf::from(config_dir);
            path.push("tile_sources.toml");
            path
        };

        if sources_file.is_file() {
            info!("load tile sources from path {:?}", sources_file);

            self.add_tile_sources_from_file(sources_file)
        } else {
            // try to write a default config file

            match create_config_file(
                config_dir,
                &sources_file,
                DEFAULT_TILE_SOURCES.as_bytes()
            ) {
                Err(err) => {
                    warn!("{}", err);
                    self.add_tile_sources_from_str::<&str>(DEFAULT_TILE_SOURCES, None)
                },
                Ok(()) => {
                    info!("create default tile sources file {:?}", sources_file);
                    self.add_tile_sources_from_str(DEFAULT_TILE_SOURCES, Some(sources_file))
                },
            }

        }
    }

    fn from_toml_str<P: AsRef<Path>>(toml_str: &str, config_path: Option<P>) -> Result<Config, String> {
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
                        None => {
                            let mut path = PathBuf::from(proj_dirs_result()?.cache_dir());
                            path.push("tiles");
                            path
                        },
                    }
                };

                let pbf_path = {
                    match table.get("pbf_file") {
                        Some(&Value::String(ref pbf_file)) => {
                            match config_path.as_ref() {
                                Some(config_path) => {
                                    let p = config_path.as_ref().parent()
                                        .ok_or_else(|| "root path is not a valid config file.")?;
                                    let mut p = PathBuf::from(p);
                                    p.push(pbf_file);
                                    p = p.canonicalize().
                                        map_err(|e| format!("pbf_file ({:?}): {}", p, e))?;
                                    Some(p)
                                },
                                None => Some(PathBuf::from(pbf_file)),
                            }
                        },
                        Some(_) => {
                            return Err("pbf_file has to be a string.".to_string());
                        },
                        None => None,
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

                let open_last_session = {
                    match table.get("open_last_session") {
                        Some(&Value::Boolean(x)) => x,
                        Some(_) => return Err("open_last_session has to be a boolean.".to_string()),
                        None => false,
                    }
                };

                Ok(
                    Config {
                        config_file_path: config_path.map(|p| PathBuf::from(p.as_ref())),
                        tile_sources_file_path: None,
                        tile_cache_dir,
                        sources: vec![],
                        pbf_path,
                        search_pattern: None,
                        keyval: vec![],
                        keyvalregex: vec![],
                        fps,
                        use_network,
                        async,
                        open_last_session,
                    }
                )
            },
            Ok(_) => Err("TOML file has invalid structure. Expected a Table as the top-level element.".to_string()),
            Err(e) => Err(format!("{}", e)),
        }
    }

    fn from_toml_file<P: AsRef<Path>>(path: P) -> Result<Config, String> {
        let mut file = File::open(&path).map_err(|e| format!("{}", e))?;

        let mut content = String::new();
        file.read_to_string(&mut content).map_err(|e| format!("{}", e))?;

        Config::from_toml_str(&content, Some(path))
    }

    fn add_tile_sources_from_str<P>(
        &mut self,
        toml_str: &str,
        file_path: Option<P>
        ) -> Result<(), String>
        where P: AsRef<Path>
    {
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

                self.tile_sources_file_path = file_path.map(|p| PathBuf::from(p.as_ref()));
                Ok(())
            },
            Ok(_) => Err("TOML file has invalid structure. Expected a Table as the top-level element.".to_string()),
            Err(e) => Err(format!("{}", e)),
        }
    }

    fn add_tile_sources_from_file<P: AsRef<Path>>(&mut self, path: P) -> Result<(), String> {
        let mut file = File::open(&path).map_err(|e| format!("{}", e))?;

        let mut content = String::new();
        file.read_to_string(&mut content).map_err(|e| format!("{}", e))?;

        self.add_tile_sources_from_str(&content, Some(path))
    }

    pub fn list_paths(&self) {
        let config = match self.config_file_path.as_ref() {
            Some(path) => format!("{:?}", path),
            None => "<None>".to_string(),
        };

        let sources = match self.tile_sources_file_path.as_ref() {
            Some(path) => format!("{:?}", path),
            None => "<None>".to_string(),
        };

        let pbf = match self.pbf_path.as_ref() {
            Some(path) => format!("{:?}", path),
            None => "<None>".to_string(),
        };

        println!("\
            main configuration file: {}\n\
            tile sources file:       {}\n\
            tile cache directory:    {:?}\n\
            OSM PBF file:            {}",
            config,
            sources,
            self.tile_cache_dir,
            pbf,
        );
    }

    pub fn tile_sources(&self) -> &[(String, TileSource)] {
        &self.sources
    }

    pub fn pbf_path(&self) -> Option<&Path> {
        self.pbf_path.as_ref().map(|p| p.as_path())
    }

    pub fn search_pattern(&self) -> Option<&str> {
        self.search_pattern.as_ref().map(|s| s.as_str())
    }

    pub fn keyval(&self) -> &[(String, String)] {
        self.keyval.as_slice()
    }

    pub fn keyvalregex(&self) -> &[(String, String)] {
        self.keyvalregex.as_slice()
    }

    pub fn query_args(&self) -> Option<QueryArgs> {
        match (&self.search_pattern, self.keyval.first(), self.keyvalregex.first()) {
            (&Some(ref pattern), None, None) => Some(
                QueryArgs::ValuePattern(pattern.to_string())
            ),
            (&None, Some(keyval), None) => Some(
                QueryArgs::KeyValue(keyval.0.to_string(), keyval.1.to_string())
            ),
            (&None, None, Some(keyvalregex)) => Some(
                QueryArgs::KeyValueRegex(keyvalregex.0.to_string(), keyvalregex.1.to_string())
            ),
            (pattern_opt, _, _) => {
                let mut vec = vec![];

                if let Some(ref pattern) = pattern_opt {
                    vec.push(QueryArgs::ValuePattern(pattern.to_string()));
                }

                for (k, v) in &self.keyval {
                    vec.push(QueryArgs::KeyValue(k.to_string(), v.to_string()));
                }

                for (k, v) in &self.keyvalregex {
                    vec.push(QueryArgs::KeyValueRegex(k.to_string(), v.to_string()));
                }

                if vec.is_empty() {
                    None
                } else {
                    Some(QueryArgs::Intersection(vec))
                }
            },
        }
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

    pub fn open_last_session(&self) -> bool {
        self.open_last_session
    }
}

fn create_config_file<P: AsRef<Path> + Debug>(dir_path: P, file_path: P, contents: &[u8]) -> Result<(), String> {
    if !dir_path.as_ref().is_dir() {
        if let Err(err) = ::std::fs::create_dir_all(&dir_path) {
            return Err(format!("failed to create config directory ({:?}): {}",
                dir_path,
                err
            ));
        }
    }

    let mut file = File::create(&file_path)
        .map_err(|err| format!("failed to create config file {:?}: {}", &file_path, err))?;

    file.write_all(contents)
        .map_err(|err| format!(
            "failed to write contents to config file {:?}: {}",
            &file_path,
            err
        ))
}

pub fn read_last_session() -> Result<Session, String> {
    let session_path = {
        let config_dir = proj_dirs_result()?.config_dir();
        let mut path = PathBuf::from(config_dir);
        path.push("last_session.toml");
        path
    };
    Session::from_toml_file(session_path)
}

pub fn save_session(session: &Session) -> Result<(), String>
{
    let config_dir = proj_dirs_result()?.config_dir();
    let session_path = {
        let mut path = PathBuf::from(config_dir);
        path.push("last_session.toml");
        path
    };
    let contents = session.to_toml_string();
    create_config_file(config_dir, &session_path, contents.as_bytes())
}


#[cfg(test)]
mod tests {
    use config::*;

    #[test]
    fn default_config() {
        let mut config = Config::from_toml_str::<&str>(DEFAULT_CONFIG, None).unwrap();
        config.add_tile_sources_from_str::<&str>(DEFAULT_TILE_SOURCES, None).unwrap();
    }
}
