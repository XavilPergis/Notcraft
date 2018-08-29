use specs::prelude::*;

mod transform;
mod input;
mod physics;
mod world;

pub use self::transform::Transform;
pub use self::input::{LookTarget, MoveDelta, ActiveDirections};
pub use self::physics::RigidBody;
pub use self::world::ChunkId;

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct ClientControlled;
impl Component for ClientControlled { type Storage = NullStorage<Self>; }

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Player;
impl Component for Player { type Storage = NullStorage<Self>; }

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct DirtyMesh;
impl Component for DirtyMesh { type Storage = NullStorage<Self>; }