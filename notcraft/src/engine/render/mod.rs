pub mod camera;
pub mod mesh;
pub mod mesher;
pub mod renderer;

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Pos {
    pub pos: [f32; 3],
}
glium::implement_vertex!(Pos, pos);

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Tex {
    pub uv: [f32; 2],
}
glium::implement_vertex!(Tex, uv);

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Norm {
    pub normal: [f32; 3],
}
glium::implement_vertex!(Norm, normal);

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Tang {
    pub tangent: [f32; 3],
}
glium::implement_vertex!(Tang, tangent);

#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Ao {
    pub ao: f32,
}
glium::implement_vertex!(Ao, ao);

#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct TexId {
    // TODO: u16?
    pub id: u32,
}
glium::implement_vertex!(TexId, id);
