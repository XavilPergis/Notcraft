use engine::world::ChunkPos;
use specs::prelude::*;

#[derive(Copy, Clone, Debug, PartialEq, Component)]
#[storage(DenseVecStorage)]
pub struct ChunkId(pub ChunkPos);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default, Component)]
#[storage(NullStorage)]
pub struct MarkedForDeletion;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default, Component)]
#[storage(NullStorage)]
pub struct MarkedForUpdate;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default, Component)]
#[storage(NullStorage)]
pub struct MarkedForLoading;
