mod program;
mod shader;
mod uniform;

pub struct Compiled<T>(pub T);

pub use self::{program::*, shader::*, uniform::*};
