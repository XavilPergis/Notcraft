use cgmath::{Point3, Vector2, Vector3, Vector4};
use engine::components as comp;
use engine::mesh::Mesh;
use engine::systems::debug_render::DebugAccumulator;
use engine::systems::debug_render::Shape;
use engine::world::chunk::SIZE;
use engine::world::BlockPos;
use engine::world::ChunkPos;
use engine::world::VoxelWorld;
use engine::Side;
use shrev::EventChannel;
use specs::prelude::*;

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
        Write<'a, EventChannel<(Entity, Mesh<BlockVertex, u32>)>>,
        ReadExpect<'a, DebugAccumulator>,
        Entities<'a>,
    );

    fn run(
        &mut self,
        (mut dirty_markers, chunk_ids, world, mut mesh_channel, debug, entities): Self::SystemData,
    ) {
        for (_, &comp::ChunkId(pos), entity) in (&dirty_markers, &chunk_ids, &*entities).join() {
            let mut section = debug.section("mesher");
            section.draw(Shape::Chunk(2.0, pos, Vector4::new(0.5, 0.5, 1.0, 1.0)));
            // Ensure that the surrounding volume of chunks exist before meshing this one.
            let mut surrounded = true;
            for x in -1..=1 {
                for y in -1..=1 {
                    for z in -1..=1 {
                        surrounded &= world.chunk_exists(pos.offset((x, y, z)));
                    }
                }
            }

            if surrounded {
                section.draw(Shape::Chunk(2.0, pos, Vector4::new(1.0, 0.0, 1.0, 1.0)));
                trace!("Chunk {:?} is ready for meshing", pos);
                let mut mesher = CullMesher::new(pos, &world);
                mesher.mesh();
                mesh_channel.single_write((entity, mesher.mesh_constructor.mesh));
                dirty_markers.remove(entity);
                break;
            }
        }
    }
}

struct CullMesher<'w> {
    pos: ChunkPos,
    world: &'w VoxelWorld,
    mesh_constructor: MeshConstructor<'w>,
}

impl<'w> CullMesher<'w> {
    fn new(pos: ChunkPos, world: &'w VoxelWorld) -> Self {
        CullMesher {
            pos,
            world,
            mesh_constructor: MeshConstructor {
                index: 0,
                mesh: Default::default(),
                world,
            },
        }
    }

    fn face_ao(&self, pos: BlockPos, side: Side) -> FaceAo {
        let is_opaque = |pos| self.world.get_block_properties(pos).unwrap().opaque;

        let neg_neg = is_opaque(pos.offset(side.uvl_to_xyz(-1, -1, 1)));
        let neg_cen = is_opaque(pos.offset(side.uvl_to_xyz(-1, 0, 1)));
        let neg_pos = is_opaque(pos.offset(side.uvl_to_xyz(-1, 1, 1)));
        let pos_neg = is_opaque(pos.offset(side.uvl_to_xyz(1, -1, 1)));
        let pos_cen = is_opaque(pos.offset(side.uvl_to_xyz(1, 0, 1)));
        let pos_pos = is_opaque(pos.offset(side.uvl_to_xyz(1, 1, 1)));
        let cen_neg = is_opaque(pos.offset(side.uvl_to_xyz(0, -1, 1)));
        let cen_pos = is_opaque(pos.offset(side.uvl_to_xyz(0, 1, 1)));

        let face_pos_pos = ao_value(cen_pos, pos_pos, pos_cen); // c+ ++ +c
        let face_pos_neg = ao_value(pos_cen, pos_neg, cen_neg); // +c +- c-
        let face_neg_neg = ao_value(cen_neg, neg_neg, neg_cen); // c- -- -c
        let face_neg_pos = ao_value(neg_cen, neg_pos, cen_pos); // -c -+ c+

        FaceAo(
            face_pos_pos << FaceAo::AO_POS_POS
                | face_pos_neg << FaceAo::AO_POS_NEG
                | face_neg_neg << FaceAo::AO_NEG_NEG
                | face_neg_pos << FaceAo::AO_NEG_POS,
        )
    }

    fn is_not_occluded(&self, pos: BlockPos, offset: Vector3<i32>) -> bool {
        let cur = self.world.get_block_properties(pos).unwrap();
        let other = self.world.get_block_properties(pos.offset(offset)).unwrap();

        cur.opaque && !other.opaque
    }

    fn mesh(&mut self) {
        let size = SIZE as i32;
        let base = self.pos.base();
        for x in 0..size {
            for y in 0..size {
                for z in 0..size {
                    let pos = base.offset((x, y, z));
                    // let block = self.world.get_block(pos);

                    for side in &[
                        Side::Top,
                        Side::Bottom,
                        Side::Left,
                        Side::Right,
                        Side::Front,
                        Side::Back,
                    ] {
                        if self.is_not_occluded(pos, side.normal()) {
                            self.mesh_constructor
                                .add(self.face_ao(pos, *side), *side, pos)
                        }
                    }
                }
            }
        }
    }
}

vertex! {
    vertex BlockVertex {
        pos: Point3<f32>,
        normal: Vector3<f32>,
        face: i32,
        tile_offset: Vector2<f32>,
        uv: Vector2<f32>,
        ao: f32,
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
struct FaceAo(u8);

impl FaceAo {
    const AO_POS_POS: u8 = 6;
    const AO_POS_NEG: u8 = 4;
    const AO_NEG_NEG: u8 = 2;
    const AO_NEG_POS: u8 = 0;

    fn corner_ao(&self, bits: u8) -> u8 {
        (self.0 & (3 << bits)) >> bits
    }
}

#[derive(Clone, Debug)]
struct MeshConstructor<'w> {
    index: u32,
    mesh: Mesh<BlockVertex, u32>,
    world: &'w VoxelWorld,
}

impl<'w> MeshConstructor<'w> {
    fn add(&mut self, ao: FaceAo, side: Side, pos: BlockPos) {
        const FLIPPED_QUAD_CW: &'static [u32] = &[0, 1, 2, 3, 2, 1];
        const FLIPPED_QUAD_CCW: &'static [u32] = &[2, 1, 0, 1, 2, 3];
        const NORMAL_QUAD_CW: &'static [u32] = &[3, 2, 0, 0, 1, 3];
        const NORMAL_QUAD_CCW: &'static [u32] = &[0, 2, 3, 3, 1, 0];

        let top = side.facing_positive();

        let ao_pp = (ao.corner_ao(FaceAo::AO_POS_POS) as f32) / 3.0;
        let ao_pn = (ao.corner_ao(FaceAo::AO_POS_NEG) as f32) / 3.0;
        let ao_nn = (ao.corner_ao(FaceAo::AO_NEG_NEG) as f32) / 3.0;
        let ao_np = (ao.corner_ao(FaceAo::AO_NEG_POS) as f32) / 3.0;
        let flipped = ao_pp + ao_nn > ao_pn + ao_np;

        let clockwise = match side {
            Side::Top => false,
            Side::Front => true,
            Side::Right => false,
            Side::Bottom => true,
            Side::Back => false,
            Side::Left => true,
        };

        let quad = if flipped {
            if clockwise {
                FLIPPED_QUAD_CW
            } else {
                FLIPPED_QUAD_CCW
            }
        } else {
            if clockwise {
                NORMAL_QUAD_CW
            } else {
                NORMAL_QUAD_CCW
            }
        };

        // let quad = match (flipped, side) {
        //     (false, false, Axis::X) => NORMAL_QUAD_CW,
        //     (false, true, Axis::X) => NORMAL_QUAD_CCW,
        //     (true, false, Axis::X) => FLIPPED_QUAD_CW,
        //     (true, true, Axis::X) => FLIPPED_QUAD_CCW,

        //     (false, false, Axis::Y) => NORMAL_QUAD_CCW,
        //     (false, true, Axis::Y) => NORMAL_QUAD_CW,
        //     (true, false, Axis::Y) => FLIPPED_QUAD_CCW,
        //     (true, true, Axis::Y) => FLIPPED_QUAD_CW,

        //     (false, false, Axis::Z) => NORMAL_QUAD_CCW,
        //     (false, true, Axis::Z) => NORMAL_QUAD_CW,
        //     (true, false, Axis::Z) => FLIPPED_QUAD_CCW,
        //     (true, true, Axis::Z) => FLIPPED_QUAD_CW,
        // };

        let index = self.index;
        self.mesh.indices.extend(quad.iter().map(|i| i + index));
        self.index += 4;

        let normal = side.normal();

        let tile_offset = self
            .world
            .get_block_properties(pos)
            .unwrap()
            .get_texture_offset(side);

        let base = pos.base().0.cast::<f32>().unwrap();
        let mut push_vertex = |offset, uv, ao| {
            self.mesh.vertices.push(BlockVertex {
                pos: (base + offset),
                uv,
                ao,
                normal,
                tile_offset,
                face: 0,
            })
        };

        let h = if top { 1.0 } else { 0.0 };

        if side == Side::Left || side == Side::Right {
            push_vertex(Vector3::new(h, 1.0, 0.0), Vector2::new(0.0, 0.0), ao_pn);
            push_vertex(Vector3::new(h, 1.0, 1.0), Vector2::new(1.0, 0.0), ao_pp);
            push_vertex(Vector3::new(h, 0.0, 0.0), Vector2::new(0.0, 1.0), ao_nn);
            push_vertex(Vector3::new(h, 0.0, 1.0), Vector2::new(1.0, 1.0), ao_np);
        }

        if side == Side::Top || side == Side::Bottom {
            push_vertex(Vector3::new(0.0, h, 1.0), Vector2::new(0.0, 0.0), ao_pn);
            push_vertex(Vector3::new(1.0, h, 1.0), Vector2::new(1.0, 0.0), ao_pp);
            push_vertex(Vector3::new(0.0, h, 0.0), Vector2::new(0.0, 1.0), ao_nn);
            push_vertex(Vector3::new(1.0, h, 0.0), Vector2::new(1.0, 1.0), ao_np);
        }

        if side == Side::Front || side == Side::Back {
            push_vertex(Vector3::new(0.0, 1.0, h), Vector2::new(0.0, 0.0), ao_np);
            push_vertex(Vector3::new(1.0, 1.0, h), Vector2::new(1.0, 0.0), ao_pp);
            push_vertex(Vector3::new(0.0, 0.0, h), Vector2::new(0.0, 1.0), ao_nn);
            push_vertex(Vector3::new(1.0, 0.0, h), Vector2::new(1.0, 1.0), ao_pn);
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
