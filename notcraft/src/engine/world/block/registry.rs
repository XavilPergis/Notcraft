use crate::engine::{world::block::Faces, Side};
use rand::prelude::*;
use std::{collections::HashMap, error::Error, path::Path};

pub const AIR: BlockId = BlockId(0);
pub const STONE: BlockId = BlockId(1);
pub const DIRT: BlockId = BlockId(2);
pub const GRASS: BlockId = BlockId(3);
pub const SAND: BlockId = BlockId(4);
pub const WATER: BlockId = BlockId(5);

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum FaceTexture<T> {
    Always(T),
    Weighted(Vec<(f32, T)>),
}

fn weighted_select<T>(items: &Vec<(f32, T)>) -> &T {
    assert!(items.len() > 0);

    let sum: f32 = items.iter().map(|(weight, _)| weight).sum();
    let mut num = SmallRng::from_entropy().gen_range(1.0, sum);

    for item in items {
        num -= item.0;
        if num <= 0.0 {
            return &item.1;
        }
    }

    unreachable!()
}

impl<T> FaceTexture<T> {
    fn map<U, F>(self, mut func: F) -> FaceTexture<U>
    where
        F: FnMut(T) -> U,
    {
        match self {
            FaceTexture::Always(val) => FaceTexture::Always(func(val)),
            FaceTexture::Weighted(vec) => FaceTexture::Weighted(
                vec.into_iter()
                    .map(|(weight, item)| (weight, func(item)))
                    .collect(),
            ),
        }
    }

    pub fn select(&self) -> &T {
        match self {
            FaceTexture::Always(item) => item,
            FaceTexture::Weighted(vec) => weighted_select(vec),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct BlockFace<T> {
    pub random_orientation: bool,
    pub texture: FaceTexture<T>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum BlockTextures {
    /// The textures for all the faces are all the same
    #[serde(rename = "same")]
    AllSame(BlockFace<String>),

    /// The textures for the sides are all the same, but the top and the bottom
    /// are different, like a grass block
    #[serde(rename = "top_bottom")]
    TopBottom {
        top: BlockFace<String>,
        bottom: BlockFace<String>,
        side: BlockFace<String>,
    },

    /// The texture for each face is different
    #[serde(rename = "different")]
    AllDifferent {
        top: BlockFace<String>,
        bottom: BlockFace<String>,
        left: BlockFace<String>,
        right: BlockFace<String>,
        front: BlockFace<String>,
        back: BlockFace<String>,
    },
}

impl BlockTextures {
    fn expand(self) -> Faces<BlockFace<String>> {
        match self {
            BlockTextures::AllSame(val) => Faces {
                top: val.clone(),
                bottom: val.clone(),
                left: val.clone(),
                right: val.clone(),
                front: val.clone(),
                back: val,
            },
            BlockTextures::TopBottom { top, bottom, side } => Faces {
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
            } => Faces {
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

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct BlockRegistryEntry {
    name: String,
    collidable: bool,
    opaque: bool,
    liquid: bool,
    textures: Option<BlockTextures>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct BlockId(pub(crate) usize);

#[derive(Clone, Debug, PartialEq, Default)]
pub struct BlockRegistryBuilder {
    // per-block
    names: Vec<String>,
    opaque: Vec<bool>,
    collidable: Vec<bool>,
    liquid: Vec<bool>,
    texture_indices: Vec<Option<Faces<BlockFace<usize>>>>,

    // other
    textures: Vec<String>,
}

impl BlockRegistryBuilder {
    pub fn register(&mut self, entry: BlockRegistryEntry) {
        self.names.push(entry.name);
        self.opaque.push(entry.opaque);
        self.collidable.push(entry.collidable);
        self.liquid.push(entry.liquid);

        if let Some(textures) = entry.textures {
            // expand the face textures into a `Faces`, where all sides are reified
            // into fields for each face, try to add the face items into the
            // textures array
            let faces_ref = textures.expand().map(|face| {
                let texture = face.texture.map(|name| {
                    // if the texture was already added to the list, return its index. We pass the
                    // list of names to the terrain renderer later so it can load the files
                    // associated with the names. We don't want to load the same thing twice and
                    // then store the extraneous texture in the texture array.
                    // TODO: greater than linear time :(
                    if let Some(idx) = self.get_texture_index(&name) {
                        idx
                    } else {
                        // if the item was not found, push it onto the vec and return its index,
                        // which will the the index of the last item on the list
                        self.textures.push(name);
                        self.textures.len() - 1
                    }
                });
                BlockFace {
                    texture,
                    random_orientation: face.random_orientation,
                }
            });

            self.texture_indices.push(Some(faces_ref));
        } else {
            self.texture_indices.push(None);
        }
    }

    fn get_texture_index(&self, name: &str) -> Option<usize> {
        self.textures.iter().position(|item| item == name)
    }

    pub fn build(self) -> BlockRegistry {
        let mut registry = BlockRegistry::default();

        debug!("builder: {:#?}", &self);

        registry.name_map = self
            .names
            .into_iter()
            .enumerate()
            .map(|(a, b)| (b, BlockId(a)))
            .collect();

        registry.opaque = self.opaque;
        registry.collidable = self.collidable;
        registry.texture_indices = self.texture_indices;
        registry.liquid = self.liquid;
        registry.texture_paths = self.textures;

        registry
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct BlockRegistry {
    name_map: HashMap<String, BlockId>,
    opaque: Vec<bool>,
    collidable: Vec<bool>,
    liquid: Vec<bool>,
    texture_indices: Vec<Option<Faces<BlockFace<usize>>>>,
    texture_paths: Vec<String>,
}

impl BlockRegistry {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>> {
        let entries: Vec<BlockRegistryEntry> =
            serde_json::from_reader(::std::fs::File::open(path)?)?;
        let mut builder = BlockRegistryBuilder::default();

        // could probably use Iterator::fold here for extra cool points :sunglasses:
        for entry in entries {
            debug!("Adding {:#?}", entry);
            builder.register(entry);
        }

        Ok(builder.build())
    }

    pub fn get_id(&self, name: &str) -> BlockId {
        self.name_map[name]
    }

    pub(crate) fn num_entries(&self) -> usize {
        self.opaque.len()
    }

    pub fn texture_paths<'a>(&'a self) -> impl Iterator<Item = &'a str> {
        self.texture_paths.iter().map(|s| &**s)
    }

    #[inline(always)]
    pub fn opaque(&self, id: BlockId) -> bool {
        self.opaque[id.0]
    }

    #[inline(always)]
    pub fn collidable(&self, id: BlockId) -> bool {
        self.collidable[id.0]
    }

    #[inline(always)]
    pub fn liquid(&self, id: BlockId) -> bool {
        self.liquid[id.0]
    }

    #[inline(always)]
    pub fn block_textures(&self, id: BlockId) -> &Option<Faces<BlockFace<usize>>> {
        &self.texture_indices[id.0]
    }

    pub fn block_texture(&self, id: BlockId, side: Side) -> Option<&BlockFace<usize>> {
        self.texture_indices[id.0].as_ref().map(|faces| match side {
            Side::Top => &faces.top,
            Side::Right => &faces.right,
            Side::Front => &faces.front,
            Side::Left => &faces.left,
            Side::Bottom => &faces.bottom,
            Side::Back => &faces.back,
        })
    }

    #[inline(always)]
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
        self.registry.opaque(self.id)
    }

    #[inline(always)]
    pub fn collidable(&self) -> bool {
        self.registry.collidable(self.id)
    }

    #[inline(always)]
    pub fn liquid(&self) -> bool {
        self.registry.liquid(self.id)
    }

    #[inline(always)]
    pub fn block_textures(&self) -> &Option<Faces<BlockFace<usize>>> {
        self.registry.block_textures(self.id)
    }

    #[inline(always)]
    pub fn block_texture(&self, side: Side) -> Option<&BlockFace<usize>> {
        self.registry.block_texture(self.id, side)
    }
}
