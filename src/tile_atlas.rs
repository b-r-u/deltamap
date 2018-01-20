use coord::{ScreenRect, SubTileCoord, TileCoord, TextureRect};
use image;
use linked_hash_map::LinkedHashMap;
use map_view::VisibleTile;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use texture::Texture;
use tile::Tile;
use tile_cache::TileCache;
use tile_source::TileSource;

#[derive(Clone, Debug)]
pub struct TexturedVisibleTile {
    pub screen_rect: ScreenRect,
    pub tex_rect: TextureRect,
    pub tex_minmax: TextureRect,
}

#[derive(Clone, Debug)]
pub struct TileAtlas<'a> {
    texture: Texture<'a>,
    tile_size: u32,
    slots_lru: LinkedHashMap<CacheSlot, Option<Tile>>, // LRU cache of slots
    tile_to_slot: HashMap<Tile, CacheSlot>,
}

impl<'a> TileAtlas<'a> {
    fn init(&mut self) {
        // add tile for default slot
        {
            let img = image::open("img/no_tile.png").unwrap();
            self.texture.sub_image(0, 0, &img);
        }

        let slots_x = self.texture.width() / self.tile_size;
        let slots_y = self.texture.height() / self.tile_size;
        let num_slots = (slots_x * slots_y) as usize;

        self.slots_lru.clear();
        self.slots_lru.reserve(num_slots);
        for x in 0..slots_x {
            for y in 0..slots_y {
                let slot = CacheSlot { x: x, y: y };
                self.slots_lru.insert(slot, None);
            }
        }
        self.slots_lru.remove(&Self::default_slot());

        self.tile_to_slot.clear();
        self.tile_to_slot.reserve(num_slots);
    }

    pub fn new(tex: Texture<'a>, tile_size: u32) -> Self {
        let mut atlas = TileAtlas {
            texture: tex,
            tile_size: tile_size,
            slots_lru: LinkedHashMap::new(),
            tile_to_slot: HashMap::new(),
        };

        atlas.init();
        atlas
    }

    pub fn double_texture_size(&mut self) -> Result<(), ()> {
        let max_size = self.texture.context().max_texture_size() as u32;

        let new_width = self.texture.width() * 2;
        let new_height = self.texture.height() * 2;

        if new_width <= max_size && new_height <= max_size {
            self.texture.resize(new_width, new_height);

            // remove old entries, initialize texture
            self.init();

            info!("new atlas size {}x{}", new_width, new_height);

            Ok(())
        } else {
            Err(())
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

    /// Finds textures from the cache for a given slice of visible tiles. The texture atlas may not
    /// be big enough to hold all textures at once; a possible remainder of untextured visible tiles is
    /// returned as an `Option`.
    /// The function guarantees that no more than `max_tiles_to_use` tiles are used for texturing;
    /// the number of used tiles is returned as an `usize`.
    pub fn textured_visible_tiles<'b>(
        &mut self,
        visible_tiles: &'b [VisibleTile],
        max_tiles_to_use: usize,
        source: &TileSource,
        cache: &mut TileCache,
        ) -> (Vec<TexturedVisibleTile>, Option<&'b [VisibleTile]>, usize)
    {
        let mut tvt = Vec::with_capacity(visible_tiles.len());

        let inset_x = 0.5 / f64::from(self.texture.width());
        let inset_y = 0.5 / f64::from(self.texture.height());

        let num_usable_slots = self.slots_lru.len();
        // The number of actually used slots may be lower, because slots can be used multiple times
        // in the same view (especially the default slot).
        let mut used_slots = 0_usize;

        for (i, vt) in visible_tiles.iter().enumerate() {
            if used_slots >= num_usable_slots || used_slots >= max_tiles_to_use {
                return (tvt, Some(&visible_tiles[i..]), used_slots);
            }

            if let Some(slot) = self.store(vt.tile, source, cache, true) {
                let tex_rect = self.slot_to_texture_rect(slot);
                used_slots += 1;
                tvt.push(
                    TexturedVisibleTile {
                        screen_rect: vt.rect,
                        tex_rect: tex_rect,
                        tex_minmax: tex_rect.inset(inset_x, inset_y),
                    }
                );
            } else {
                // exact tile not found

                if used_slots + 5 > num_usable_slots || used_slots + 5 > max_tiles_to_use {
                    return (tvt, Some(&visible_tiles[i..]), used_slots);
                }

                // default tile
                let mut tex_sub_rect = self.slot_to_texture_rect(Self::default_slot());
                let mut tex_rect = tex_sub_rect;

                // look for cached tiles in lower zoom layers
                for dist in 1..31 {
                    if let Some((parent_tile, sub_coord)) = vt.tile.parent(dist) {
                        if let Some(slot) = self.store(parent_tile, source, cache, false) {
                            used_slots += 1;
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
                        used_slots += 1;
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

        (tvt, None, used_slots)
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
