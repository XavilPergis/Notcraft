use cgmath::{Vector2, Vector3, Vector4};
use engine::{
    components as comp,
    render::{
        debug::{DebugAccumulator, Shape},
        terrain::{BlockVertex, ChunkMesh},
    },
    world::{chunk::SIZE, BlockPos, ChunkPos, VoxelWorld},
    Side,
};
use specs::prelude::*;

pub struct ChunkMesher;

impl ChunkMesher {
    pub fn new() -> Self {
        ChunkMesher
    }
}

impl<'a> System<'a> for ChunkMesher {
    type SystemData = (
        ReadStorage<'a, comp::ChunkId>,
        WriteExpect<'a, VoxelWorld>,
        WriteStorage<'a, ChunkMesh>,
        ReadExpect<'a, DebugAccumulator>,
        Entities<'a>,
    );

    fn run(&mut self, (chunk_ids, mut world, mut meshes, debug, entities): Self::SystemData) {
        let mut section = debug.section("mesher");
        for _ in 0..4 {
            if let Some(pos) = world.get_dirty_chunk() {
                section.draw(Shape::Chunk(2.0, pos, Vector4::new(0.5, 0.5, 1.0, 1.0)));
                trace!("Chunk {:?} is ready for meshing", pos);
                let mut mesher = CullMesher::new(pos, &world);
                mesher.mesh();
                if let Some((_, entity)) = (&chunk_ids, &entities)
                    .join()
                    .find(|(&comp::ChunkId(cpos), _)| cpos == pos)
                {
                    let _ = meshes.insert(entity, mesher.mesh_constructor.mesh);
                    world.clean_chunk(pos);
                }
            } else {
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
        let is_opaque = |pos| self.world.registry(pos).unwrap().opaque();

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
        let cur = self.world.registry(pos).unwrap();
        let other = self.world.registry(pos.offset(offset)).unwrap();

        cur.opaque() && !other.opaque()
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

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
struct FaceAo(u8);

impl FaceAo {
    const AO_NEG_NEG: u8 = 2;
    const AO_NEG_POS: u8 = 0;
    const AO_POS_NEG: u8 = 4;
    const AO_POS_POS: u8 = 6;

    fn corner_ao(&self, bits: u8) -> u8 {
        (self.0 & (3 << bits)) >> bits
    }
}

#[derive(Debug)]
struct MeshConstructor<'w> {
    index: u32,
    mesh: ChunkMesh,
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

        let index = self.index;
        self.mesh.indices.extend(quad.iter().map(|i| i + index));
        self.index += 4;

        let normal = side.normal();

        let tex_id = self
            .world
            .registry(pos)
            .unwrap()
            .block_texture(side)
            .unwrap() as i32;

        let base = pos.base().0.cast::<f32>().unwrap();
        let mut push_vertex = |offset, uv, ao| {
            self.mesh.vertices.push(BlockVertex {
                pos: (base + offset),
                uv,
                ao,
                normal,
                tex_id,
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
