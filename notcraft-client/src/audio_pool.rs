use notcraft_common::prelude::*;
use rand::Rng;
use serde::Deserialize;
use std::{collections::HashMap, fs::File, path::Path};

use crate::{
    client::audio::{AudioId, AudioState, EmitterParameters},
    WeightedList,
};

// #[derive(Clone, Debug, PartialEq, Deserialize)]
// pub struct ManifestPoolParams {
//     min_pitch: Option<f32>,
//     max_pitch: Option<f32>,
//     min_amplitude: Option<f32>,
//     max_amplitude: Option<f32>,
// }

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum ManifestNode {
    /// A reference to another node in the manifest
    Ref(String),
    Pool {
        inherit: Option<String>,
        patterns: Option<Vec<String>>,
        // #[serde(flatten)]
        // params: ManifestPoolParams,
        min_pitch: Option<f32>,
        max_pitch: Option<f32>,
        min_amplitude: Option<f32>,
        max_amplitude: Option<f32>,
    },
    Choice(Vec<Box<ManifestNode>>),
    Weighted(Vec<(usize, Box<ManifestNode>)>),
    Layered {
        /// node used when no sounds are selected to be played from the list of
        /// layers.
        default: Box<ManifestNode>,
        layers: Vec<(f32, Box<ManifestNode>)>,
    },
}

#[derive(Debug)]
enum ManifestError {
    Io(std::io::Error),
    Glob(glob::GlobError),
    UnknownReference(String),
    CannotInherit(String),
}

impl std::error::Error for ManifestError {}
impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ManifestError::Io(err) => err.fmt(f),
            ManifestError::Glob(err) => err.fmt(f),
            ManifestError::UnknownReference(name) => {
                write!(f, "'{name}' was not found in the manifest")
            }
            ManifestError::CannotInherit(name) => write!(f, "cannot inherit from entry '{name}'"),
        }
    }
}

impl From<std::io::Error> for ManifestError {
    fn from(err: std::io::Error) -> Self {
        ManifestError::Io(err)
    }
}

impl From<glob::GlobError> for ManifestError {
    fn from(err: glob::GlobError) -> Self {
        ManifestError::Glob(err)
    }
}

fn get_node_ref<'n>(
    name: &str,
    manifest: &'n AudioManifest,
) -> Result<&'n ManifestNode, ManifestError> {
    let res = manifest.0.get(name);
    res.ok_or_else(|| ManifestError::UnknownReference(name.into()))
}

struct InheritanceOut<'n> {
    patterns: &'n Option<Vec<String>>,
    min_pitch: &'n Option<f32>,
    max_pitch: &'n Option<f32>,
    min_amplitude: &'n Option<f32>,
    max_amplitude: &'n Option<f32>,
}

fn resolve_inheritance<'n>(
    node: &'n ManifestNode,
    manifest: &'n AudioManifest,
    name: &str,
    out: &mut InheritanceOut<'n>,
) -> Result<(), ManifestError> {
    match node {
        ManifestNode::Ref(name) => {
            resolve_inheritance(get_node_ref(name, manifest)?, manifest, name, out)?
        }
        ManifestNode::Pool {
            inherit,
            patterns,
            min_pitch,
            max_pitch,
            min_amplitude,
            max_amplitude,
        } => {
            fn apply<'a, T>(out: &mut &'a Option<T>, val: &'a Option<T>) {
                if out.is_none() {
                    *out = val;
                }
            }

            apply(&mut out.patterns, patterns);
            apply(&mut out.min_pitch, &min_pitch);
            apply(&mut out.max_pitch, &max_pitch);
            apply(&mut out.min_amplitude, &min_amplitude);
            apply(&mut out.max_amplitude, &max_amplitude);

            if let Some(name) = inherit {
                resolve_inheritance(get_node_ref(name, manifest)?, manifest, name, out)?
            }
        }
        _ => return Err(ManifestError::CannotInherit(name.into())),
    }
    Ok(())
}

fn resolve_node(
    node: &ManifestNode,
    manifest: &AudioManifest,
    state: &mut AudioState,
    last_name: &str,
) -> Result<AudioNode> {
    Ok(match node {
        ManifestNode::Ref(name) => {
            resolve_node(get_node_ref(name, manifest)?, manifest, state, name)?
        }
        ManifestNode::Pool {
            inherit: _,
            patterns,
            min_pitch,
            max_pitch,
            min_amplitude,
            max_amplitude,
        } => {
            let mut out = InheritanceOut {
                patterns,
                min_pitch,
                max_pitch,
                min_amplitude,
                max_amplitude,
            };
            resolve_inheritance(node, manifest, last_name, &mut out)?;

            let empty = Vec::new();
            let patterns = out.patterns.as_ref().unwrap_or(&empty);
            let params = EmitterParameters {
                min_pitch: out.min_pitch.unwrap_or(1.0),
                max_pitch: out.max_pitch.unwrap_or(1.0),
                min_amplitude: out.min_amplitude.unwrap_or(1.0),
                max_amplitude: out.max_amplitude.unwrap_or(1.0),
            };

            let mut items = WeightedList::default();
            for pattern in patterns.iter() {
                // TODO: does this allow attackers to use `..` to escape the resources dir?
                // would it even matter?
                for path in glob::glob(&format!("resources/audio/{pattern}"))? {
                    let id = state.add(File::open(path?)?)?;
                    items.push(1, Box::new(AudioNode::Sound { id, params }));
                }
            }
            AudioNode::Choice(items)
        }

        ManifestNode::Choice(choices) => {
            let mut items = WeightedList::default();
            for choice in choices.iter() {
                let node = resolve_node(choice, manifest, state, last_name)?;
                items.push(1, Box::new(node));
            }
            AudioNode::Choice(items)
        }
        ManifestNode::Weighted(choices) => {
            let mut items = WeightedList::default();
            for &(weight, ref choice) in choices.iter() {
                let node = resolve_node(choice, manifest, state, last_name)?;
                items.push(weight, Box::new(node));
            }
            AudioNode::Choice(items)
        }
        ManifestNode::Layered { default, layers } => AudioNode::Layered {
            default: Box::new(resolve_node(default, manifest, state, last_name)?),
            layers: {
                let mut out = Vec::with_capacity(layers.len());
                for &(probability, ref layer) in layers.iter() {
                    let node = resolve_node(layer, manifest, state, last_name)?;
                    out.push((probability, Box::new(node)));
                }
                out
            },
        },
    })
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct AudioManifest(HashMap<String, ManifestNode>);

#[derive(Clone, Debug)]
enum AudioNode {
    Sound {
        id: AudioId,
        params: EmitterParameters,
    },
    Layered {
        default: Box<AudioNode>,
        layers: Vec<(f32, Box<AudioNode>)>,
    },
    Choice(WeightedList<Box<AudioNode>>),
}

#[derive(Clone, Debug, Default)]
pub struct RandomizedAudioPools {
    sound_idx_map: HashMap<String, usize>,
    sounds: Vec<AudioNode>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct SoundId(usize);

impl RandomizedAudioPools {
    pub fn id(&self, name: &str) -> Option<SoundId> {
        self.sound_idx_map.get(name).copied().map(SoundId)
    }

    pub fn select<R, F>(&self, rng: &mut R, id: SoundId, mut func: F)
    where
        R: Rng + ?Sized,
        F: FnMut(AudioId, EmitterParameters),
    {
        select_sounds(rng, &self.sounds[id.0], &mut func)
    }
}

fn select_sounds<R, F>(rng: &mut R, node: &AudioNode, func: &mut F)
where
    R: Rng + ?Sized,
    F: FnMut(AudioId, EmitterParameters),
{
    match node {
        &AudioNode::Sound { id, params } => func(id, params),
        AudioNode::Layered { default, layers } => {
            let mut use_default = true;
            for &(probability, ref node) in layers.iter() {
                if rng.gen_bool(probability as f64) {
                    use_default = false;
                    select_sounds(rng, node, func)
                }
            }
            if use_default {
                select_sounds(rng, default, func)
            }
        }
        AudioNode::Choice(choices) => {
            if let Some(choice) = choices.select(rng) {
                select_sounds(rng, choice, func)
            }
        }
    }
}

pub fn load_audio<P: AsRef<Path>>(path: P, state: &mut AudioState) -> Result<RandomizedAudioPools> {
    let manifest: AudioManifest = ron::from_str(&std::fs::read_to_string(path)?)?;

    let mut pools = RandomizedAudioPools::default();
    for (name, node) in manifest.0.iter() {
        match resolve_node(node, &manifest, state, name) {
            Ok(resolved) => {
                pools.sound_idx_map.insert(name.into(), pools.sounds.len());
                pools.sounds.push(resolved);
            }
            // having a screwed up entry is ok, we just don't add it to the pools.
            Err(err) => log::error!("audio manifest entry '{name}' failed to load: {err}"),
        }
    }

    Ok(pools)
}
