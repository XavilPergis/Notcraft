use simdnoise::NoiseBuilder;

use crate::engine::world::{
    chunk::{SIZE, VOLUME},
    Chunk,
};

use super::{
    chunk::ChunkType,
    registry::{BlockId, BlockRegistry, AIR},
    ChunkPos,
};

#[derive(Clone, Debug)]
pub struct NoiseGenerator {
    stone_id: BlockId,
    dirt_id: BlockId,
    grass_id: BlockId,
}

impl NoiseGenerator {
    pub fn new_default(registry: &BlockRegistry) -> Self {
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
            AIR
        }
    }

    pub fn make_chunk(&self, pos: ChunkPos) -> ChunkType {
        let base = pos.base().0;
        let noise = NoiseBuilder::gradient_2d_offset(base.x as f32, SIZE, base.z as f32, SIZE)
            .with_freq(0.01)
            // .with_octaves(5)
            .with_seed(1337)
            // .with_lacunarity(0.5)
            .generate_scaled(-10.0, 10.0);

        // if base.y as f32 - noise_min > 0.0 {
        //     return ChunkType::Homogeneous(block::AIR);
        // }

        let mut chunk_data = Vec::with_capacity(VOLUME);
        for x in 0..SIZE {
            for z in 0..SIZE {
                let surface_height = noise[x as usize * SIZE + z];

                for y in 0..SIZE {
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
