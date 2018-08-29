use cgmath::Vector3;
use collision::Aabb3;
use specs::prelude::*;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RigidBody {
    pub mass: f64,
    pub drag: Vector3<f64>,
    pub velocity: Vector3<f64>,
    pub aabb: Aabb3<f64>,
}

impl Component for RigidBody {
    type Storage = DenseVecStorage<Self>;
}
