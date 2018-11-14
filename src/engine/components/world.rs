use cgmath::Point3;
use specs::prelude::*;

#[derive(Copy, Clone, Debug, PartialEq, Component)]
#[storage(DenseVecStorage)]
pub struct ChunkId(pub Point3<i32>);
