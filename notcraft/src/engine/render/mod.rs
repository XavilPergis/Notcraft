pub mod camera;
pub mod mesh;
pub mod mesher;
pub mod renderer;

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Tex {
    pub uv: [f32; 2],
}
glium::implement_vertex!(Tex, uv);

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct PosTex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
}
glium::implement_vertex!(PosTex, pos, uv);
