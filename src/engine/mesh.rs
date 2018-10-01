use std::ops::Index;
use gl_api::error::GlResult;
use gl;
use gl::types::*;
use gl_api::layout::GlLayout;
use gl_api::vertex_array::VertexArray;
use gl_api::buffer::{VertexBuffer, ElementBuffer, UsageType};
use gl_api::shader::program::LinkedProgram;

pub trait MeshIndex: Copy {
    const INDEX_TYPE: GLenum;
    // HACK: Vec needs a usize to be able to index into it, so we need a mechanism to convert the mesh index into a usize
    // There might be other, better, ways to do this, but idk
    fn as_usize(self) -> usize;
}

impl MeshIndex for u8 { #[inline(always)] fn as_usize(self) -> usize { self as usize } const INDEX_TYPE: GLenum = gl::UNSIGNED_BYTE; }
impl MeshIndex for u16 { #[inline(always)] fn as_usize(self) -> usize { self as usize } const INDEX_TYPE: GLenum = gl::UNSIGNED_SHORT; }
impl MeshIndex for u32 { #[inline(always)] fn as_usize(self) -> usize { self as usize } const INDEX_TYPE: GLenum = gl::UNSIGNED_INT; }

#[derive(Debug)]
pub struct GlMesh<V: GlLayout, I: MeshIndex> {
    vao: VertexArray,
    vertices: VertexBuffer<V>,
    indices: ElementBuffer<I>,
}

impl<V: GlLayout, I: MeshIndex> GlMesh<V, I> {
    pub fn new() -> GlResult<Self> {
        let vbo = VertexBuffer::new();
        let ibo = ElementBuffer::new();
        let mut vao = VertexArray::new();
        vao.add_buffer(&vbo)?;

        Ok(GlMesh { vao, indices: ibo, vertices: vbo })
    }

    pub fn upload<IV: AsRef<[V]>, II: AsRef<[I]>>(&mut self, vertices: IV, indices: II, usage_type: UsageType) -> GlResult<()> {
        self.vertices.upload(vertices.as_ref(), usage_type)?;
        self.indices.upload(indices.as_ref(), usage_type)?;

        let stray_vertices = self.indices.len() % 3;
        println!(
            "Created Mesh: \n\tVertices: {}\n\tTriangles: {}\n\tStray Vertices: {}",
            vertices.as_ref().len(),
            indices.as_ref().len(),
            stray_vertices);

        Ok(())
    }

    pub fn draw_with(&self, pipeline: &LinkedProgram) -> GlResult<()> {
        // Only issue a draw call if there's something to render!
        if self.vertices.len() > 0 {
            self.vao.bind();
            self.indices.bind();
            pipeline.bind();
            unsafe { gl_call!(DrawElements(gl::TRIANGLES, self.indices.len() as i32, I::INDEX_TYPE, 0 as *const _)) }
        } else { Ok(()) }
    }
}

struct TriangleIter<'m, V: 'm, I: MeshIndex + 'm> {
    mesh: &'m Mesh<V, I>,
    num: usize,
}

impl<'m, V: 'm, I: MeshIndex + 'm> Iterator for TriangleIter<'m, V, I> {
    type Item = TriangleRef<'m, V, I>;

    fn next(&mut self) -> Option<Self::Item> {
        let res = self.mesh.triangle(self.num);
        self.num += 1;
        res
    }
}

pub struct TriangleRef<'m, V: 'm, I: MeshIndex> {
    indices: (I, I, I),
    vertices: (&'m V, &'m V, &'m V),
}

// impl<'m, V: 'm, I: MeshIndex> TriangleRef<'m, V, I> {
//     fn from_indices(vertices: &[V], indices: &[I])
// }

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Mesh<V, I: MeshIndex> {
    crate vertices: Vec<V>,
    crate indices: Vec<I>,
}

impl<V, I: MeshIndex> Default for Mesh<V, I> {
    fn default() -> Self { Mesh { vertices: Vec::new(), indices: Vec::new() } }
}


impl<V, I: MeshIndex> Mesh<V, I> {
    pub fn with_capacity(verts_cap: usize, indices_cap: usize) -> Self {
        Mesh {
            vertices: Vec::with_capacity(verts_cap),
            indices: Vec::with_capacity(indices_cap),
        }
    }

    pub fn triangle(&self, triangle_index: usize) -> Option<TriangleRef<'_, V, I>> {
        if triangle_in_bounds(self.indices.len(), triangle_index) {
            let base = triangle_index * 3;
            Some(TriangleRef {
                vertices: (self.vertices.get(self.indices[base].as_usize())?, self.vertices.get(self.indices[base + 1].as_usize())?, self.vertices.get(self.indices[base + 2].as_usize())?),
                indices: (self.indices[base], self.indices[base + 1], self.indices[base + 2]),
            })
        } else { None }
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() * 3
    }

    pub fn triangles(&self) -> impl Iterator<Item=TriangleRef<'_, V, I>> {
        TriangleIter { mesh: self, num: 0 }
    }
}

impl<V: GlLayout, I: MeshIndex> Mesh<V, I> {
    /// Creates the GPU buffers for this mesh and uploads the data in this mesh to it. Note that this has to clone the data
    pub fn to_gl_mesh(&self, usage_type: UsageType) -> GlResult<GlMesh<V, I>> {
        let mut mesh = GlMesh::new()?;
        mesh.upload(&self.vertices[..], &self.indices[..], usage_type)?;
        Ok(mesh)
    }
}

fn triangle_in_bounds(triangle_list_len: usize, triangle_index: usize) -> bool {
    triangle_index < triangle_list_len / 3
}
