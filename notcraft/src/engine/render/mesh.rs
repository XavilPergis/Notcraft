use glium::{
    backend::Facade,
    index::{Index, PrimitiveType},
    vertex::BufferCreationError,
    *,
};
use num_traits::{AsPrimitive, CheckedAdd, FromPrimitive};

#[derive(Debug)]
pub struct GpuMesh<V: Copy, I: Index> {
    pub vertices: VertexBuffer<V>,
    pub indices: IndexBuffer<I>,
}

#[derive(Debug)]
pub enum BufferError {
    Vertex(vertex::BufferCreationError),
    Index(index::BufferCreationError),
}

impl<V: Copy + Vertex, I: Index> GpuMesh<V, I> {
    pub fn empty_dynamic<F: Facade>(
        ctx: &F,
        primitive_type: PrimitiveType,
        reserve_vert: usize,
        reserve_idx: usize,
    ) -> Result<Self, BufferError> {
        Ok(GpuMesh {
            vertices: VertexBuffer::empty_dynamic(ctx, reserve_vert)
                .map_err(BufferError::Vertex)?,
            indices: IndexBuffer::empty_dynamic(ctx, primitive_type, reserve_idx)
                .map_err(BufferError::Index)?,
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct Mesh<V, I> {
    vertices: Vec<V>,
    indices: Vec<I>,
}

impl<V, I> Mesh<V, I> {
    #[must_use]
    pub fn add<VI, II>(&mut self, verts: VI, indices: II) -> bool
    where
        VI: IntoIterator<Item = V>,
        II: IntoIterator<Item = I>,
        I: FromPrimitive + CheckedAdd + Copy + 'static,
    {
        match I::from_usize(self.vertices.len()) {
            Some(base) => {
                for index in indices.into_iter() {
                    match base.checked_add(&index) {
                        None => return false,
                        Some(sum) => self.indices.push(sum),
                    }
                }
                self.vertices.extend(verts);
                true
            }
            None => false,
        }
    }

    pub fn to_gpu_mesh<F: Facade>(
        &self,
        ctx: &F,
        primitive: PrimitiveType,
    ) -> Result<GpuMesh<V, I>, BufferError>
    where
        V: Copy + Vertex,
        I: Index,
    {
        let vertices = VertexBuffer::new(ctx, &self.vertices).map_err(BufferError::Vertex)?;
        let indices =
            IndexBuffer::new(ctx, primitive, &self.indices).map_err(BufferError::Index)?;

        Ok(GpuMesh { vertices, indices })
    }
}
