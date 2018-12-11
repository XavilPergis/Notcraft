use specs::prelude::*;

mod physics;
mod transform;
mod world;

pub use self::{
    physics::{Collidable, RigidBody},
    transform::Transform,
    world::*,
};

#[derive(Copy, Clone, Debug, PartialEq, Default, Component)]
#[storage(NullStorage)]
pub struct ClientControlled;

#[derive(Copy, Clone, Debug, PartialEq, Default, Component)]
#[storage(NullStorage)]
pub struct Player;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default, Component)]
#[storage(NullStorage)]
pub struct DirtyMesh;
