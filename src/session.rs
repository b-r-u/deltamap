use projection::Projection;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::str::FromStr;
use toml::Value;
use toml::value::Table;
use toml;


#[derive(Debug)]
pub struct Session {
    pub view: Table,
}

impl Session {
    pub fn from_toml_file<P: AsRef<Path>>(path: P) -> Result<Session, String> {
        let mut file = File::open(&path).map_err(|e| format!("{}", e))?;

        let mut content = String::new();
        file.read_to_string(&mut content).map_err(|e| format!("{}", e))?;

        Session::from_toml_str(&content)
    }

    pub fn from_toml_str(toml_str: &str) -> Result<Session, String> {
        match toml_str.parse::<Value>() {
            Ok(Value::Table(mut table)) => {
                let view = match table.remove("view") {
                    Some(Value::Table(table)) => table,
                    Some(_) => return Err("view has to be a table.".to_string()),
                    None => return Err("view table is missing.".to_string()),
                };

                Ok(
                    Session {
                        view,
                    }
                )
            },
            Ok(_) => Err("TOML file has invalid structure. Expected a Table as the top-level element.".to_string()),
            Err(e) => Err(format!("{}", e)),
        }
    }

    pub fn to_toml_string(&self) -> String {
        let mut root = Table::new();
        root.insert("view".to_string(), Value::Table(self.view.clone()));

        toml::ser::to_string_pretty(&Value::Table(root)).unwrap()
    }

    pub fn set_tile_source(&mut self, tile_source: Option<&str>) {
        match tile_source {
            None => {
                self.view.remove("tile_source");
            },
            Some(tile_source) => {
                self.view.insert(
                    "tile_source".to_string(),
                    Value::String(tile_source.to_string())
                );
            },
        }
    }

    pub fn tile_source(&self) -> Option<&str> {
        if let Some(Value::String(s)) = self.view.get("tile_source") {
            Some(s.as_str())
        } else {
            None
        }
    }

    pub fn projection(&self) -> Option<Projection> {
        match self.view.get("projection") {
            Some(Value::String(s)) => Projection::from_str(s.as_str()).ok(),
            _ => None,
        }
    }
}
