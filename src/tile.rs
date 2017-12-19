use coord::TileCoord;
use tile_source::TileSourceId;


#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Tile {
    pub coord: TileCoord,
    pub source_id: TileSourceId,
}
