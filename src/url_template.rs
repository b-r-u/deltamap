use coord::TileCoord;
use regex::Regex;


/// Kinds of placeholders for a `UrlTemplate`
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
enum Placeholder {
    /// Tile x coordinate
    X,
    /// Tile y coordinate
    Y,
    /// Tile zoom
    Z,
    /// Quadkey encoded coord
    Quadkey
}

impl Placeholder {
    /// Returns maximum number of bytes that the value for a placeholder with occupy.
    fn max_size(&self) -> usize {
        match *self {
            Placeholder::X | Placeholder::Y | Placeholder::Z => 11,
            Placeholder::Quadkey => 30,
        }
    }
}

#[derive(Debug)]
pub struct UrlTemplate {
    /// The template string that includes placeholders between static parts
    template_string: String,
    /// Ranges into `template_string` for static parts
    static_parts: Vec<::std::ops::Range<usize>>,
    /// Kinds of placeholders between the static parts
    placeholders: Vec<Placeholder>,
    /// Maximum length in bytes of a filled template
    max_size: usize,
}

impl UrlTemplate {
    pub fn new<S: Into<String>>(template_str: S) -> Result<UrlTemplate, String> {
        let template_string = template_str.into();
        let mut static_parts = vec![];
        let mut placeholders = vec![];
        let mut max_size = 0;

        lazy_static! {
            static ref RE: Regex = Regex::new(r"\{([a-z]+)\}").unwrap();
        }

        let mut offset = 0;
        for cap in RE.captures_iter(&template_string) {
            let cap0 = cap.get(0).unwrap();
            static_parts.push(offset..cap0.start());
            max_size += cap0.start() - offset;

            {
                let ph = match cap.get(1).unwrap().as_str() {
                    "x" => Placeholder::X,
                    "y" => Placeholder::Y,
                    "z" => Placeholder::Z,
                    "quadkey" => Placeholder::Quadkey,
                    s => return Err(format!("Invalid placeholder in url template: {:?}", s)),
                };
                max_size += ph.max_size();
                placeholders.push(ph);
            }

            offset = cap0.end();
        }

        static_parts.push(offset..template_string.len());
        max_size += template_string.len() - offset;

        let template_valid =
            placeholders.contains(&Placeholder::Quadkey) ||
            (placeholders.contains(&Placeholder::X) &&
             placeholders.contains(&Placeholder::Y) &&
             placeholders.contains(&Placeholder::Z));

        if !template_valid {
            return Err(format!(
                "template is not valid because one or multiple placeholders are missing: {:?}",
                template_string)
            );
        }

        Ok(UrlTemplate {
            template_string,
            static_parts,
            placeholders,
            max_size,
        })
    }

    pub fn fill(&self, tile_coord: TileCoord) -> Option<String> {
        let mut ret = String::with_capacity(self.max_size);

        if let Some(prefix) = self.static_parts.first() {
            ret += &self.template_string[prefix.start..prefix.end];
        }

        for (i, static_part) in self.static_parts.iter().skip(1).enumerate() {
            let dyn_part = match self.placeholders[i] {
                Placeholder::X => tile_coord.x.to_string(),
                Placeholder::Y => tile_coord.y.to_string(),
                Placeholder::Z => tile_coord.zoom.to_string(),
                Placeholder::Quadkey => {
                    match tile_coord.to_quadkey() {
                        Some(q) => q,
                        None => return None,
                    }
                }
            };
            ret += &dyn_part;
            ret += &self.template_string[static_part.start..static_part.end];;
        }
        Some(ret)
    }
}

#[cfg(test)]
mod tests {
    use url_template::*;

    fn check_templ(templ_str: &str, coord: TileCoord, result: &str) {
        let t = UrlTemplate::new(templ_str).unwrap();
        assert_eq!(t.fill(coord), Some(result.to_string()));
    }

    #[test]
    fn check_new() {
        assert!(UrlTemplate::new("").is_err());
        assert!(UrlTemplate::new("abc").is_err());
        assert!(UrlTemplate::new("{x}").is_err());
        assert!(UrlTemplate::new("{z}{y}").is_err());
        assert!(UrlTemplate::new("{x}{z}{y}").is_ok());
        assert!(UrlTemplate::new("{quadkey}").is_ok());
        assert!(UrlTemplate::new("{x}{quadkey}").is_ok());
    }

    #[test]
    fn check_fill() {
        check_templ("https://tiles.example.com/{z}/{x}/{y}.png",
                    TileCoord::new(2, 1, 0),
                    "https://tiles.example.com/2/1/0.png");
        check_templ("{z}{x}{y}",
                    TileCoord::new(2, 1, 0),
                    "210");
        check_templ("{quadkey}",
                    TileCoord::new(3, 1, 0),
                    "001");
        check_templ("{x}{x}{y}{z}",
                    TileCoord::new(2, 1, 0),
                    "1102");
        check_templ("a{quadkey}b{z}c",
                    TileCoord::new(1, 0, 0),
                    "a0b1c");
    }
}
