pub mod camera;
pub mod chunk_manager;
pub mod chunk;
pub mod mesh;
pub mod terrain;

/// Six sides of a cube.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Side {
    /// Positive Y.
    Top,
    /// Negative Y.
    Bottom,
    /// Positive X.
    Right,
    /// Negative X.
    Left,
    /// Positive Z.
    Front,
    /// Negative Z.
    Back,
}
