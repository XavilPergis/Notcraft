use rand::prelude::*;
use rodio::{Decoder, Device, OutputStream, Sink, Source};
use std::{
    fs, io,
    path::{Path, PathBuf},
    time::Duration,
};

const MIN_MUSIC_GAP_MILLISECS: u64 = 6000;
const MAX_MUSIC_GAP_MILLISECS: u64 = 60000;

fn random_duration() -> Duration {
    Duration::from_millis(
        rand::thread_rng().gen_range(MIN_MUSIC_GAP_MILLISECS, MAX_MUSIC_GAP_MILLISECS),
    )
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

pub struct MusicState {
    music_sink: Sink,
    output_stream: OutputStream,
}

impl MusicState {
    pub fn new() -> Self {
        let (os, handle) = OutputStream::try_default().unwrap();
        let sink = Sink::try_new(&handle).unwrap();

        Self {
            music_sink: sink,
            output_stream: os,
        }
    }
}

#[legion::system]
pub fn intermittent_music(#[state] state: &mut MusicState) {
    if state.music_sink.empty() {
        if let Some(Some(source)) = select_audio_file("resources/audio").ok() {
            let duration = random_duration();
            debug!("Playing music in {} seconds", duration.as_secs_f64());
            state.music_sink.append(source.delay(duration));
        }
    }
}
