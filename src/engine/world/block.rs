use cgmath::Vector2;
use engine::Side;
use std::{collections::HashMap, error::Error, io, path::Path};

pub const AIR: BlockId = BlockId(0);
pub const STONE: BlockId = BlockId(1);
pub const DIRT: BlockId = BlockId(2);
pub const GRASS: BlockId = BlockId(3);
pub const WATER: BlockId = BlockId(4);

#[derive(Clone, Debug, PartialEq, Deserialize)]
enum BlockTextures {
    /// The textures for all the faces are all the same
    #[serde(rename = "same")]
    AllSame(String),

    /// The textures for the sides are all the same, but the top and the bottom
    /// are different, like a grass block
    #[serde(rename = "top_bottom")]
    TopBottom {
        top: String,
        bottom: String,
        side: String,
    },

    /// The texture for each face is different
    #[serde(rename = "different")]
    AllDifferent {
        top: String,
        bottom: String,
        left: String,
        right: String,
        front: String,
        back: String,
    },
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
struct BlockRegistryEntry {
    name: String,
    collidable: bool,
    opaque: bool,
    textures: Option<BlockTextures>,
}

impl BlockTextures {
    fn expand(self) -> BlockFaces<String> {
        match self {
            BlockTextures::AllSame(val) => BlockFaces {
                top: val.clone(),
                bottom: val.clone(),
                left: val.clone(),
                right: val.clone(),
                front: val.clone(),
                back: val,
            },
            BlockTextures::TopBottom { top, bottom, side } => BlockFaces {
                top,
                bottom,
                left: side.clone(),
                right: side.clone(),
                front: side.clone(),
                back: side,
            },
            BlockTextures::AllDifferent {
                top,
                bottom,
                left,
                right,
                front,
                back,
            } => BlockFaces {
                top,
                bottom,
                left,
                right,
                front,
                back,
            },
        }
    }
}

pub enum RegistryFileError {
    Io(io::Error),
    Serde(()),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub struct BlockFaces<T> {
    pub top: T,
    pub bottom: T,
    pub left: T,
    pub right: T,
    pub front: T,
    pub back: T,
}

impl<T> BlockFaces<T> {
    fn map<U, F>(self, mut func: F) -> BlockFaces<U>
    where
        F: FnMut(T) -> U,
    {
        BlockFaces {
            top: func(self.top),
            bottom: func(self.bottom),
            left: func(self.left),
            right: func(self.right),
            front: func(self.front),
            back: func(self.back),
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct BlockId(usize);

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct BlockRegistryBuilder {
    // per-block
    names: Vec<String>,
    opaque: Vec<bool>,
    collidable: Vec<bool>,
    texture_indices: Vec<Option<BlockFaces<usize>>>,

    // other
    textures: Vec<String>,
}

fn get_or_insert_texture(textures: &mut Vec<String>, val: String) -> usize {
    textures
        .iter()
        .position(|item| item == &val)
        .unwrap_or_else(|| {
            textures.push(val);
            textures.len() - 1
        })
}

impl BlockRegistryBuilder {
    pub fn register(
        &mut self,
        name: String,
        opaque: bool,
        collidable: bool,
        textures: Option<BlockFaces<String>>,
    ) {
        self.names.push(name);
        self.opaque.push(opaque);
        self.collidable.push(collidable);

        if let Some(textures) = textures {
            let texture_vec = &mut self.textures;
            self.texture_indices.push(Some(
                textures.map(|name| get_or_insert_texture(texture_vec, name)),
            ));
        } else {
            self.texture_indices.push(None);
        }
    }

    pub fn build(self) -> (BlockRegistry, Vec<String>) {
        let mut registry = BlockRegistry::default();

        registry.name_map = self
            .names
            .into_iter()
            .enumerate()
            .map(|(a, b)| (b, BlockId(a)))
            .collect();

        registry.opaque = self.opaque;
        registry.collidable = self.collidable;
        registry.texture_indices = self.texture_indices;

        (registry, self.textures)
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct BlockRegistry {
    name_map: HashMap<String, BlockId>,
    opaque: Vec<bool>,
    collidable: Vec<bool>,
    texture_indices: Vec<Option<BlockFaces<usize>>>,
}

impl BlockRegistry {
    // pub fn with_defaults(mut self) -> Self {
    //     macro_rules! proto {
    //         ($opaque:expr, $solid:expr, [$($x:expr, $y:expr);*]) => {
    //             BlockProperties {
    //                 opaque: $opaque,
    //                 collidable: $solid,
    //                 texture_offsets: [$(Vector2::new($x as f32, $y as f32)),*]
    //             }
    //         };
    //     }

    //     self.register(
    //         "air",
    //         proto! { false, false, [0.0, 0.0; 0.0, 0.0; 0.0, 0.0; 0.0, 0.0; 0.0,
    // 0.0; 0.0, 0.0] },         Some(AIR),
    //     );
    //     self.register(
    //         "stone",
    //         proto! { true,  true,  [1.0, 0.0; 1.0, 0.0; 1.0, 0.0; 1.0, 0.0; 1.0,
    // 0.0; 1.0, 0.0] },         Some(STONE),
    //     );
    //     self.register(
    //         "dirt",
    //         proto! { true,  true,  [2.0, 0.0; 2.0, 0.0; 2.0, 0.0; 2.0, 0.0; 2.0,
    // 0.0; 2.0, 0.0] },         Some(DIRT),
    //     );
    //     self.register(
    //         "grass",
    //         proto! { true,  true,  [0.0, 1.0; 0.0, 0.0; 0.0, 1.0; 0.0, 1.0; 2.0,
    // 0.0; 0.0, 1.0] },         Some(GRASS),
    //     );
    //     self.register(
    //         "water",
    //         proto! { true,  true,  [1.0, 0.0; 1.0, 0.0; 1.0, 0.0; 1.0, 0.0; 1.0,
    // 0.0; 1.0, 0.0] },         Some(WATER),
    //     );

    //     self
    // }

    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<(Self, Vec<String>), Box<Error>> {
        let entries: Vec<BlockRegistryEntry> =
            serde_json::from_reader(::std::fs::File::open(path)?)?;
        let mut builder = BlockRegistryBuilder::default();

        for entry in entries {
            builder.register(
                entry.name,
                entry.opaque,
                entry.collidable,
                entry.textures.map(BlockTextures::expand),
            );
        }

        Ok(builder.build())
    }

    pub fn opaque(&self, id: BlockId) -> bool {
        self.opaque[id.0]
    }

    pub fn collidable(&self, id: BlockId) -> bool {
        self.collidable[id.0]
    }

    pub fn block_textures(&self, id: BlockId) -> &Option<BlockFaces<usize>> {
        &self.texture_indices[id.0]
    }

    pub fn block_texture(&self, id: BlockId, side: Side) -> Option<usize> {
        self.texture_indices[id.0].as_ref().map(|faces| match side {
            Side::Top => faces.top,
            Side::Right => faces.right,
            Side::Front => faces.front,
            Side::Left => faces.left,
            Side::Bottom => faces.bottom,
            Side::Back => faces.back,
        })
    }

    pub fn get_ref(&self, id: BlockId) -> RegistryRef {
        RegistryRef { registry: self, id }
    }
}

pub struct RegistryRef<'r> {
    registry: &'r BlockRegistry,
    id: BlockId,
}

impl<'r> RegistryRef<'r> {
    #[inline(always)]
    pub fn opaque(&self) -> bool {
        self.registry.opaque[self.id.0]
    }

    #[inline(always)]
    pub fn collidable(&self) -> bool {
        self.registry.collidable[self.id.0]
    }

    #[inline(always)]
    pub fn block_textures(&self) -> &Option<BlockFaces<usize>> {
        &self.registry.texture_indices[self.id.0]
    }

    #[inline(always)]
    pub fn block_texture(&self, side: Side) -> Option<usize> {
        self.registry.texture_indices[self.id.0]
            .as_ref()
            .map(|faces| match side {
                Side::Top => faces.top,
                Side::Right => faces.right,
                Side::Front => faces.front,
                Side::Left => faces.left,
                Side::Bottom => faces.bottom,
                Side::Back => faces.back,
            })
    }
}
