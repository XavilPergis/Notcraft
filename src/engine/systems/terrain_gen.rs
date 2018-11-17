use engine::systems::debug_render::DebugAccumulator;
use engine::systems::debug_render::Shape;
use noise::{Fbm, MultiFractal, NoiseFn, SuperSimplex};
use specs::world::EntitiesRes;
use std::collections::HashSet;
use std::sync::mpsc;

use engine::prelude::*;

pub struct NoiseGenerator {
    noise: Fbm,
    biome_noise: SuperSimplex,
}

impl NoiseGenerator {
    pub fn new_default() -> Self {
        let noise = Fbm::default().set_frequency(0.125);
        // noise = noise.set_persistence(0.9);
        let biome_noise = SuperSimplex::new();
        NoiseGenerator { noise, biome_noise }
    }

    fn block_at(&self, pos: Point3<f64>) -> BlockId {
        // let biome_noise = smoothstep((self.biome_noise.get([pos.x / 512.0, pos.z / 512.0]) + 1.0) / 2.0, 0.7, 0.5);
        // // noise::Worley
        // let noise1 = (256.0 * self.noise.get([pos.x / 6.0, pos.z / 6.0]) + 1.0) / 2.0;
        // let noise2 = (64.0 * self.noise.get([pos.x / 8.0, pos.z / 8.0]) + 1.0) / 2.0;
        // let min = ::util::min(noise1, noise2);
        // let max = ::util::max(noise1, noise2);

        // let noise = (min + biome_noise * (max - min)) - pos.y;
        let noise = self.noise.get([pos.x, pos.y, pos.z]);
        let block_noise = self
            .biome_noise
            .get([pos.x / 2.0, pos.y / 2.0, pos.z / 2.0]);

        let block = if block_noise > 0.33 {
            block::GRASS
        } else if block_noise > -0.33 {
            block::DIRT
        } else {
            block::STONE
        };

        if noise > 0.0 {
            block
        }
        // else if noise > 1.0 { block::DIRT }
        // else if noise > 0.0 { block::GRASS }
        else {
            block::AIR
        }
    }

    fn pos_at_block(ChunkPos(pos): ChunkPos, offset: Vector3<usize>) -> Point3<f64> {
        const SIZE: i32 = chunk::SIZE as i32;
        let x = ((SIZE * pos.x) as f64 + offset.x as f64) / SIZE as f64;
        let y = ((SIZE * pos.y) as f64 + offset.y as f64) / SIZE as f64;
        let z = ((SIZE * pos.z) as f64 + offset.z as f64) / SIZE as f64;
        Point3::new(x, y, z)
    }
}

impl ChunkGenerator<BlockId> for NoiseGenerator {
    fn generate_chunk(&self, pos: ChunkPos) -> Chunk<BlockId> {
        Chunk::new(::nd::Array3::from_shape_fn(
            (chunk::SIZE, chunk::SIZE, chunk::SIZE),
            |coord| self.block_at(Self::pos_at_block(pos, coord.into())),
        ))
    }
}

pub trait ChunkGenerator<T>: Send + Sync {
    fn generate_chunk(&self, pos: ChunkPos) -> Chunk<T>;
}

pub struct TerrainGenerator {
    chunk_rx: mpsc::Receiver<(ChunkPos, Chunk<BlockId>)>,
    request_tx: mpsc::Sender<ChunkPos>,
    queue: HashSet<ChunkPos>,
}

impl TerrainGenerator {
    pub fn new<G: ChunkGenerator<BlockId> + 'static>(gen: G) -> Self {
        let (request_tx, request_rx) = mpsc::channel();
        let (chunk_tx, chunk_rx) = mpsc::channel();

        ::std::thread::spawn(move || {
            while let Ok(request) = request_rx.recv() {
                match chunk_tx.send((request, gen.generate_chunk(request))) {
                    Ok(_) => (),
                    // Err means the rx has hung up, so we can just shut down this thread
                    // if that happens
                    Err(_) => break,
                }
            }
        });

        TerrainGenerator {
            chunk_rx,
            request_tx,
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
        let dist = view_distance.0;
        for (_, transform) in (&players, &transforms).join() {
            let base_pos: ChunkPos = WorldPos(transform.position).into();
            for xo in -dist.x..=dist.x {
                for yo in -dist.y..=dist.y {
                    for zo in -dist.z..=dist.z {
                        let pos = base_pos.offset((xo, yo, zo));
                        if !voxel_world.chunk_exists(pos) && !self.queue.contains(&pos) {
                            self.queue.insert(pos);
                            self.request_tx.send(pos).unwrap();
                        }
                    }
                }
            }
        }

        let mut section = debug.section("terrain generation");
        for item in self.queue.iter() {
            section.draw(Shape::Chunk(2.0, *item, Vector4::new(1.0, 0.0, 0.0, 1.0)));
        }

        for (pos, chunk) in self.chunk_rx.try_iter() {
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
