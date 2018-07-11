use std::str::FromStr;


#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Projection {
    // EPSG:3857: WGS 84 / Pseudo-Mercator
    Mercator,
    // Orthographic projection, WGS 84 coordinates mapped to the sphere
    Orthografic,
}

impl Projection {
    pub fn to_str(&self) -> &str {
        match *self {
            Projection::Mercator => "mercator",
            Projection::Orthografic => "orthografic",
        }
    }
}

impl FromStr for Projection {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, ()> {
        match s {
            "mercator" => Ok(Projection::Mercator),
            "orthografic" => Ok(Projection::Orthografic),
            _ => Err(()),
        }
    }
}
