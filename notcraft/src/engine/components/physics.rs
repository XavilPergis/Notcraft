use cgmath::Vector3;
use collision::Aabb3;
use specs::prelude::*;

#[derive(Copy, Clone, Debug, PartialEq, Component)]
#[storage(DenseVecStorage)]
pub struct RigidBody {
    pub mass: f32,
    pub drag: Vector3<f32>,
    pub velocity: Vector3<f32>,
}

#[derive(Copy, Clone, Debug, PartialEq, Component)]
#[storage(DenseVecStorage)]
pub struct Collidable {
    pub aabb: Aabb3<f32>,
}
