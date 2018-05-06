use gl_api::error::GlResult;
use gl;
use gl::types::*;
use gl_api::layout::InternalLayout;
use gl_api::vertex_array::VertexArray;
use gl_api::buffer::{VertexBuffer, ElementBuffer, UsageType};
use gl_api::shader::program::LinkedProgram;

pub trait IndexingType {
    const INDEX_TYPE: GLenum;
}

impl IndexingType for u8 { const INDEX_TYPE: GLenum = gl::UNSIGNED_BYTE; }
impl IndexingType for u16 { const INDEX_TYPE: GLenum = gl::UNSIGNED_SHORT; }
impl IndexingType for u32 { const INDEX_TYPE: GLenum = gl::UNSIGNED_INT; }

pub struct Mesh<V: InternalLayout, I: IndexingType> {
    vao: VertexArray,
    vertices: VertexBuffer<V>,
    indices: ElementBuffer<I>,
}

impl<V: InternalLayout, I: IndexingType> Mesh<V, I> {
    pub fn new() -> GlResult<Self> {
        let vbo = VertexBuffer::new();
        let ibo = ElementBuffer::new();
        let mut vao = VertexArray::new();
        vao.add_buffer(&vbo)?;

        Ok(Mesh { vao, indices: ibo, vertices: vbo })
    }

    pub fn upload<IV: AsRef<[V]>, II: AsRef<[I]>>(&mut self, vertices: IV, indices: II, usage_type: UsageType) -> GlResult<()> {
        self.vertices.upload(vertices.as_ref(), usage_type)?;
        self.indices.upload(indices.as_ref(), usage_type)?;

        println!("Created mesh with {} vertices and {} indices ({}) cube faces",
            vertices.as_ref().len(),
            indices.as_ref().len(),
            indices.as_ref().len()/6);

        Ok(())
    }

    pub fn draw_with(&self, pipeline: &LinkedProgram) -> GlResult<()> {
        // Ondy issude a draw call if there's something to render!
        if self.vertices.len() > 0 {
            self.vao.bind();
            self.indices.bind();
            pipeline.bind();
            unsafe { gl_call!(DrawElements(gl::TRIANGLES, self.indices.len() as i32, I::INDEX_TYPE, 0 as *const _)) }
        } else { Ok(()) }
    }
}
