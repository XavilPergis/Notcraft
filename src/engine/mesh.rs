use gl_api::buffer::{Buffer, UsageType};
use gl_api::context::Context;
use gl_api::error::GlResult;
use gl_api::layout::Layout;
use gl_api::shader::program::LinkedProgram;
use gl_api::BufferIndex;
use gl_api::PrimitiveType;

#[derive(Debug)]
pub struct GlMesh<V: Layout, I: BufferIndex> {
    ctx: Context,
    vertices: Buffer<V>,
    indices: Buffer<I>,
}

impl<V: Layout, I: BufferIndex> GlMesh<V, I> {
    pub fn new(ctx: &Context) -> GlResult<Self> {
        let vbo = Buffer::new(ctx);
        let ibo = Buffer::new(ctx);

        Ok(GlMesh {
            indices: ibo,
            vertices: vbo,
            ctx: ctx.clone(),
        })
    }

    pub fn upload<IV: AsRef<[V]>, II: AsRef<[I]>>(
        &mut self,
        vertices: IV,
        indices: II,
        usage_type: UsageType,
    ) -> GlResult<()> {
        self.vertices.upload(vertices.as_ref(), usage_type)?;
        self.indices.upload(indices.as_ref(), usage_type)?;

        Ok(())
    }

    pub fn draw_with(&self, program: &LinkedProgram) {
        self.ctx.draw_elements(
            PrimitiveType::Triangles,
            program,
            &self.vertices,
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

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Mesh<V, I: BufferIndex> {
    crate vertices: Vec<V>,
    crate indices: Vec<I>,
}

impl<V, I: BufferIndex> Default for Mesh<V, I> {
    fn default() -> Self {
        Mesh {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }
}

impl<V, I: BufferIndex> Mesh<V, I> {
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
}

impl<V: Layout, I: BufferIndex> Mesh<V, I> {
    /// Creates the GPU buffers for this mesh and uploads the data in this mesh to it. Note that this has to clone the data
    pub fn to_gl_mesh(&self, ctx: &Context, usage_type: UsageType) -> GlResult<GlMesh<V, I>> {
        let mut mesh = GlMesh::new(ctx)?;
        mesh.upload(&self.vertices[..], &self.indices[..], usage_type)?;
        Ok(mesh)
    }
}

fn triangle_in_bounds(triangle_list_len: usize, triangle_index: usize) -> bool {
    triangle_index < triangle_list_len / 3
}
