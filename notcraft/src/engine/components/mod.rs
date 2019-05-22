use specs::prelude::*;

mod transform;
pub use transform::*;

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Player;

impl Component for Player {
    type Storage = NullStorage<Self>;
}
