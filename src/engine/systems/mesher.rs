use shrev::EventChannel;
use engine::world::block::BlockRenderPrototype;
use engine::mesh::Mesh;
use engine::world::block::BlockRegistry;
use engine::world::block::BlockId;
use engine::ChunkPos;
use engine::world::VoxelWorld;
use specs::prelude::*;
use cgmath::{Point3, Vector2, Vector3};
use engine::world::chunk::SIZE;
use engine::world::Chunk;
use engine::components as c;

use nd;

pub struct ChunkMesher {
    // mesh_parts: HashMap<BlockId, Neighborhood<GlMesh<BlockVertex, u32>>>,
}

impl<'a> System<'a> for ChunkMesher {
    type SystemData = (
        WriteStorage<'a, c::DirtyMesh>,
        ReadStorage<'a, c::ChunkId>,
        ReadExpect<'a, VoxelWorld>,
        ReadExpect<'a, BlockRegistry>,
        Write<'a, EventChannel<(Entity, Mesh<BlockVertex, u32>)>>,
        Entities<'a>,
    );

    fn run(&mut self, (mut dirty_markers, chunk_ids, world, registry, mut mesh_channel, entities): Self::SystemData) {
        // let mut cleaned = vec![];
        for (_, c::ChunkId(pos), entity) in (&dirty_markers, &chunk_ids, &*entities).join() {
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
                println!("Chunk `{:?}` is ready for meshing", pos);
                let mut mesher = GreedyMesher::new(*pos, &world, &registry);
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
    X = 0, Y = 1, Z = 2
}

struct ChunkView<'c, T> {
    chunk: &'c Chunk<T>,
    axis: Axis,
}

impl<'c, T: Clone> ChunkView<'c, T> {
    fn to_local_space(&self, chunk_space: Point3<i32>) -> Point3<i32> {
        let (u, l, v) = chunk_space.into();
        match self.axis {
            Axis::X => Point3::new(v, l, u),
            Axis::Y => Point3::new(l, u, v),
            Axis::Z => Point3::new(u, v, l),
        }

        // Side::Up => Vector3::new(0, 1, 0),
        // Side::Right => Vector3::new(1, 0, 0),
        // Side::Front => Vector3::new(0, 0, 1),
        // Side::Down => Vector3::new(0, -1, 0),
        // Side::Left => Vector3::new(-1, 0, 0),
        // Side::Back => Vector3::new(0, 0, -1),
    }
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
    fn add(&mut self, face: VoxelFace, proto: BlockRenderPrototype, axis: Axis, top: bool, pos: Point3<i32>, width: usize, height: usize) {
        const NORMAL_QUAD_CW: &'static [u32] = &[0,1,2,3,2,1];
        
        let index = self.index;
        self.mesh.indices.extend(NORMAL_QUAD_CW.iter().map(|i| i + index));
        self.index += 4;

        let mut normal = Vector3::new(0.0, 0.0, 0.0);
        normal[axis as usize] = if top { 1.0 } else { -1.0 };

        let tile_offset = proto.texture_offsets[if top { axis as usize } else { 3 + axis as usize }];

        let mut push_vertex = |pos, uv, ao| self.mesh.vertices.push(BlockVertex {
            pos, uv, ao,
            normal, tile_offset,
            face: axis as i32,
        });

        let ao_pp = (face.corner_ao(VoxelFace::AO_POS_POS) as f32) / 3.0;
        let ao_pn = (face.corner_ao(VoxelFace::AO_POS_NEG) as f32) / 3.0;
        let ao_nn = (face.corner_ao(VoxelFace::AO_NEG_NEG) as f32) / 3.0;
        let ao_np = (face.corner_ao(VoxelFace::AO_NEG_POS) as f32) / 3.0;

        let Point3 { x, y, z }: Point3<f32> = pos.cast().unwrap();
        let (w, h) = (width as f32, height as f32);
        let fh = top as usize as f32;

        push_vertex(Vector3::new(x,     y + fh, z    ), Vector2::new(0.0, 0.0), ao_nn);
        push_vertex(Vector3::new(x + w, y + fh, z    ), Vector2::new(w,   0.0), ao_pn);
        push_vertex(Vector3::new(x,     y + fh, z + h), Vector2::new(0.0, h  ), ao_np);
        push_vertex(Vector3::new(x + w, y + fh, z + h), Vector2::new(w,   h  ), ao_pp);

        // BlockVertex {
        //     pos: Vector3::new(self.pos.x + x, self.pos.y + y, self.pos.z + z),
        //     norm: Vector3::new($nx as f32, $ny as f32, $nz as f32),
        //     face: $face,
        //     uv: Vector2::new($ou as f32, $ov as f32),
        //     tile: proto.texture_for_side(Side::$side),
        //     ao: $ao,
        // }
    }
}

fn window<I>(a: impl Iterator<Item=I> + Clone) -> impl Iterator<Item=(I, I)> {
    a.clone().zip(a.skip(1))
}

struct PaddedSlice<'c> {
    center: nd::ArrayView2<'c, BlockId>,

    top: nd::ArrayView1<'c, BlockId>,
    bottom: nd::ArrayView1<'c, BlockId>,
    right: nd::ArrayView1<'c, BlockId>,
    left: nd::ArrayView1<'c, BlockId>,

    top_right: BlockId,
    bottom_right: BlockId,
    bottom_left: BlockId,
    top_left: BlockId,
}

// impl<'c> PaddedSlice<'c> {
//     fn bottom_
// }

fn ao_value(side1: bool, corner: bool, side2: bool) -> u8 {
    if side1 && side2 { 0 } else {
        3 - (side1 as u8 + side2 as u8 + corner as u8)
    }
}

pub struct GreedyMesher<'w, 'r> {
    pos: Point3<i32>,
    world: &'w VoxelWorld,
    registry: &'r BlockRegistry,
    // chunk: &'w Chunk<BlockId>,
    next_slice: nd::Array2<VoxelFace>,
    previous_slice: nd::Array2<VoxelFace>,
    mesh_constructor: MeshConstructor,
}

impl<'w, 'r> GreedyMesher<'w, 'r> {
    fn new(pos: Point3<i32>, world: &'w VoxelWorld, registry: &'r BlockRegistry) -> Self {
        println!("Created mesher: pos={:?}", pos);
        GreedyMesher {
            pos, world, registry,
            next_slice: nd::Array2::default((SIZE, SIZE)),
            previous_slice: nd::Array2::default((SIZE, SIZE)),
            mesh_constructor: MeshConstructor {
                pos, index: 0, mesh: Default::default()
            }
        }
    }

    fn mesh(&mut self) {
        for cy in 0..SIZE as i32 {
            // compute "mask"
            for cx in 0..SIZE as i32 {
                for cz in 0..SIZE as i32 {
                    let pos = SIZE as i32 * self.pos + Vector3::new(cx, cy, cz);
                    let Point3 { x, y, z } = pos;

                    let block = self.world.get_block(pos);
                    let above = self.world.get_block(pos + Vector3::unit_y());
                    let below = self.world.get_block(pos - Vector3::unit_y());

                    let (mut top, mut bot) = <(VoxelFace, VoxelFace)>::default();

                    top.visible = !self.registry[above].opaque && self.registry[block].opaque;
                    bot.visible = !self.registry[below].opaque && self.registry[block].opaque;

                    if top.visible {
                        let is_opaque = |x, z| self.registry[self.world.get_block(Point3::new(x, y + 1, z))].opaque;

                        let neg_neg = is_opaque(x-1, z-1);
                        let neg_cen = is_opaque(x-1, z, );
                        let neg_pos = is_opaque(x-1, z+1);
                        let pos_neg = is_opaque(x+1, z-1);
                        let pos_cen = is_opaque(x+1, z, );
                        let pos_pos = is_opaque(x+1, z+1);
                        let cen_neg = is_opaque(x,   z-1);
                        let cen_pos = is_opaque(x,   z+1);

                        let face_pos_pos = ao_value(cen_pos, pos_pos, pos_cen); // c+ ++ +c
                        let face_pos_neg = ao_value(pos_cen, pos_neg, cen_neg); // +c +- c-
                        let face_neg_neg = ao_value(cen_neg, neg_neg, neg_cen); // c- -- -c
                        let face_neg_pos = ao_value(neg_cen, neg_pos, cen_pos); // -c -+ c+

                        top.ao =
                              face_pos_pos << VoxelFace::AO_POS_POS
                            | face_pos_neg << VoxelFace::AO_POS_NEG
                            | face_neg_neg << VoxelFace::AO_NEG_NEG
                            | face_neg_pos << VoxelFace::AO_NEG_POS
                    }

                    if bot.visible {
                        let is_opaque = |x, z| self.registry[self.world.get_block(Point3::new(x, y - 1, z))].opaque;

                        let neg_neg = is_opaque(x-1, z-1);
                        let neg_cen = is_opaque(x-1, z, );
                        let neg_pos = is_opaque(x-1, z+1);
                        let pos_neg = is_opaque(x+1, z-1);
                        let pos_cen = is_opaque(x+1, z, );
                        let pos_pos = is_opaque(x+1, z+1);
                        let cen_neg = is_opaque(x,   z-1);
                        let cen_pos = is_opaque(x,   z+1);

                        let face_pos_pos = ao_value(cen_pos, pos_pos, pos_cen); // c+ ++ +c
                        let face_pos_neg = ao_value(pos_cen, pos_neg, cen_neg); // +c +- c-
                        let face_neg_neg = ao_value(cen_neg, neg_neg, neg_cen); // c- -- -c
                        let face_neg_pos = ao_value(neg_cen, neg_pos, cen_pos); // -c -+ c+

                        bot.ao =
                              face_pos_pos << VoxelFace::AO_POS_POS
                            | face_pos_neg << VoxelFace::AO_POS_NEG
                            | face_neg_neg << VoxelFace::AO_NEG_NEG
                            | face_neg_pos << VoxelFace::AO_NEG_POS
                    }

                    let (cx, cz) = (cx as usize, cz as usize);

                    self.next_slice[(cx, cz)] = top;
                    self.previous_slice[(cx, cz)] = bot;
                }
            }

            // Generate mesh slice for "mask"
            for cx in 0..SIZE {
                for cz in 0..SIZE {
                    let (mut width, mut height) = (1, 1);
                    let start = self.next_slice[(cx, cz)];

                    // Don't bother with faces that were already added to the mesh, or with faces that can't be added to the mesh
                    if start.visited || !start.visible { continue; }

                    // While the quad is in chunk bounds and can merge with the next face, increase the width
                    while cx + width < SIZE && start.can_merge(self.next_slice[(cx + width, cz)]) { width += 1; }

                    // While the quad is in chunk bounds and all the next faces can merge, increase the height
                    while cz + height < SIZE {
                        let mut all_can_merge = true;

                        for xo in 0..width {
                            all_can_merge &= start.can_merge(self.next_slice[(cx + xo, cz + height)]);
                        }

                        if all_can_merge { height += 1; } else { break; }
                    }

                    // Mark all the faces underneath the quad as visited
                    for xo in 0..width {
                        for zo in 0..height {
                            self.next_slice[(cx + xo, cz + zo)].visited = true;
                        }
                    }

                    // We expanded the quad, now add it to the mesh builder
                    let pos = SIZE as i32 * self.pos + Vector3::new(cx as i32, cy, cz as i32);
                    self.mesh_constructor.add(
                        start,
                        self.registry[self.world.get_block(pos)],
                        Axis::Y,
                        true,
                        pos,
                        width,
                        height
                    );
                }
            }

            for cx in 0..SIZE {
                for cz in 0..SIZE {
                    let (mut width, mut height) = (1, 1);
                    let start = self.next_slice[(cx, cz)];

                    // Don't bother with faces that were already added to the mesh, or with faces that can't be added to the mesh
                    if start.visited || !start.visible { continue; }

                    // While the quad is in chunk bounds and can merge with the next face, increase the width
                    while cx + width < SIZE && start.can_merge(self.previous_slice[(cx + width, cz)]) { width += 1; }

                    // While the quad is in chunk bounds and all the next faces can merge, increase the height
                    while cz + height < SIZE {
                        let mut all_can_merge = true;

                        for xo in 0..width {
                            all_can_merge &= start.can_merge(self.previous_slice[(cx + xo, cz + height)]);
                        }

                        if all_can_merge { height += 1; } else { break; }
                    }

                    // Mark all the faces underneath the quad as visited
                    for xo in 0..width {
                        for zo in 0..height {
                            self.previous_slice[(cx + xo, cz + zo)].visited = true;
                        }
                    }

                    // We expanded the quad, now add it to the mesh builder
                    let pos = SIZE as i32 * self.pos + Vector3::new(cx as i32, cy, cz as i32);
                    self.mesh_constructor.add(
                        start,
                        self.registry[self.world.get_block(pos)],
                        Axis::Y,
                        false,
                        pos,
                        width,
                        height
                    );
                }
            }
        }

        // for each dimension...
        // for &axis in &[Axis::X, Axis::Y, Axis::Z] { // X=0, Y=1, Z=2
        //     // bottom: slice for -axis + 0
        //     // top: slice for +axis + chunk_size - 1
        //     self.chunk.data.axis_iter(nd::Axis(axis as usize))
        //         .map(|slice|);
        //     // for (layer, (slice, next_slice)) in window(self.chunk.data.axis_iter(nd::Axis(axis as usize))).enumerate() {
        //     //     for ((idx, item), next_item) in slice.indexed_iter().zip(next_slice.iter()) {
        //     //         // let (front, back) = self.face(axis, idx.0, idx.1, layer);
        //     //         // self.slice_forward[idx] = front;
        //     //         // self.slice_backward[idx] = back;
        //     //     }
        //     // }
        // }
    }
}
