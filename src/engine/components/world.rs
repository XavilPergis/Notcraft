use engine::world::ChunkPos;
use specs::prelude::*;

#[derive(Copy, Clone, Debug, PartialEq, Component)]
#[storage(DenseVecStorage)]
pub struct ChunkId(pub ChunkPos);
