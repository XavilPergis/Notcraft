use engine::VoxelProperties;
use cgmath::{Vector2, Vector3};
use engine::{Precomputed, Side, Voxel};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Block {
    Air,
    Stone,
    Dirt,
    Grass,
    Water,
}

impl Voxel for Block {
    fn properties(&self) -> VoxelProperties {
        VoxelProperties {
            opaque: *self != Block::Air
        }
    }
}
