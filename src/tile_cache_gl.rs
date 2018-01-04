use coord::{ScreenRect, SubTileCoord, TileCoord};
use linked_hash_map::LinkedHashMap;
use map_view::VisibleTile;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use texture::Texture;
use tile::Tile;
use tile_cache::TileCache;
use tile_source::TileSource;

#[derive(Copy, Clone, Debug)]
pub struct TextureRect {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

impl TextureRect {
    pub fn inset(self, margin_x: f64, margin_y: f64) -> TextureRect {
        TextureRect {
            x1: self.x1 + margin_x,
            y1: self.y1 + margin_y,
            x2: self.x2 - margin_x,
            y2: self.y2 - margin_y,
        }
    }

    pub fn subdivide(&self, sub_tile: &SubTileCoord) -> TextureRect {
        let scale = 1.0 / f64::from(sub_tile.size);
        let w = (self.x2 - self.x1) * scale;
        let h = (self.y2 - self.y1) * scale;
        TextureRect {
            x1: self.x1 + f64::from(sub_tile.x) * w,
            y1: self.y1 + f64::from(sub_tile.y) * h,
            x2: self.x1 + f64::from(sub_tile.x + 1) * w,
            y2: self.y1 + f64::from(sub_tile.y + 1) * h,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TexturedVisibleTile {
    pub screen_rect: ScreenRect,
    pub tex_rect: TextureRect,
    pub tex_minmax: TextureRect,
}

#[derive(Clone, Debug)]
pub struct TileCacheGl<'a> {
    texture: Texture<'a>,
    tile_size: u32,
    slots_lru: LinkedHashMap<CacheSlot, Option<Tile>>, // LRU cache of slots
    tile_to_slot: HashMap<Tile, CacheSlot>,
}

impl<'a> TileCacheGl<'a> {
    pub fn new(tex: Texture<'a>, tile_size: u32) -> Self {
        let slots_x = tex.width() / tile_size;
        let slots_y = tex.height() / tile_size;
        let num_slots = (slots_x * slots_y) as usize;

        let mut slots_lru = LinkedHashMap::with_capacity(num_slots);
        for x in 0..slots_x {
            for y in 0..slots_y {
                let slot = CacheSlot { x: x, y: y };
                slots_lru.insert(slot, None);
            }
        }

        slots_lru.remove(&Self::default_slot());

        TileCacheGl {
            texture: tex,
            tile_size: tile_size,
            slots_lru: slots_lru,
            tile_to_slot: HashMap::with_capacity(num_slots),
        }
    }

    pub fn default_slot() -> CacheSlot {
        CacheSlot { x: 0, y: 0 }
    }

    pub fn store(&mut self, tile_coord: TileCoord, source: &TileSource, cache: &mut TileCache, load: bool) -> Option<CacheSlot> {
        let mut remove_tile = None;
        let tile = Tile::new(tile_coord, source.id());

        let slot = match self.tile_to_slot.entry(tile) {
            Entry::Vacant(entry) => {
                let img_option = if load {
                    cache.get_async(tile_coord, source, true)
                } else {
                    cache.lookup(tile)
                };

                if let Some(img) = img_option {
                    let (slot, old_tile) = self.slots_lru.pop_front().unwrap();
                    self.slots_lru.insert(slot, Some(tile));

                    remove_tile = old_tile;

                    self.texture.sub_image(
                        (slot.x * self.tile_size) as i32,
                        (slot.y * self.tile_size) as i32,
                        img,
                    );
                    Some(*entry.insert(slot))
                } else {
                    None
                }
            },
            Entry::Occupied(entry) => {
                let slot = *entry.into_mut();

                self.slots_lru.get_refresh(&slot);

                Some(slot)
            },
        };

        if let Some(t) = remove_tile {
            self.tile_to_slot.remove(&t);
        }

        slot
    }

    pub fn textured_visible_tiles(
        &mut self,
        visible_tiles: &[VisibleTile],
        source: &TileSource,
        cache: &mut TileCache,
        ) -> Vec<TexturedVisibleTile>
    {
        let mut tvt = Vec::with_capacity(visible_tiles.len());

        let inset_x = 0.5 / f64::from(self.texture.width());
        let inset_y = 0.5 / f64::from(self.texture.height());

        for vt in visible_tiles {
            if let Some(slot) = self.store(vt.tile, source, cache, true) {
                let tex_rect = self.slot_to_texture_rect(slot);
                tvt.push(
                    TexturedVisibleTile {
                        screen_rect: vt.rect,
                        tex_rect: tex_rect,
                        tex_minmax: tex_rect.inset(inset_x, inset_y),
                    }
                );
            } else {
                // exact tile not found

                // default tile
                let mut tex_sub_rect = self.slot_to_texture_rect(Self::default_slot());
                let mut tex_rect = tex_sub_rect;

                // look for cached tiles in lower zoom layers
                for dist in 1..31 {
                    if let Some((parent_tile, sub_coord)) = vt.tile.parent(dist) {
                        if let Some(slot) = self.store(parent_tile, source, cache, false) {
                            tex_sub_rect = self.subslot_to_texture_rect(slot, sub_coord);
                            tex_rect = self.slot_to_texture_rect(slot);
                            break;
                        }
                    } else {
                        break;
                    }
                }

                // look for cached tiles in higher zoom layers
                for &(child_tile, child_sub_coord) in &vt.tile.children() {
                    if let Some(slot) = self.store(child_tile, source, cache, false) {
                        let tex_rect = self.slot_to_texture_rect(slot);

                        tvt.push(
                            TexturedVisibleTile {
                                screen_rect: vt.rect.subdivide(&child_sub_coord),
                                tex_rect: tex_rect,
                                tex_minmax: tex_rect.inset(inset_x, inset_y),
                            }
                        );
                    } else {
                        tvt.push(
                            TexturedVisibleTile {
                                screen_rect: vt.rect.subdivide(&child_sub_coord),
                                tex_rect: tex_sub_rect.subdivide(&child_sub_coord),
                                tex_minmax: tex_rect.inset(inset_x, inset_y),
                            }
                        );
                    }
                }
            };
        }

        tvt
    }

    fn slot_to_texture_rect(&self, slot: CacheSlot) -> TextureRect {
        let scale_x = f64::from(self.tile_size) / f64::from(self.texture.width());
        let scale_y = f64::from(self.tile_size) / f64::from(self.texture.height());

        TextureRect {
            x1: f64::from(slot.x) * scale_x,
            y1: f64::from(slot.y) * scale_y,
            x2: f64::from(slot.x + 1) * scale_x,
            y2: f64::from(slot.y + 1) * scale_y,
        }
    }

    fn subslot_to_texture_rect(&self, slot: CacheSlot, sub_coord: SubTileCoord) -> TextureRect {
        let scale_x = f64::from(self.tile_size) / (f64::from(self.texture.width()) * f64::from(sub_coord.size));
        let scale_y = f64::from(self.tile_size) / (f64::from(self.texture.height()) * f64::from(sub_coord.size));

        TextureRect {
            x1: f64::from(slot.x * sub_coord.size + sub_coord.x) * scale_x,
            y1: f64::from(slot.y * sub_coord.size + sub_coord.y) * scale_y,
            x2: f64::from(slot.x * sub_coord.size + sub_coord.x + 1) * scale_x,
            y2: f64::from(slot.y * sub_coord.size + sub_coord.y + 1) * scale_y,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct CacheSlot {
    pub x: u32,
    pub y: u32,
}
