use super::{chunk::Chunk, LoadEvents};
use crate::prelude::*;
use std::sync::Arc;

pub struct RegionPos {
    pub x: i32,
    pub z: i32,
}

pub struct WorldPersistence {
    // loaded_in_region: HashMap<RegionPos, HashSet<>>,
}

impl WorldPersistence {
    pub fn new() -> Self {
        Self {}
    }

    pub fn save_chunk(&mut self, chunk: &Arc<Chunk>) -> Result<()> {
        todo!()
    }

    pub fn load_chunk(&mut self) -> Result<Chunk> {
        todo!()
    }
}

pub fn update_persistence(persistence: ResMut<WorldPersistence>, load_events: LoadEvents) {}
