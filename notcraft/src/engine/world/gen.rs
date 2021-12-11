use crate::engine::prelude::*;
use na::{vector, OPoint};
use simdnoise::{NoiseBuilder, RidgeSettings};
use std::collections::HashSet;

use super::chunk::ChunkType;

struct NoiseOptions {}

// fn

fn generate_noise(opts: NoiseOptions, out: &mut [f32]) {}

#[derive(Clone, Debug)]
pub struct NoiseGenerator {
    stone_id: BlockId,
    dirt_id: BlockId,
    grass_id: BlockId,
}

impl NoiseGenerator {
    pub fn new_default(registry: &block::BlockRegistry) -> Self {
        Self {
            stone_id: registry.get_id("stone"),
            dirt_id: registry.get_id("dirt"),
            grass_id: registry.get_id("grass"),
        }
    }

    fn block_from_surface_dist(&self, distance: f32) -> BlockId {
        if distance < -4.0 {
            self.stone_id
        } else if distance < -1.0 {
            self.dirt_id
        } else if distance < 0.0 {
            self.grass_id
        } else {
            block::AIR
        }
    }

    pub fn make_chunk(&self, pos: ChunkPos) -> ChunkType {
        let base = pos.base().0;
        let noise = NoiseBuilder::gradient_2d_offset(
            base.x as f32,
            chunk::SIZE,
            base.z as f32,
            chunk::SIZE,
        )
        .with_freq(0.01)
        // .with_octaves(5)
        .with_seed(1337)
        // .with_lacunarity(0.5)
        .generate_scaled(-10.0, 10.0);

        // if base.y as f32 - noise_min > 0.0 {
        //     return ChunkType::Homogeneous(block::AIR);
        // }

        let mut chunk_data = Vec::with_capacity(chunk::VOLUME);
        for x in 0..chunk::SIZE {
            for z in 0..chunk::SIZE {
                let surface_height = noise[x as usize * chunk::SIZE + z];

                for y in 0..chunk::SIZE {
                    let id =
                        self.block_from_surface_dist(base.y as f32 + y as f32 - surface_height);
                    chunk_data.push(id);
                }
            }
        }

        assert!(!chunk_data.is_empty());
        ChunkType::Array(Chunk::new(chunk_data))
    }
}

// impl job::Worker for NoiseGenerator {
//     type Input = ChunkPos;
//     type Output = Chunk;

//     fn compute(&mut self, pos: &Self::Input) -> Self::Output {}
// }

// pub struct ChunkUnloader {
//     keep_loaded: HashSet<ChunkPos>,
// }

// impl Default for ChunkUnloader {
//     fn default() -> Self {
//         ChunkUnloader {
//             keep_loaded: HashSet::new(),
//         }
//     }
// }

// impl<'a> System<'a> for ChunkUnloader {
//     type SystemData = (
//         WriteExpect<'a, VoxelWorld>,
//         WriteStorage<'a, comp::MarkedForDeletion>,
//         ReadStorage<'a, comp::ChunkId>,
//         ReadStorage<'a, comp::Player>,
//         ReadStorage<'a, comp::Transform>,
//         Read<'a, res::ViewDistance>,
//         Entities<'a>,
//         // ReadExpect<'a, DebugAccumulator>,
//     );

//     fn run(
//         &mut self,
//         (mut world, mut marked, chunks, players, transforms, distance,
// entities): Self::SystemData,     ) {
//         let distance = distance.0;

//         for (entity, chunk, _) in (&entities, &chunks, &marked).join() {
//             world.unload_chunk(chunk.0);
//             let _ = entities.delete(entity);
//         }

//         self.keep_loaded.clear();
//         for &comp::ChunkId(chunk) in chunks.join() {
//             for (transform, _) in (&transforms, &players).join() {
//                 let center: ChunkPos = WorldPos(transform.position).into();
//                 if crate::util::in_range(chunk.0, center.0, distance) {
//                     self.keep_loaded.insert(chunk);
//                 }
//             }
//         }

//         for (entity, &comp::ChunkId(chunk)) in (&entities, &chunks).join() {
//             if !self.keep_loaded.contains(&chunk) {
//                 let _ = marked.insert(entity, comp::MarkedForDeletion);
//             }
//         }
//     }
// }

// #[derive(Debug)]
// pub struct TerrainGenerator {
//     service: job::Service<NoiseGenerator>,
//     queue: HashSet<ChunkPos>,
// }

// impl TerrainGenerator {
//     pub fn new() -> Self {
//         let service = job::Service::new("Chunk Generator", 4,
// NoiseGenerator::new_default());

//         TerrainGenerator {
//             service,
//             queue: HashSet::default(),
//         }
//     }

//     pub fn enqueue_radius(&mut self, world: &VoxelWorld, center: ChunkPos,
// radius: usize) {         let radius = radius as i32;
//         for xo in -radius..=radius {
//             for yo in -radius..=radius {
//                 for zo in -radius..=radius {
//                     let pos = center.offset((xo, yo, zo));
//                     if !world.chunk_exists(pos) && !self.queue.contains(&pos)
// {                         self.queue.insert(pos);
//                         self.service.request(pos);
//                     }
//                 }
//             }
//         }
//     }

//     pub fn drain_finished_chunks(&mut self) -> impl Iterator<Item =
// (ChunkPos, Chunk)> + '_ {         self.service.gather()
//     }
// }

// impl<'a> System<'a> for TerrainGenerator {
//     type SystemData = (
//         WriteExpect<'a, VoxelWorld>,
//         ReadStorage<'a, comp::Player>,
//         ReadStorage<'a, comp::Transform>,
//         Read<'a, res::ViewDistance>,
//         Read<'a, LazyUpdate>,
//         Read<'a, EntitiesRes>,
//         // ReadExpect<'a, DebugAccumulator>,
//     );

//     fn run(
//         &mut self,
//         (mut voxel_world, players, transforms, view_distance, lazy,
// entity_res): Self::SystemData,     ) {
//         let dist = view_distance.0;
//         for (_, transform) in (&players, &transforms).join() {
//             self.enqueue_radius(
//                 &voxel_world,
//                 WorldPos(transform.position).into(),
//                 dist.x as usize,
//             );
//         }

//         // let mut section = debug.section("terrain generation");
//         // for item in self.queue.iter() {
//         //     section.chunk(*item, 2.0, Vector4::new(1.0, 0.0, 0.0, 1.0));
//         // }

//         for (pos, chunk) in self.service.gather() {
//             // section.chunk(pos, 2.0, Vector4::new(0.0, 1.0, 0.0, 1.0));
//             voxel_world.set_chunk(pos, chunk);
//             self.queue.remove(&pos);
//             lazy.create_entity(&entity_res)
//                 .with(comp::ChunkId(pos))
//
// .with(comp::Transform::default().with_position(pos.base().base().0))
//                 .build();
//         }
//     }
// }
