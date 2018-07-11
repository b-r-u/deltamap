use coord::MapCoord;
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
    pub view_center: MapCoord,
    pub zoom: f64,
    pub tile_source: Option<String>,
    pub projection: Projection,
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
            Ok(Value::Table(ref table)) => {

                let view = match table.get("view") {
                    Some(&Value::Table(ref table)) => table,
                    Some(_) => return Err("view has to be a table.".to_string()),
                    None => return Err("view table is missing.".to_string()),
                };

                let x = match view.get("x") {
                    Some(&Value::Float(x)) => x,
                    Some(&Value::Integer(x)) => x as f64,
                    Some(_) => return Err("x has to be a number.".to_string()),
                    None => return Err("x position is missing.".to_string()),
                };

                let y = match view.get("y") {
                    Some(&Value::Float(y)) => y,
                    Some(&Value::Integer(y)) => y as f64,
                    Some(_) => return Err("y has to be a number.".to_string()),
                    None => return Err("y position is missing.".to_string()),
                };

                let zoom = match view.get("zoom") {
                    Some(&Value::Float(z)) => z,
                    Some(&Value::Integer(z)) => z as f64,
                    Some(_) => return Err("zoom has to be a number.".to_string()),
                    None => return Err("zoom value is missing.".to_string()),
                };

                let tile_source = match view.get("tile_source") {
                    Some(&Value::String(ref s)) => Some(s.clone()),
                    Some(_) => return Err("tile_source has to be a string.".to_string()),
                    None => None,
                };

                let projection = match view.get("projection") {
                    Some(&Value::String(ref s)) => {
                        Projection::from_str(s).unwrap_or_else(|_| Projection::Mercator)
                    },
                    Some(_) => return Err("projection has to be a string.".to_string()),
                    None => Projection::Mercator,
                };

                Ok(
                    Session {
                        view_center: MapCoord::new(x, y),
                        zoom,
                        tile_source,
                        projection,
                    }
                )
            },
            Ok(_) => Err("TOML file has invalid structure. Expected a Table as the top-level element.".to_string()),
            Err(e) => Err(format!("{}", e)),
        }
    }

    pub fn to_toml_string(&self) -> String {
        let mut view = Table::new();
        view.insert("x".to_string(), Value::Float(self.view_center.x));
        view.insert("y".to_string(), Value::Float(self.view_center.y));
        view.insert("zoom".to_string(), Value::Float(self.zoom));
        if let Some(ref tile_source) = self.tile_source {
            view.insert("tile_source".to_string(), Value::String(tile_source.clone()));
        }
        view.insert("projection".to_string(), Value::String(self.projection.to_str().to_string()));

        let mut root = Table::new();
        root.insert("view".to_string(), Value::Table(view));

        toml::ser::to_string_pretty(&Value::Table(root)).unwrap()
    }
}
