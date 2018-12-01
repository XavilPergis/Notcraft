#[macro_use]
pub mod error;
#[macro_use]
pub mod layout;

pub mod buffer;
pub mod context;
mod draw;
pub mod limits;
pub mod misc;
pub mod program;
pub mod texture;
pub mod texture_array;
pub mod vertex_array;

pub use self::{
    buffer::{Buffer, BufferBuilder, UsageType},
    context::Context,
    draw::{BufferIndex, PrimitiveType},
    program::{create_program, ProgramStages},
};

pub struct Cons<T, L>(pub T, pub L);
pub struct Nil;

#[macro_export]
macro_rules! cons {
    ($head:expr, $($tail:tt)*) => {
        $crate::Cons($head, $crate::cons!($($tail)*))
    };

    ($head:expr) => {
        $crate::Cons($head, $crate::Nil)
    };
}

#[macro_export]
macro_rules! cons_ty {
    ($head:ty, $($tail:tt)*) => {
        $crate::Cons<$head, $crate::cons_ty!($($tail)*)>
    };

    ($head:ty) => {
        $crate::Cons<$head, $crate::Nil>
    };
}
