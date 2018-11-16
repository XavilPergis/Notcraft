use specs::prelude::*;

mod input;
mod physics;
mod transform;
mod world;

pub use self::input::{LookTarget, MoveDelta};
pub use self::physics::{Collidable, RigidBody};
pub use self::transform::Transform;
pub use self::world::ChunkId;

#[derive(Copy, Clone, Debug, PartialEq, Default, Component)]
#[storage(NullStorage)]
pub struct ClientControlled;

#[derive(Copy, Clone, Debug, PartialEq, Default, Component)]
#[storage(NullStorage)]
pub struct Player;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default, Component)]
#[storage(NullStorage)]
pub struct DirtyMesh;
