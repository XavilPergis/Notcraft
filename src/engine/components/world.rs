use specs::prelude::*;
use cgmath::Point3;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ChunkId(pub Point3<i32>);

impl Component for ChunkId {
    type Storage = DenseVecStorage<Self>;
}