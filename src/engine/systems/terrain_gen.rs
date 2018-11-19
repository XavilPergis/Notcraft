use engine::systems::debug_render::DebugAccumulator;
use engine::systems::debug_render::Shape;
use noise::{Fbm, MultiFractal, NoiseFn, SuperSimplex};
use specs::world::EntitiesRes;
use std::collections::HashSet;
use std::sync::mpsc;

use engine::prelude::*;

#[derive(Clone, Debug)]
pub struct NoiseGenerator {
    noise: Fbm,
    biome_noise: SuperSimplex,
}

impl NoiseGenerator {
    pub fn new_default() -> Self {
        let noise = Fbm::default().set_frequency(0.001).set_persistence(0.8);
        let biome_noise = SuperSimplex::new();
        NoiseGenerator { noise, biome_noise }
    }

    fn block_at(&self, pos: WorldPos) -> BlockId {
        let WorldPos(Point3 { x, y, z }) = pos;

        let noise = 100.0 * self.noise.get([x, z]);
        let block_noise = self.biome_noise.get([x / 32.0, y / 32.0, z / 32.0]);

        if noise > y {
            if block_noise > 0.33 {
                block::GRASS
            } else if block_noise > -0.33 {
                block::DIRT
            } else {
                block::STONE
            }
        } else {
            block::AIR
        }
    }
}

impl job::Worker for NoiseGenerator {
    type Input = ChunkPos;
    type Output = Chunk<BlockId>;

    fn compute(&mut self, pos: &Self::Input) -> Self::Output {
        Chunk::new(::nd::Array3::from_shape_fn(
            (chunk::SIZE, chunk::SIZE, chunk::SIZE),
            |(x, y, z)| self.block_at(pos.base().offset((x as i32, y as i32, z as i32)).center()),
        ))
    }
}

pub struct ChunkUnloader {
    keep_loaded: HashSet<ChunkPos>,
}

impl Default for ChunkUnloader {
    fn default() -> Self {
        ChunkUnloader {
            keep_loaded: HashSet::new(),
        }
    }
}

impl<'a> System<'a> for ChunkUnloader {
    type SystemData = (
        WriteExpect<'a, VoxelWorld>,
        WriteStorage<'a, comp::MarkedForDeletion>,
        ReadStorage<'a, comp::ChunkId>,
        ReadStorage<'a, comp::Player>,
        ReadStorage<'a, comp::Transform>,
        Read<'a, res::ViewDistance>,
        Entities<'a>,
        ReadExpect<'a, DebugAccumulator>,
    );

    fn run(
        &mut self,
        (mut world, mut marked, chunks, players, transforms, distance, entities, debug): Self::SystemData,
    ) {
        let distance = distance.0;

        for (entity, chunk, _) in (&entities, &chunks, &marked).join() {
            world.unload_chunk(chunk.0);
            let _ = entities.delete(entity);
        }

        self.keep_loaded.clear();
        for &comp::ChunkId(chunk) in chunks.join() {
            for (transform, _) in (&transforms, &players).join() {
                let center: ChunkPos = WorldPos(transform.position).into();
                if ::util::in_range(chunk.0, center.0, distance) {
                    self.keep_loaded.insert(chunk);
                }
            }
        }

        for (entity, &comp::ChunkId(chunk)) in (&entities, &chunks).join() {
            if !self.keep_loaded.contains(&chunk) {
                let _ = marked.insert(entity, comp::MarkedForDeletion);
            }
        }
    }
}

pub struct ChunkLoader {
    loaded: HashSet<ChunkPos>,
}

impl<'a> System<'a> for ChunkLoader {
    type SystemData = (
        ReadExpect<'a, VoxelWorld>,
        ReadStorage<'a, comp::Player>,
        ReadStorage<'a, comp::Transform>,
        Entities<'a>,
        Read<'a, res::ViewDistance>,
        Read<'a, LazyUpdate>,
        ReadExpect<'a, DebugAccumulator>,
    );

    fn run(
        &mut self,
        (world, players, transforms, entities, distance, lazy, debug): Self::SystemData,
    ) {
        let distance = distance.0;
        for (_, transform) in (&players, &transforms).join() {
            let base: ChunkPos = WorldPos(transform.position).into();
            for xo in -distance.x..=distance.x {
                for yo in -distance.y..=distance.y {
                    for zo in -distance.z..=distance.z {
                        let pos = base.offset((xo, yo, zo));
                        if self.loaded.insert(pos) {
                            lazy.create_entity(&entities)
                                .with(comp::ChunkId(pos))
                                .with(comp::MarkedForLoading)
                                .with(comp::Transform::default())
                                .build();
                        }
                    }
                }
            }
        }
    }
}

pub struct TerrainGenerator {
    service: job::Service<NoiseGenerator>,
    queue: HashSet<ChunkPos>,
}

impl TerrainGenerator {
    pub fn new() -> Self {
        let service = job::Service::new("Chunk Generator", 4, NoiseGenerator::new_default());

        TerrainGenerator {
            service,
            queue: HashSet::default(),
        }
    }
}

impl<'a> System<'a> for TerrainGenerator {
    type SystemData = (
        WriteExpect<'a, VoxelWorld>,
        ReadStorage<'a, comp::Player>,
        ReadStorage<'a, comp::Transform>,
        Read<'a, res::ViewDistance>,
        Read<'a, LazyUpdate>,
        Read<'a, EntitiesRes>,
        ReadExpect<'a, DebugAccumulator>,
    );

    fn run(
        &mut self,
        (mut voxel_world, players, transforms, view_distance, lazy, entity_res, debug): Self::SystemData,
    ) {
        self.service.update();

        let dist = view_distance.0;
        for (_, transform) in (&players, &transforms).join() {
            let base_pos: ChunkPos = WorldPos(transform.position).into();
            for xo in -dist.x..=dist.x {
                for yo in -dist.y..=dist.y {
                    for zo in -dist.z..=dist.z {
                        let pos = base_pos.offset((xo, yo, zo));
                        if !voxel_world.chunk_exists(pos) && !self.queue.contains(&pos) {
                            self.queue.insert(pos);
                            self.service.dispatch(pos);
                        }
                    }
                }
            }
        }

        let mut section = debug.section("terrain generation");
        for item in self.queue.iter() {
            section.draw(Shape::Chunk(2.0, *item, Vector4::new(1.0, 0.0, 0.0, 1.0)));
        }

        for (pos, chunk) in self.service.poll() {
            section.draw(Shape::Chunk(2.0, pos, Vector4::new(0.0, 1.0, 0.0, 1.0)));
            voxel_world.set_chunk(pos, chunk);
            self.queue.remove(&pos);
            lazy.create_entity(&entity_res)
                .with(comp::ChunkId(pos))
                .with(comp::DirtyMesh)
                .with(comp::Transform::default())
                .build();
        }
    }
}
