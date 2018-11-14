use cgmath::{Point3, Vector2, Vector3, Vector4};
use engine::components as comp;
use engine::mesh::Mesh;
use engine::systems::debug_render::Shape;
use engine::world::block::{BlockRegistry, BlockRenderPrototype};
use engine::world::chunk::SIZE;
use engine::world::VoxelWorld;
use shrev::EventChannel;
use specs::prelude::*;

use nd;

pub struct ChunkMesher;

impl ChunkMesher {
    pub fn new() -> Self {
        ChunkMesher
    }
}

impl<'a> System<'a> for ChunkMesher {
    type SystemData = (
        WriteStorage<'a, comp::DirtyMesh>,
        ReadStorage<'a, comp::ChunkId>,
        ReadExpect<'a, VoxelWorld>,
        ReadExpect<'a, BlockRegistry>,
        Write<'a, EventChannel<(Entity, Mesh<BlockVertex, u32>)>>,
        WriteExpect<'a, EventChannel<Shape>>,
        Entities<'a>,
    );

    fn run(
        &mut self,
        (
            mut dirty_markers,
            chunk_ids,
            world,
            registry,
            mut mesh_channel,
            mut debug_channel,
            entities,
        ): Self::SystemData,
    ) {
        for (_, &comp::ChunkId(pos), entity) in (&dirty_markers, &chunk_ids, &*entities).join() {
            debug_channel.single_write(Shape::Chunk(2.0, pos, Vector4::new(0.5, 0.5, 1.0, 1.0)));
            // Ensure that the surrounding volume of chunks exist before meshing this one.
            let mut surrounded = true;
            for x in -1..=1 {
                for y in -1..=1 {
                    for z in -1..=1 {
                        surrounded &= world.chunk_exists(pos + Vector3::new(x, y, z));
                    }
                }
            }

            if surrounded {
                debug_channel.single_write(Shape::Chunk(
                    2.0,
                    pos,
                    Vector4::new(1.0, 0.0, 1.0, 1.0),
                ));
                trace!("Chunk {:?} is ready for meshing", pos);
                let mut mesher = GreedyMesher::new(pos, &world, &registry);
                mesher.mesh();
                mesh_channel.single_write((entity, mesher.mesh_constructor.mesh));
                dirty_markers.remove(entity);
                break;
                // cleaned.push(entity);
            }
        }

        // for entity in cleaned {
        //     println!("Marked `{:?}` as cleaned", entity);
        //     dirty_markers.remove(entity);
        // }
    }
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
enum Axis {
    X = 0,
    Y = 1,
    Z = 2,
}

// ++ +- -- -+
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
struct VoxelFace {
    ao: u8,
    visible: bool,
    visited: bool,
}

impl VoxelFace {
    const AO_POS_POS: u8 = 6;
    const AO_POS_NEG: u8 = 4;
    const AO_NEG_NEG: u8 = 2;
    const AO_NEG_POS: u8 = 0;

    fn corner_ao(&self, bits: u8) -> u8 {
        (self.ao & (3 << bits)) >> bits
    }

    fn can_merge(&self, other: VoxelFace) -> bool {
        *self == other
    }
}

vertex! {
    vertex BlockVertex {
        pos: Vector3<f32>,
        normal: Vector3<f32>,
        face: i32,
        tile_offset: Vector2<f32>,
        uv: Vector2<f32>,
        ao: f32,
    }
}

#[derive(Clone, Debug)]
struct MeshConstructor {
    pos: Point3<i32>,
    index: u32,
    mesh: Mesh<BlockVertex, u32>,
}

impl MeshConstructor {
    fn add(
        &mut self,
        face: VoxelFace,
        proto: &BlockRenderPrototype,
        axis: Axis,
        top: bool,
        pos: Point3<i32>,
        width: usize,
        height: usize,
    ) {
        const NORMAL_QUAD_CW: &'static [u32] = &[0, 1, 2, 3, 2, 1];
        const FLIPPED_QUAD_CW: &'static [u32] = &[3, 2, 0, 0, 1, 3];
        const NORMAL_QUAD_CCW: &'static [u32] = &[2, 1, 0, 1, 2, 3];
        const FLIPPED_QUAD_CCW: &'static [u32] = &[0, 2, 3, 3, 1, 0];

        let ao_pp = (face.corner_ao(VoxelFace::AO_POS_POS) as f32) / 3.0;
        let ao_pn = (face.corner_ao(VoxelFace::AO_POS_NEG) as f32) / 3.0;
        let ao_nn = (face.corner_ao(VoxelFace::AO_NEG_NEG) as f32) / 3.0;
        let ao_np = (face.corner_ao(VoxelFace::AO_NEG_POS) as f32) / 3.0;
        let flipped = ao_pp + ao_nn > ao_pn + ao_np;

        let quad = match (flipped, top, axis) {
            (false, false, Axis::X) => NORMAL_QUAD_CW,
            (false, true, Axis::X) => NORMAL_QUAD_CCW,
            (true, true, Axis::X) => FLIPPED_QUAD_CCW,
            (true, false, Axis::X) => FLIPPED_QUAD_CW,

            (false, false, Axis::Y) => NORMAL_QUAD_CCW,
            (false, true, Axis::Y) => NORMAL_QUAD_CW,
            (true, true, Axis::Y) => FLIPPED_QUAD_CW,
            (true, false, Axis::Y) => FLIPPED_QUAD_CCW,

            (false, false, Axis::Z) => NORMAL_QUAD_CCW,
            (false, true, Axis::Z) => NORMAL_QUAD_CW,
            (true, true, Axis::Z) => FLIPPED_QUAD_CW,
            (true, false, Axis::Z) => FLIPPED_QUAD_CCW,
        };

        let index = self.index;
        self.mesh.indices.extend(quad.iter().map(|i| i + index));
        self.index += 4;

        let mut normal = Vector3::new(0.0, 0.0, 0.0);
        normal[axis as usize] = if top { 1.0 } else { -1.0 };

        let tile_offset = proto.texture_offsets[if top {
            axis as usize
        } else {
            3 + axis as usize
        }];

        let mut push_vertex = |pos, uv, ao| {
            self.mesh.vertices.push(BlockVertex {
                pos,
                uv,
                ao,
                normal,
                tile_offset,
                face: axis as i32,
            })
        };

        let Point3 { x, y, z }: Point3<f32> = pos.cast().unwrap();
        let (w, h) = (width as f32, height as f32);
        let fh = top as usize as f32;

        if axis == Axis::X {
            push_vertex(
                Vector3::new(x + fh, y + w, z),
                Vector2::new(0.0, 0.0),
                ao_pn,
            );
            push_vertex(
                Vector3::new(x + fh, y + w, z + h),
                Vector2::new(h, 0.0),
                ao_pp,
            );
            push_vertex(Vector3::new(x + fh, y, z), Vector2::new(0.0, w), ao_nn);
            push_vertex(Vector3::new(x + fh, y, z + h), Vector2::new(h, w), ao_np);
        }

        if axis == Axis::Y {
            push_vertex(Vector3::new(x, y + fh, z), Vector2::new(0.0, 0.0), ao_nn);
            push_vertex(Vector3::new(x + w, y + fh, z), Vector2::new(w, 0.0), ao_np);
            push_vertex(Vector3::new(x, y + fh, z + h), Vector2::new(0.0, h), ao_pn);
            push_vertex(
                Vector3::new(x + w, y + fh, z + h),
                Vector2::new(w, h),
                ao_pp,
            );
        }

        if axis == Axis::Z {
            push_vertex(
                Vector3::new(x, y + h, z + fh),
                Vector2::new(0.0, 0.0),
                ao_np,
            );
            push_vertex(
                Vector3::new(x + w, y + h, z + fh),
                Vector2::new(w, 0.0),
                ao_pp,
            );
            push_vertex(Vector3::new(x, y, z + fh), Vector2::new(0.0, h), ao_nn);
            push_vertex(Vector3::new(x + w, y, z + fh), Vector2::new(w, h), ao_pn);
        }
    }
}

fn ao_value(side1: bool, corner: bool, side2: bool) -> u8 {
    if side1 && side2 {
        0
    } else {
        3 - (side1 as u8 + side2 as u8 + corner as u8)
    }
}

fn slice_to_local(axis: Axis, u: i32, v: i32, l: i32) -> Vector3<i32> {
    match axis {
        Axis::X => Vector3::new(l, u, v),
        Axis::Y => Vector3::new(u, l, v),
        Axis::Z => Vector3::new(u, v, l),
    }
}

pub struct GreedyMesher<'w, 'r> {
    pos: Point3<i32>,
    world: &'w VoxelWorld,
    registry: &'r BlockRegistry,
    next_slice: nd::Array2<VoxelFace>,
    previous_slice: nd::Array2<VoxelFace>,
    mesh_constructor: MeshConstructor,
}

impl<'w, 'r> GreedyMesher<'w, 'r> {
    fn new(pos: Point3<i32>, world: &'w VoxelWorld, registry: &'r BlockRegistry) -> Self {
        GreedyMesher {
            pos,
            world,
            registry,
            next_slice: nd::Array2::default((SIZE, SIZE)),
            previous_slice: nd::Array2::default((SIZE, SIZE)),
            mesh_constructor: MeshConstructor {
                pos,
                index: 0,
                mesh: Default::default(),
            },
        }
    }

    fn process_slices(&mut self, axis: Axis, layer: i32) {
        for (slice, side) in ::std::iter::once((&mut self.previous_slice, false))
            .chain(::std::iter::once((&mut self.next_slice, true)))
        {
            for cu in 0..SIZE {
                for cv in 0..SIZE {
                    let (mut width, mut height) = (1, 1);
                    let start = slice[(cu, cv)];

                    // Don't bother with faces that were already added to the mesh, or with faces that can't be added to the mesh
                    if start.visited || !start.visible {
                        continue;
                    }

                    // While the quad is in chunk bounds and can merge with the next face, increase the width
                    while cu + width < SIZE && start.can_merge(slice[(cu + width, cv)]) {
                        width += 1;
                    }

                    // While the quad is in chunk bounds and all the next faces can merge, increase the height
                    while cv + height < SIZE {
                        let mut all_can_merge = true;

                        for xo in 0..width {
                            all_can_merge &= start.can_merge(slice[(cu + xo, cv + height)]);
                        }

                        if all_can_merge {
                            height += 1;
                        } else {
                            break;
                        }
                    }

                    // Mark all the faces underneath the quad as visited
                    for xo in 0..width {
                        for zo in 0..height {
                            slice[(cu + xo, cv + zo)].visited = true;
                        }
                    }

                    // We expanded the quad, now add it to the mesh builder
                    let pos =
                        SIZE as i32 * self.pos + slice_to_local(axis, cu as i32, cv as i32, layer);
                    self.mesh_constructor.add(
                        start,
                        &self.registry[self.world.get_block(pos)],
                        axis,
                        side,
                        pos,
                        width,
                        height,
                    );
                }
            }
        }
    }

    fn face_ao(&self, pos: Point3<i32>, axis: Axis, offset: i32) -> u8 {
        let mut dir = Vector3::new(0, 0, 0);
        dir[axis as usize] = offset;

        let r = |u, v| {
            let mut vec = Vector3::new(0, 0, 0);
            vec[(axis as usize + 1) % 3] = u;
            vec[(axis as usize + 2) % 3] = v;
            vec
        };

        let is_opaque = |pos| self.registry[self.world.get_block(pos)].opaque;

        let neg_neg = is_opaque(pos + dir + r(-1, -1));
        let neg_cen = is_opaque(pos + dir + r(-1, 0));
        let neg_pos = is_opaque(pos + dir + r(-1, 1));
        let pos_neg = is_opaque(pos + dir + r(1, -1));
        let pos_cen = is_opaque(pos + dir + r(1, 0));
        let pos_pos = is_opaque(pos + dir + r(1, 1));
        let cen_neg = is_opaque(pos + dir + r(0, -1));
        let cen_pos = is_opaque(pos + dir + r(0, 1));

        let face_pos_pos = ao_value(cen_pos, pos_pos, pos_cen); // c+ ++ +c
        let face_pos_neg = ao_value(pos_cen, pos_neg, cen_neg); // +c +- c-
        let face_neg_neg = ao_value(cen_neg, neg_neg, neg_cen); // c- -- -c
        let face_neg_pos = ao_value(neg_cen, neg_pos, cen_pos); // -c -+ c+

        face_pos_pos << VoxelFace::AO_POS_POS
            | face_pos_neg << VoxelFace::AO_POS_NEG
            | face_neg_neg << VoxelFace::AO_NEG_NEG
            | face_neg_pos << VoxelFace::AO_NEG_POS
    }

    fn mesh(&mut self) {
        let size = SIZE as i32;

        for cx in 0..size {
            let mut slice_has_faces = false;
            // compute "mask"
            for cy in 0..size {
                for cz in 0..size {
                    let pos = size * self.pos + Vector3::new(cx, cy, cz);
                    let mut bot_top = <[VoxelFace; 2]>::default();
                    let block = self.world.get_block(pos);

                    for (idx, &dir) in (&[-1, 1]).iter().enumerate() {
                        let side = &mut bot_top[idx];
                        let above = self.world.get_block(pos + Vector3::new(dir, 0, 0)); // HERE
                        side.visible = !self.registry[above].opaque && self.registry[block].opaque;
                        if side.visible {
                            slice_has_faces = true;
                            side.ao = self.face_ao(pos, Axis::X, dir);
                        }
                    }

                    let (cy, cz) = (cy as usize, cz as usize);

                    self.previous_slice[(cy, cz)] = bot_top[0];
                    self.next_slice[(cy, cz)] = bot_top[1];
                }
            }

            // Generate mesh slice for "mask"
            if slice_has_faces {
                self.process_slices(Axis::X, cx);
            }
        }

        for cy in 0..size {
            let mut slice_has_faces = false;
            // compute "mask"
            for cx in 0..size {
                for cz in 0..size {
                    let pos = size * self.pos + Vector3::new(cx, cy, cz);
                    let mut bot_top = <[VoxelFace; 2]>::default();
                    let block = self.world.get_block(pos);

                    for (idx, &dir) in (&[-1, 1]).iter().enumerate() {
                        let side = &mut bot_top[idx];
                        let above = self.world.get_block(pos + Vector3::new(0, dir, 0));
                        side.visible = !self.registry[above].opaque && self.registry[block].opaque;
                        if side.visible {
                            slice_has_faces = true;
                            side.ao = self.face_ao(pos, Axis::Y, dir);
                        }
                    }

                    let (cx, cz) = (cx as usize, cz as usize);

                    self.previous_slice[(cx, cz)] = bot_top[0];
                    self.next_slice[(cx, cz)] = bot_top[1];
                }
            }

            // Generate mesh slice for "mask"
            if slice_has_faces {
                self.process_slices(Axis::Y, cy);
            }
        }

        for cz in 0..size {
            let mut slice_has_faces = false;
            // compute "mask"
            for cx in 0..size {
                for cy in 0..size {
                    let pos = size * self.pos + Vector3::new(cx, cy, cz);
                    let mut bot_top = <[VoxelFace; 2]>::default();
                    let block = self.world.get_block(pos);

                    for (idx, &dir) in (&[-1, 1]).iter().enumerate() {
                        let side = &mut bot_top[idx];
                        let above = self.world.get_block(pos + Vector3::new(0, 0, dir));
                        side.visible = !self.registry[above].opaque && self.registry[block].opaque;
                        if side.visible {
                            slice_has_faces = true;
                            side.ao = self.face_ao(pos, Axis::Z, dir);
                        }
                    }

                    let (cx, cy) = (cx as usize, cy as usize);

                    self.previous_slice[(cx, cy)] = bot_top[0];
                    self.next_slice[(cx, cy)] = bot_top[1];
                }
            }

            // Generate mesh slice for "mask"
            if slice_has_faces {
                self.process_slices(Axis::Z, cz);
            }
        }
    }
}
