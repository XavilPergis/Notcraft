// // use rendy::{hal::Primitive, mesh};
// // use specs::prelude::*;
// // use std::{borrow::Cow, collections::HashMap};

// // /// A type-erased, owned vertex buffer.
// // #[derive(Clone, Debug, PartialEq)]
// // pub struct AttributeBuffer<'a> {
// //     data: *const u8,
// //     len: usize,
// //     format: VertexFormat<'a>,
// // }

// // impl<T> From<Box<[T]>> for AttributeBuffer
// // where
// //     T: AsVertex,
// // {
// //     fn from(data: Box<T>) -> Self {
// //         AttributeBuffer {
// //             len: data.len() * std::mem::size_of::<T>(),
// //             data: data.into_raw() as *const _,
// //             format: T::VERTEX,
// //         }
// //     }
// // }

// // #[derive(Clone, Debug, PartialEq)]
// // pub enum IndexBuffer {
// //     U16(Box<[u16]>),
// //     U32(Box<[u32]>),
// // }

// // #[derive(Clone, Debug, PartialEq)]
// // pub struct Mesh {
// //     // Indices are special
// //     pub(crate) indices: Option<IndexBuffer>,
// //     pub(crate) attributes: HashMap<Cow<'static, str>, AttributeBuffer>,
// //     pub primitive: Primitive,
// // }

// // impl Component for Mesh {
// //     type Storage = FlaggedStorage<Self, DenseVecStorage<Self>>;
// // }

// // impl Mesh {
// //     pub const COLORS: &'static str = "color";
// //     pub const NORMALS: &'static str = "normal";
// //     pub const POSITIONS: &'static str = "position";
// //     pub const TANGENTS: &'static str = "tangent";
// //     pub const UVS: &'static str = "uv";

// //     // pub fn set_indices<I>(&mut self, indices: )

// //     pub fn insert<S, A>(&mut self, name: S, attrib: A) ->
// // Option<AttributeBuffer>     where
// //         S: Into<Cow<'static, str>>,
// //         A: Into<AttributeBuffer>,
// //     {
// //         self.attributes.insert(name.into(), attrib.into())
// //     }

// //     pub fn into_gpu_mesh<B: Backend>(&self, queue: QueueId, factory:
// // &Factory<B>) -> mesh::Mesh<B> {         //
// //     }
// // }

// // use cgmath::InnerSpace;
// use glium::{
//     backend::Facade,
//     index::{Index, PrimitiveType},
//     *,
// };
// use nalgebra::{vector, Point2, Point3, Vector3};
// use num_traits::{AsPrimitive, CheckedAdd, FromPrimitive};

// #[derive(Debug)]
// pub struct GpuMesh<V: Copy, I: Index> {
//     pub vertices: VertexBuffer<V>,
//     pub indices: IndexBuffer<I>,
// }

// #[derive(Debug)]
// pub enum BufferError {
//     Vertex(vertex::BufferCreationError),
//     Index(index::BufferCreationError),
// }

// impl<V: Copy + Vertex, I: Index> GpuMesh<V, I> {
//     pub fn empty_dynamic<F: Facade>(
//         ctx: &F,
//         primitive_type: PrimitiveType,
//         reserve_vert: usize,
//         reserve_idx: usize,
//     ) -> Result<Self, BufferError> {
//         Ok(GpuMesh {
//             vertices: VertexBuffer::empty_dynamic(ctx, reserve_vert)
//                 .map_err(BufferError::Vertex)?,
//             indices: IndexBuffer::empty_dynamic(ctx, primitive_type, reserve_idx)
//                 .map_err(BufferError::Index)?,
//         })
//     }
// }

// #[derive(Clone, Debug, Default)]
// pub struct Mesh<V, I> {
//     vertices: Vec<V>,
//     indices: Vec<I>,
// }

// impl<V, I> Mesh<V, I> {
//     #[must_use]
//     pub fn add<VI, II>(&mut self, verts: VI, indices: II) -> bool
//     where
//         VI: IntoIterator<Item = V>,
//         II: IntoIterator<Item = I>,
//         I: FromPrimitive + CheckedAdd + Copy + 'static,
//     {
//         match I::from_usize(self.vertices.len()) {
//             Some(base) => {
//                 for index in indices.into_iter() {
//                     match base.checked_add(&index) {
//                         None => return false,
//                         Some(sum) => self.indices.push(sum),
//                     }
//                 }
//                 self.vertices.extend(verts);
//                 true
//             }
//             None => false,
//         }
//     }

//     pub fn to_gpu_mesh<F: Facade>(
//         &self,
//         ctx: &F,
//         primitive: PrimitiveType,
//     ) -> Result<GpuMesh<V, I>, BufferError>
//     where
//         V: Copy + Vertex,
//         I: Index,
//     {
//         let vertices = VertexBuffer::new(ctx, &self.vertices).map_err(BufferError::Vertex)?;
//         let indices =
//             IndexBuffer::new(ctx, primitive, &self.indices).map_err(BufferError::Index)?;

//         Ok(GpuMesh { vertices, indices })
//     }

//     /// Recalculate the normal and tangent vectors (from which we can derive the
//     /// rest of the basis) for each triangle in the mesh
//     pub fn recalculate_tangent_bases(&mut self)
//     where
//         I: AsPrimitive<usize>,
//         V: GeometryVertex,
//     {
//         // Zero vertex normal and tangent vectors
//         self.vertices.iter_mut().for_each(|vertex| {
//             *vertex.normal_mut() = vector!(0.0, 0.0, 0.0);
//             *vertex.tangent_mut() = vector!(0.0, 0.0, 0.0);
//         });

//         // Sum all the unit normals and tangents for each vertex. for non-"flat" meshes
//         // where different indices point to the same vertex, the vector will not be a
//         // normal vector after the summation. We compensate for this, though, in the
//         // next step by normalizing all the basis vectors
//         for (a, b, c) in self
//             .indices
//             .chunks(3)
//             .map(|i| (i[0].as_(), i[1].as_(), i[2].as_()))
//         {
//             let triangle_norm = triangle_normal(self, a, b, c);
//             let triangle_tang = triangle_tangent(self, a, b, c);

//             *self.vertices[a].normal_mut() -= triangle_norm;
//             *self.vertices[a].tangent_mut() -= triangle_tang;

//             *self.vertices[b].normal_mut() -= triangle_norm;
//             *self.vertices[b].tangent_mut() -= triangle_tang;

//             *self.vertices[c].normal_mut() -= triangle_norm;
//             *self.vertices[c].tangent_mut() -= triangle_tang;
//         }

//         // Normalize the normals and tangents
//         self.vertices.iter_mut().for_each(|vertex| {
//             *vertex.normal_mut() = vertex.normal().normalize();
//             *vertex.tangent_mut() = vertex.tangent().normalize();
//         });
//     }
// }

// pub trait GeometryVertex: Copy + 'static {
//     fn position(self) -> Point3<f32>;
//     fn uv(self) -> Point2<f32>;
//     fn normal(self) -> Vector3<f32>;
//     fn tangent(self) -> Vector3<f32>;

//     fn position_mut(&mut self) -> &mut Point3<f32>;
//     fn uv_mut(&mut self) -> &mut Point2<f32>;
//     fn normal_mut(&mut self) -> &mut Vector3<f32>;
//     fn tangent_mut(&mut self) -> &mut Vector3<f32>;
// }

// macro_rules! impl_geom_vertex {
//     ($type:ident, $pos:ident, $uv:ident, $normal:ident, $tangent:ident) => {
//         impl crate::engine::render::mesh::GeometryVertex for $type {
//             #[inline(always)]
//             fn position(self) -> ::cgmath::Point3<f32> {
//                 self.$pos.into()
//             }

//             #[inline(always)]
//             fn uv(self) -> ::cgmath::Point2<f32> {
//                 self.$uv.into()
//             }

//             #[inline(always)]
//             fn normal(self) -> ::cgmath::Vector3<f32> {
//                 self.$normal.into()
//             }

//             #[inline(always)]
//             fn tangent(self) -> ::cgmath::Vector3<f32> {
//                 self.$tangent.into()
//             }

//             #[inline(always)]
//             fn position_mut(&mut self) -> &mut ::cgmath::Point3<f32> {
//                 From::from(&mut self.$pos)
//             }

//             #[inline(always)]
//             fn uv_mut(&mut self) -> &mut ::cgmath::Point2<f32> {
//                 From::from(&mut self.$uv)
//             }

//             #[inline(always)]
//             fn normal_mut(&mut self) -> &mut ::cgmath::Vector3<f32> {
//                 From::from(&mut self.$normal)
//             }

//             #[inline(always)]
//             fn tangent_mut(&mut self) -> &mut ::cgmath::Vector3<f32> {
//                 From::from(&mut self.$tangent)
//             }
//         }
//     };
// }

// fn triangle_normal<V: GeometryVertex, I>(
//     mesh: &Mesh<V, I>,
//     a: usize,
//     b: usize,
//     c: usize,
// ) -> Vector3<f32> {
//     let pa = mesh.vertices[a].position();
//     let pb = mesh.vertices[b].position();
//     let pc = mesh.vertices[c].position();

//     let edge_b = pb - pa;
//     let edge_c = pc - pa;

//     edge_b.cross(&edge_c)
// }

// fn triangle_tangent<V: GeometryVertex, I>(
//     mesh: &Mesh<V, I>,
//     a: usize,
//     b: usize,
//     c: usize,
// ) -> Vector3<f32> {
//     let p1 = mesh.vertices[a].position();
//     let p2 = mesh.vertices[b].position();
//     let p3 = mesh.vertices[c].position();

//     let t1 = mesh.vertices[a].uv();
//     let t2 = mesh.vertices[b].uv();
//     let t3 = mesh.vertices[c].uv();

//     let delta_pos_2 = p2 - p1;
//     let delta_pos_3 = p3 - p1;

//     let delta_uv_2 = t2 - t1;
//     let delta_uv_3 = t3 - t1;

//     let r = 1.0 / (delta_uv_2.x * delta_uv_3.y - delta_uv_2.y * delta_uv_3.x);
//     r * (delta_pos_2 * delta_uv_3.y - delta_pos_3 * delta_uv_2.y)
// }
