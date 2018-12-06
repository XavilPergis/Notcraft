use crate::engine::{
    components as comp,
    render::{
        debug::{DebugAccumulator, Shape},
        mesh::Mesh,
        GraphicsData, LiquidMesh, LiquidVertex, MeshPair, TerrainMesh, TerrainVertex,
    },
    world::{
        block::{self, BlockId, BlockRegistry},
        chunk::{Chunk, ChunkType, PaddedChunk, SIZE},
        BlockPos, ChunkPos, VoxelWorld,
    },
    Side,
};
use cgmath::{Point2, Point3, Vector2, Vector3, Vector4};
use rand::prelude::*;
use specs::prelude::*;
use std::{cell::RefCell, rc::Rc};

pub struct ChunkMesher {
    graphics: Rc<RefCell<GraphicsData>>,
}

impl ChunkMesher {
    pub fn new(graphics: &Rc<RefCell<GraphicsData>>) -> Self {
        ChunkMesher {
            graphics: graphics.clone(),
        }
    }
}

impl<'a> System<'a> for ChunkMesher {
    type SystemData = (
        WriteExpect<'a, VoxelWorld>,
        ReadExpect<'a, DebugAccumulator>,
    );

    fn run(&mut self, (mut world, debug): Self::SystemData) {
        let mut section = debug.section("mesher");
        loop {
            if let Some(pos) = world.get_dirty_chunk() {
                log::trace!("{:?} ", pos);
                match world.chunk(pos) {
                    Some(ChunkType::Homogeneous(id)) => {
                        // lol this fucking if statment
                        if !world.get_registry().opaque(*id) {
                            // Don't do anything lole
                        }

                        // FIXME: we can get some pretty weird inconsistencies here if we don't mesh
                        // homogeneous solid chunks. could we just generate one big cube or smth?
                        world.clean_chunk(pos);

                        section.draw(Shape::Chunk(5.0, pos, Vector4::new(0.0, 1.0, 0.0, 1.0)));
                    }

                    Some(ChunkType::Array(_)) => {
                        let mut graphics = self.graphics.borrow_mut();
                        graphics.terrain_meshes.entry(pos).or_insert_with(|| {
                            let (terrain, liquid) = mesh_chunk(pos, &world);

                            (
                                MeshPair {
                                    dirty: false,
                                    cpu: terrain,
                                    gpu: None,
                                },
                                MeshPair {
                                    dirty: false,
                                    cpu: liquid,
                                    gpu: None,
                                },
                            )
                        });

                        world.clean_chunk(pos);
                        section.draw(Shape::Chunk(5.0, pos, Vector4::new(1.0, 0.0, 1.0, 1.0)));
                        break;
                    }

                    // wat
                    _ => (),
                }
            } else {
                break;
            }
        }
    }
}

fn mesh_chunk(pos: ChunkPos, world: &VoxelWorld) -> (TerrainMesh, LiquidMesh) {
    let mut mesher = Mesher::new(pos, world);
    mesher.mesh();
    (
        mesher.mesh_constructor.terrain_mesh,
        mesher.mesh_constructor.liquid_mesh,
    )
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
struct VoxelQuad {
    ao: FaceAo,
    id: BlockId,
    width: usize,
    height: usize,
}

impl From<VoxelFace> for VoxelQuad {
    fn from(face: VoxelFace) -> Self {
        VoxelQuad {
            ao: face.ao,
            id: face.id,
            width: 1,
            height: 1,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
struct VoxelFace {
    ao: FaceAo,
    id: BlockId,
    visited: bool,
}

pub struct Mesher<'w> {
    registry: &'w BlockRegistry,
    center: PaddedChunk,
    mesh_constructor: MeshConstructor<'w>,
    slice: Vec<VoxelFace>,
}

// index into the flat voxel face slice using a 2D coordinate
const fn idx(u: usize, v: usize) -> usize {
    SIZE * u + v
}

impl<'w> Mesher<'w> {
    pub fn new(pos: ChunkPos, world: &'w VoxelWorld) -> Self {
        Mesher {
            registry: world.get_registry(),
            center: crate::engine::world::chunk::make_padded(world, pos).unwrap(),
            slice: vec![VoxelFace::default(); crate::engine::world::chunk::AREA],
            mesh_constructor: MeshConstructor {
                liquid_mesh: Default::default(),
                terrain_mesh: Default::default(),
                registry: world.get_registry(),
            },
        }
    }

    fn face_ao(&self, pos: Point3<usize>, side: Side) -> FaceAo {
        if self.registry.liquid(self.center[pos]) {
            return FaceAo::default();
        }

        let pos = pos.cast().unwrap();
        let is_opaque = |pos| self.registry.opaque(self.center[pos]);

        let neg_neg = is_opaque(pos + side.uvl_to_xyz(-1, -1, 1));
        let neg_cen = is_opaque(pos + side.uvl_to_xyz(-1, 0, 1));
        let neg_pos = is_opaque(pos + side.uvl_to_xyz(-1, 1, 1));
        let pos_neg = is_opaque(pos + side.uvl_to_xyz(1, -1, 1));
        let pos_cen = is_opaque(pos + side.uvl_to_xyz(1, 0, 1));
        let pos_pos = is_opaque(pos + side.uvl_to_xyz(1, 1, 1));
        let cen_neg = is_opaque(pos + side.uvl_to_xyz(0, -1, 1));
        let cen_pos = is_opaque(pos + side.uvl_to_xyz(0, 1, 1));

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

    fn is_not_occluded(&self, pos: Point3<usize>, offset: Vector3<isize>) -> bool {
        let offset = pos.cast::<isize>().unwrap() + offset;

        let cur_solid = self.registry.opaque(self.center[pos]);
        let other_solid = self.registry.opaque(self.center[offset]);

        let cur_liquid = self.registry.liquid(self.center[pos]);
        let other_liquid = self.registry.liquid(self.center[offset]);

        if self.registry.liquid(self.center[pos]) {
            // if the current block is liquid, we need a face when the other block is
            // non-solid or non-liquid
            cur_liquid && !other_solid && !other_liquid
        } else {
            // if the current block is not liquid...
            // if the current block is not opaque, then it would never need any faces
            // if the current block is solid, and the other is either non-opaque or a
            // liquid, then we need a face
            cur_solid && (!other_solid || self.registry.liquid(self.center[offset]))
        }
    }

    /*
    for each x:
        for each y:
            if the face has been expanded onto already, skip this.

            # note that width and height start off as 1, and mark the "next" block
            while (x + width) is still in chunk bounds and the face at (x + width, y) is the same as the current face:
                increment width

            while (y + height) is still in chunk bounds:
                # every block under the current quad
                if every block in x=[x, x + width] y=y+1 is the same as the current:
                    increment height
                else:
                    stop the loop

            mark every block under expanded quad as visited
    */
    // TODO: explain how greedy meshing works

    pub fn submit_quads(
        &mut self,
        side: Side,
        point_constructor: impl Fn(usize, usize) -> Point3<usize>,
    ) {
        for u in 0..SIZE {
            for v in 0..SIZE {
                let cur = self.slice[idx(u, v)];

                let is_liquid = self.registry.liquid(cur.id);

                // if the face has been expanded onto already, skip it.
                if cur.visited || !(self.registry.opaque(cur.id) || is_liquid) {
                    continue;
                }
                let mut quad = VoxelQuad::from(cur);

                // while the next position is in chunk bounds and is the same block face as the
                // current
                while u + quad.width < SIZE && self.slice[idx(u + quad.width, v)] == cur {
                    quad.width += 1;
                }

                while v + quad.height < SIZE {
                    if (u..u + quad.width)
                        .map(|u| self.slice[idx(u, v + quad.height)])
                        .all(|face| face == cur)
                    {
                        quad.height += 1;
                    } else {
                        break;
                    }
                }

                for w in 0..quad.width {
                    for h in 0..quad.height {
                        self.slice[idx(u + w, v + h)].visited = true;
                    }
                }

                if is_liquid {
                    self.mesh_constructor
                        .add_liquid(quad, side, point_constructor(u, v));
                } else {
                    self.mesh_constructor
                        .add_terrain(quad, side, point_constructor(u, v));
                }
            }
        }
    }

    fn mesh_slice(
        &mut self,
        side: Side,
        make_coordinate: impl Fn(usize, usize, usize) -> Point3<usize>,
    ) {
        for layer in 0..SIZE {
            for u in 0..SIZE {
                for v in 0..SIZE {
                    let padded = make_coordinate(layer, u, v) + Vector3::new(1, 1, 1);
                    self.slice[idx(u, v)] = if self.is_not_occluded(padded, side.normal()) {
                        VoxelFace {
                            id: self.center[padded],
                            ao: self.face_ao(padded, side),
                            visited: false,
                        }
                    } else {
                        VoxelFace {
                            id: BlockId::default(),
                            ao: FaceAo::default(),
                            visited: true,
                        }
                    };
                }
            }

            self.submit_quads(side, |u, v| make_coordinate(layer, u, v));
        }
    }

    pub fn mesh(&mut self) {
        self.mesh_slice(Side::Right, |layer, u, v| Point3::new(layer, u, v));
        self.mesh_slice(Side::Left, |layer, u, v| Point3::new(layer, u, v));

        self.mesh_slice(Side::Top, |layer, u, v| Point3::new(u, layer, v));
        self.mesh_slice(Side::Bottom, |layer, u, v| Point3::new(u, layer, v));

        self.mesh_slice(Side::Front, |layer, u, v| Point3::new(u, v, layer));
        self.mesh_slice(Side::Back, |layer, u, v| Point3::new(u, v, layer));
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

const FLIPPED_QUAD_CW: &'static [u32] = &[0, 1, 2, 3, 2, 1];
const FLIPPED_QUAD_CCW: &'static [u32] = &[2, 1, 0, 1, 2, 3];
const NORMAL_QUAD_CW: &'static [u32] = &[3, 2, 0, 0, 1, 3];
const NORMAL_QUAD_CCW: &'static [u32] = &[0, 2, 3, 3, 1, 0];

const UV_VARIANT_1: &[Vector2<f32>] = &[
    Vector2 { x: 1.0, y: 1.0 },
    Vector2 { x: 0.0, y: 1.0 },
    Vector2 { x: 1.0, y: 0.0 },
    Vector2 { x: 0.0, y: 0.0 },
];

const UV_VARIANT_2: &[Vector2<f32>] = &[
    Vector2 { x: 1.0, y: 0.0 },
    Vector2 { x: 1.0, y: 1.0 },
    Vector2 { x: 0.0, y: 0.0 },
    Vector2 { x: 0.0, y: 1.0 },
];

const UV_VARIANT_3: &[Vector2<f32>] = &[
    Vector2 { x: 1.0, y: 1.0 },
    Vector2 { x: 0.0, y: 1.0 },
    Vector2 { x: 1.0, y: 0.0 },
    Vector2 { x: 0.0, y: 0.0 },
];

const UV_VARIANT_4: &[Vector2<f32>] = &[
    Vector2 { x: 0.0, y: 1.0 },
    Vector2 { x: 0.0, y: 0.0 },
    Vector2 { x: 1.0, y: 1.0 },
    Vector2 { x: 1.0, y: 0.0 },
];

fn select_uv_variant(random: bool) -> &'static [Vector2<f32>] {
    if random {
        (&[UV_VARIANT_1, UV_VARIANT_2, UV_VARIANT_3, UV_VARIANT_4])
            .choose(&mut SmallRng::from_entropy())
            .unwrap()
    } else {
        UV_VARIANT_1
    }
}

#[derive(Debug)]
struct MeshConstructor<'w> {
    liquid_mesh: LiquidMesh,
    terrain_mesh: TerrainMesh,
    registry: &'w BlockRegistry,
}

impl<'w> MeshConstructor<'w> {
    fn add_liquid(&mut self, quad: VoxelQuad, side: Side, pos: Point3<usize>) {
        let pos: Point3<f32> = pos.cast().unwrap();

        let clockwise = match side {
            Side::Top => false,
            Side::Bottom => true,
            Side::Front => true,
            Side::Back => false,
            Side::Right => false,
            Side::Left => true,
        };

        let indices = if clockwise {
            NORMAL_QUAD_CW
        } else {
            NORMAL_QUAD_CCW
        };

        let normal = side.normal();

        let face = self.registry.block_texture(quad.id, side).unwrap();
        let tex_id = *face.texture.select() as i32;

        let h = if side.facing_positive() { 1.0 } else { 0.0 };
        let qw = quad.width as f32;
        let qh = quad.height as f32;

        let vert = |offset: Vector3<f32>, uv: Vector2<f32>| LiquidVertex {
            pos: (pos + offset).into(),
            uv: uv.into(),
            normal: normal.into(),
            tex_id,
        };

        let uvs = UV_VARIANT_1;

        if !self.liquid_mesh.add(
            match side {
                Side::Top | Side::Bottom => [
                    vert(
                        Vector3::new(0.0, h, qh),
                        Vector2::new(uvs[0].x * qw, uvs[0].y * qh),
                    ),
                    vert(
                        Vector3::new(qw, h, qh),
                        Vector2::new(uvs[1].x * qw, uvs[1].y * qh),
                    ),
                    vert(
                        Vector3::new(0.0, h, 0.0),
                        Vector2::new(uvs[2].x * qw, uvs[2].y * qh),
                    ),
                    vert(
                        Vector3::new(qw, h, 0.0),
                        Vector2::new(uvs[3].x * qw, uvs[3].y * qh),
                    ),
                ],

                Side::Left | Side::Right => [
                    vert(
                        Vector3::new(h, qw, 0.0),
                        Vector2::new(uvs[0].x * qh, uvs[0].y * qw),
                    ),
                    vert(
                        Vector3::new(h, qw, qh),
                        Vector2::new(uvs[1].x * qh, uvs[1].y * qw),
                    ),
                    vert(
                        Vector3::new(h, 0.0, 0.0),
                        Vector2::new(uvs[2].x * qh, uvs[2].y * qw),
                    ),
                    vert(
                        Vector3::new(h, 0.0, qh),
                        Vector2::new(uvs[3].x * qh, uvs[3].y * qw),
                    ),
                ],

                Side::Front | Side::Back => [
                    vert(
                        Vector3::new(0.0, qh, h),
                        Vector2::new(uvs[0].x * qw, uvs[0].y * qh),
                    ),
                    vert(
                        Vector3::new(qw, qh, h),
                        Vector2::new(uvs[1].x * qw, uvs[1].y * qh),
                    ),
                    vert(
                        Vector3::new(0.0, 0.0, h),
                        Vector2::new(uvs[2].x * qw, uvs[2].y * qh),
                    ),
                    vert(
                        Vector3::new(qw, 0.0, h),
                        Vector2::new(uvs[3].x * qw, uvs[3].y * qh),
                    ),
                ],
            }
            .iter()
            .cloned(),
            indices.iter().cloned(),
        ) {
            panic!("Mesh could not be created");
        }
    }

    fn add_terrain(&mut self, quad: VoxelQuad, side: Side, pos: Point3<usize>) {
        let pos: Point3<f32> = pos.cast().unwrap();

        let ao_pp = (quad.ao.corner_ao(FaceAo::AO_POS_POS) as f32) / 3.0;
        let ao_pn = (quad.ao.corner_ao(FaceAo::AO_POS_NEG) as f32) / 3.0;
        let ao_nn = (quad.ao.corner_ao(FaceAo::AO_NEG_NEG) as f32) / 3.0;
        let ao_np = (quad.ao.corner_ao(FaceAo::AO_NEG_POS) as f32) / 3.0;
        let flipped = ao_pp + ao_nn > ao_pn + ao_np;

        let clockwise = match side {
            Side::Top => false,
            Side::Bottom => true,
            Side::Front => true,
            Side::Back => false,
            Side::Right => false,
            Side::Left => true,
        };

        let indices = if flipped {
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

        let normal = side.normal();

        let face = self.registry.block_texture(quad.id, side).unwrap();
        let tex_id = *face.texture.select() as i32;

        let h = if side.facing_positive() { 1.0 } else { 0.0 };
        let qw = quad.width as f32;
        let qh = quad.height as f32;

        let mut vert = |offset: Vector3<f32>, uv: Vector2<f32>, ao| TerrainVertex {
            pos: (pos + offset).into(),
            uv: uv.into(),
            normal: normal.into(),
            ao,
            tex_id,
        };

        let uvs = UV_VARIANT_1;

        // REALLY don't know why I have to reverse the quad width and height along the X
        // axis... I bet someone more qualified could tell me :>
        if !self.terrain_mesh.add(
            match side {
                Side::Left | Side::Right => [
                    vert(
                        Vector3::new(h, qw, 0.0),
                        Vector2::new(uvs[0].x * qh, uvs[0].y * qw),
                        ao_pn,
                    ),
                    vert(
                        Vector3::new(h, qw, qh),
                        Vector2::new(uvs[1].x * qh, uvs[1].y * qw),
                        ao_pp,
                    ),
                    vert(
                        Vector3::new(h, 0.0, 0.0),
                        Vector2::new(uvs[2].x * qh, uvs[2].y * qw),
                        ao_nn,
                    ),
                    vert(
                        Vector3::new(h, 0.0, qh),
                        Vector2::new(uvs[3].x * qh, uvs[3].y * qw),
                        ao_np,
                    ),
                ],

                Side::Top | Side::Bottom => [
                    vert(
                        Vector3::new(0.0, h, qh),
                        Vector2::new(uvs[0].x * qw, uvs[0].y * qh),
                        ao_pn,
                    ),
                    vert(
                        Vector3::new(qw, h, qh),
                        Vector2::new(uvs[1].x * qw, uvs[1].y * qh),
                        ao_pp,
                    ),
                    vert(
                        Vector3::new(0.0, h, 0.0),
                        Vector2::new(uvs[2].x * qw, uvs[2].y * qh),
                        ao_nn,
                    ),
                    vert(
                        Vector3::new(qw, h, 0.0),
                        Vector2::new(uvs[3].x * qw, uvs[3].y * qh),
                        ao_np,
                    ),
                ],

                Side::Front | Side::Back => [
                    vert(
                        Vector3::new(0.0, qh, h),
                        Vector2::new(uvs[0].x * qw, uvs[0].y * qh),
                        ao_np,
                    ),
                    vert(
                        Vector3::new(qw, qh, h),
                        Vector2::new(uvs[1].x * qw, uvs[1].y * qh),
                        ao_pp,
                    ),
                    vert(
                        Vector3::new(0.0, 0.0, h),
                        Vector2::new(uvs[2].x * qw, uvs[2].y * qh),
                        ao_nn,
                    ),
                    vert(
                        Vector3::new(qw, 0.0, h),
                        Vector2::new(uvs[3].x * qw, uvs[3].y * qh),
                        ao_pn,
                    ),
                ],
            }
            .iter()
            .cloned(),
            indices.iter().cloned(),
        ) {
            panic!("Mesh could not be created");
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
