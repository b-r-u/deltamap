
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

    pub fn from_str(s: &str) -> Option<Projection> {
        match s {
            "mercator" => Some(Projection::Mercator),
            "orthografic" => Some(Projection::Orthografic),
            _ => None,
        }
    }
}
