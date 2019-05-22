use crate::{
    engine::{prelude::*, render::verts},
};
use glutin::GlWindow;

pub struct UiNode {
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub position: Point3<f32>,
    pub orientation: Vector2<Deg<f32>>,
    pub scale: Vector3<f32>,
}

pub struct UiParent(pub Entitiy);

pub enum TransformBase {
    Center,
    BottomLeft,
    BottomRight,
    TopLeft,
    TopRight,
    CenterTop,
    CenterBottom,
    CenterLeft,
    CenterRight,
}

pub struct UiPass {}

impl DeferredRenderPass for UiPass {
    
}
