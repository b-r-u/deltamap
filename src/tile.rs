use coord::TileCoord;
use tile_source::TileSourceId;


#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Tile {
    pub coord: TileCoord,
    pub source_id: TileSourceId,
}

impl Tile {
    pub fn new(coord: TileCoord, source_id: TileSourceId) -> Tile {
        Tile {
            coord: coord,
            source_id: source_id,
        }
    }
}
