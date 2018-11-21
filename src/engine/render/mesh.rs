use gl_api::buffer::{Buffer, UsageType};
use gl_api::context::Context;
use gl_api::error::GlResult;
use gl_api::layout::Layout;
use gl_api::shader::program::LinkedProgram;
use gl_api::BufferIndex;
use gl_api::PrimitiveType;
use specs::prelude::*;

#[derive(Debug, Eq, PartialEq)]
pub struct GpuMesh<V, I> {
    verts: Buffer<V>,
    indices: Buffer<I>,
}

impl<V: Layout, I: BufferIndex> GpuMesh<V, I> {
    pub fn new(ctx: &Context) -> GlResult<Self> {
        let vbo = Buffer::new(ctx);
        let ibo = Buffer::new(ctx);

        Ok(GpuMesh {
            indices: ibo,
            verts: vbo,
        })
    }

    pub fn upload<IV: AsRef<[V]>, II: AsRef<[I]>>(
        &mut self,
        ctx: &Context,
        verts: IV,
        indices: II,
        usage_type: UsageType,
    ) -> GlResult<()> {
        self.verts.upload(ctx, verts.as_ref(), usage_type)?;
        self.indices.upload(ctx, indices.as_ref(), usage_type)?;

        Ok(())
    }

    pub fn draw_with(&self, ctx: &Context, program: &LinkedProgram) {
        ctx.draw_elements(
            PrimitiveType::Triangles,
            program,
            &self.verts,
            &self.indices,
        );
    }
}

struct TriangleIter<'m, V: 'm, I: BufferIndex + 'm> {
    mesh: &'m Mesh<V, I>,
    num: usize,
}

impl<'m, V: 'm, I: BufferIndex + 'm> Iterator for TriangleIter<'m, V, I> {
    type Item = TriangleRef<'m, V, I>;

    fn next(&mut self) -> Option<Self::Item> {
        let res = self.mesh.triangle(self.num);
        self.num += 1;
        res
    }
}

pub struct TriangleRef<'m, V: 'm, I: BufferIndex> {
    pub indices: (I, I, I),
    pub vertices: (&'m V, &'m V, &'m V),
}

#[derive(Debug, Eq, PartialEq)]
pub struct Mesh<V, I: BufferIndex> {
    crate vertices: Vec<V>,
    crate indices: Vec<I>,
    crate dirty: bool,
    crate gpu_mesh: Option<GpuMesh<V, I>>,
}

impl<V: Send + Sync + 'static, I: BufferIndex> Component for Mesh<V, I> {
    type Storage = HashMapStorage<Self>;
}

impl<V, I: BufferIndex> Default for Mesh<V, I> {
    fn default() -> Self {
        Mesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            dirty: false,
            gpu_mesh: None,
        }
    }
}

impl<V, I: BufferIndex> Mesh<V, I> {
    pub fn with_capacity(verts_cap: usize, indices_cap: usize) -> Self {
        Mesh {
            vertices: Vec::with_capacity(verts_cap),
            indices: Vec::with_capacity(indices_cap),
            ..Default::default()
        }
    }

    pub fn triangle(&self, triangle_index: usize) -> Option<TriangleRef<'_, V, I>> {
        if triangle_in_bounds(self.indices.len(), triangle_index) {
            let base = triangle_index * 3;
            Some(TriangleRef {
                vertices: (
                    self.vertices.get(self.indices[base].as_usize())?,
                    self.vertices.get(self.indices[base + 1].as_usize())?,
                    self.vertices.get(self.indices[base + 2].as_usize())?,
                ),
                indices: (
                    self.indices[base],
                    self.indices[base + 1],
                    self.indices[base + 2],
                ),
            })
        } else {
            None
        }
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() * 3
    }

    pub fn triangles(&self) -> impl Iterator<Item = TriangleRef<'_, V, I>> {
        TriangleIter { mesh: self, num: 0 }
    }

    pub fn needs_new_gpu_mesh(&self) -> bool {
        self.gpu_mesh.is_none() || self.dirty
    }
}

impl<V: Layout, I: BufferIndex> Mesh<V, I> {
    /// Creates the GPU buffers for this mesh and uploads the data in this mesh to it. Note that this has to clone the mesh data
    pub fn upload(&mut self, ctx: &Context, usage_type: UsageType) -> GlResult<()> {
        let mut mesh = GpuMesh::new(ctx)?;
        mesh.upload(ctx, &self.vertices[..], &self.indices[..], usage_type)?;
        self.gpu_mesh = Some(mesh);

        Ok(())
    }
}

fn triangle_in_bounds(triangle_list_len: usize, triangle_index: usize) -> bool {
    triangle_index < triangle_list_len / 3
}
