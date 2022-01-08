use notcraft_common::prelude::*;
use rand::prelude::*;
use rodio::{Decoder, OutputStream, Sink, Source};
use std::{
    fs, io,
    path::{Path, PathBuf},
    time::Duration,
};

const MIN_MUSIC_GAP_SECONDS: u64 = 120;
const MAX_MUSIC_GAP_SECONDS: u64 = MIN_MUSIC_GAP_SECONDS * 5;

fn random_duration() -> Duration {
    Duration::from_secs(rand::thread_rng().gen_range(MIN_MUSIC_GAP_SECONDS, MAX_MUSIC_GAP_SECONDS))
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
    _output_stream: OutputStream,
}

impl MusicState {
    pub fn new() -> Self {
        let (os, handle) = OutputStream::try_default().unwrap();
        let sink = Sink::try_new(&handle).unwrap();

        Self {
            music_sink: sink,
            _output_stream: os,
        }
    }
}

pub fn intermittent_music(ctx: NonSendMut<MusicState>) {
    if ctx.music_sink.empty() {
        if let Some(Some(source)) = select_audio_file("resources/audio").ok() {
            let duration = random_duration();
            debug!("playing music in {} seconds", duration.as_secs_f64());
            ctx.music_sink.append(source.delay(duration));
        }
    }
}
