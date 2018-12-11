use crate::engine::prelude::*;
use rand::prelude::*;
use rodio::{Decoder, Device, Sink, Source};
use std::{
    fs, io,
    path::{Path, PathBuf},
    time::Duration,
};

const MIN_MUSIC_GAP_SECS: f64 = 60.0;
const MAX_MUSIC_GAP_SECS: f64 = 600.0;

fn random_duration() -> Duration {
    Duration::from_float_secs(rand::thread_rng().gen_range(MIN_MUSIC_GAP_SECS, MAX_MUSIC_GAP_SECS))
}

struct AudioManagerInner {
    _device: Device,
    music_sink: Sink,
}

fn walk_dirs(path: impl AsRef<Path>) -> io::Result<Vec<PathBuf>> {
    let dir_iter = fs::read_dir(path)?;

    let mut items = vec![];
    for entry in dir_iter {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            items.extend(walk_dirs(entry.path())?);
        } else {
            items.push(entry.path());
        }
    }

    Ok(items)
}

fn select_audio_file<P: AsRef<Path>>(dir: P) -> io::Result<Option<Decoder<fs::File>>> {
    Ok(walk_dirs(dir)?
        .choose(&mut rand::thread_rng())
        .map(|path| {
            debug!("Trying to open {}", path.display());
            fs::File::open(path)
        })
        .transpose()?
        .and_then(|file| Decoder::new(file).ok()))
}

impl AudioManagerInner {
    fn try_play_music(&mut self) {
        if self.music_sink.empty() {
            if let Some(Some(source)) = select_audio_file("resources/audio").ok() {
                let duration = random_duration();
                debug!("Playing music in {} seconds", duration.as_float_secs());
                self.music_sink.append(source.delay(duration));
            }
        }
    }
}

pub struct AudioManager(Option<AudioManagerInner>);

impl AudioManager {
    pub fn new() -> Self {
        AudioManager(
            rodio::default_output_device().map(|device| AudioManagerInner {
                music_sink: Sink::new(&device),
                _device: device,
            }),
        )
    }
}

impl<'a> System<'a> for AudioManager {
    type SystemData = ();

    fn run(&mut self, _: Self::SystemData) {
        if let AudioManager(Some(inner)) = self {
            inner.try_play_music();
        }
    }
}
