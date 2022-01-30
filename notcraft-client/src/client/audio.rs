use ambisonic::{
    rodio::{Decoder, Source},
    Ambisonic, AmbisonicBuilder, SoundController,
};
use nalgebra::{Point3, Vector3};
use notcraft_common::{prelude::*, transform::Transform};
use std::{
    collections::HashMap,
    io::{Cursor, Read},
    sync::Arc,
    time::{Duration, Instant},
};

pub struct AudioState {
    next_id: AudioId,
    audio: HashMap<AudioId, Arc<[u8]>>,
}

impl AudioState {
    pub fn new() -> Result<Self> {
        Ok(Self {
            next_id: AudioId(0),
            audio: Default::default(),
        })
    }

    pub fn add<R>(&mut self, mut reader: R) -> Result<AudioId>
    where
        R: Read,
    {
        let mut data = Vec::new();
        reader.read_to_end(&mut data)?;

        let id = self.next_id;
        self.next_id.0 += 1;

        self.audio.insert(id, data.into_boxed_slice().into());
        Ok(id)
    }

    fn get(&self, id: AudioId) -> Arc<[u8]> {
        self.audio[&id].clone()
    }
}

#[derive(Clone, Debug)]
pub struct AudioListener {
    // TODO: make this work. or maybe we want a different api?
    volume: f32,
}

impl Default for AudioListener {
    fn default() -> Self {
        Self { volume: 1.0 }
    }
}

// might add custom sources in the future
#[derive(Debug)]
pub enum EmitterSource {
    Sample(AudioId),
}

pub struct AudioEmitter {
    sound: SoundController,
    start: Instant,
    duration: Option<Duration>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
struct DespawnEmitter;

impl std::fmt::Debug for AudioEmitter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioEmitter")
            .field("start", &self.start)
            .field("duration", &self.duration)
            .finish_non_exhaustive()
    }
}

fn update_emitters(
    mut cmd: Commands,
    active_listener: Res<ActiveAudioListener>,
    listener_query: Query<(&Transform, &AudioListener)>,
    emitter_query: Query<(
        Entity,
        &Transform,
        &mut AudioEmitter,
        Option<&DespawnEmitter>,
    )>,
) {
    let (listener_transform, _) = match active_listener.0.and_then(|e| listener_query.get(e).ok()) {
        Some(it) => it,
        _ => return,
    };

    emitter_query.for_each_mut(|(entity, transform, mut emitter, despawn)| {
        match emitter.duration {
            Some(duration) if emitter.start.elapsed() > duration => match despawn {
                Some(DespawnEmitter) => cmd.entity(entity).despawn(),
                None => drop(cmd.entity(entity).remove::<AudioEmitter>()),
            },

            _ => {
                // TODO: we could also update emitter velocities here!
                let matrix = listener_transform.to_matrix().try_inverse().unwrap();
                let audio_pos = matrix.transform_point(&transform.pos());

                emitter.sound.adjust_position(audio_pos.into());
            }
        }
    });
}

fn process_audio_events(
    mut cmd: Commands,
    audio_scene: NonSend<Ambisonic>,
    state: Res<AudioState>,
    mut events: EventReader<AudioEvent>,
    active_listener: Res<ActiveAudioListener>,
    listener_query: Query<(&Transform, &AudioListener)>,
    emitter_query: Query<(Entity, &Transform)>,
) {
    let (listener_transform, _) = match active_listener.0.and_then(|e| listener_query.get(e).ok()) {
        Some(it) => it,
        _ => return,
    };

    for event in events.iter() {
        let source = match event.source() {
            &EmitterSource::Sample(id) => Decoder::new(Cursor::new(state.get(id))),
        };
        // TODO: unwrap
        let source = source.unwrap().convert_samples();
        match event {
            AudioEvent::PlaySpatial(entity, _) => {
                if let Ok((entity, transform)) = emitter_query.get(*entity) {
                    let duration = source.total_duration();

                    let matrix = listener_transform.to_matrix().try_inverse().unwrap();
                    let audio_pos = matrix.transform_point(&transform.pos());

                    let sound = audio_scene.play_at(source, audio_pos.into());
                    cmd.entity(entity).insert(AudioEmitter {
                        sound,
                        start: Instant::now(),
                        duration,
                    });
                }
            }

            &AudioEvent::SpawnSpatial(pos, _) => {
                let duration = source.total_duration();

                let matrix = listener_transform.to_matrix().try_inverse().unwrap();
                let audio_pos = matrix.transform_point(&pos);

                let sound = audio_scene.play_at(source, audio_pos.into());
                cmd.spawn()
                    .insert(Transform::to(pos))
                    .insert(DespawnEmitter)
                    .insert(AudioEmitter {
                        sound,
                        start: Instant::now(),
                        duration,
                    });
            }

            AudioEvent::PlayGlobal(_) => {
                // TODO: unwrap
                audio_scene.play_omni(source);
            }
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct AudioId(usize);

#[derive(Debug)]
pub enum AudioEvent {
    /// Notifies the sound system to play a 3D sound at the given entity's
    /// location, and attaches an [`AudioEmitter`] component to the entity.
    /// If the entity is moved, the audio emitter will be as well. The component
    /// will be removed from the entity when the sound is done playing.
    PlaySpatial(Entity, EmitterSource),

    /// Similar to [`AudioEvent::PlaySpatial`], except this also spawns a
    /// temporary "holder" object to contain the emitter. The temporary
    /// entity is managed by the audio system and is despawned after the
    /// sound is done playing.
    SpawnSpatial(Point3<f32>, EmitterSource),

    /// Notifies the sound system to play a sound directly to the active audio
    /// listener, without spatial effects.
    PlayGlobal(EmitterSource),
}

impl AudioEvent {
    pub fn source(&self) -> &EmitterSource {
        match self {
            AudioEvent::PlaySpatial(_, source)
            | AudioEvent::SpawnSpatial(_, source)
            | AudioEvent::PlayGlobal(source) => source,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ActiveAudioListener(pub Option<Entity>);

#[derive(Debug, Default)]
pub struct AudioPlugin {}

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut AppBuilder) {
        app.insert_non_send_resource(AmbisonicBuilder::default().build());
        app.insert_resource(AudioState::new().expect("failed to init audio"));
        app.insert_resource(ActiveAudioListener(None));

        app.add_event::<AudioEvent>();

        app.add_system_to_stage(CoreStage::PostUpdate, update_emitters.system());
        app.add_system_to_stage(CoreStage::PostUpdate, process_audio_events.system());
    }
}
