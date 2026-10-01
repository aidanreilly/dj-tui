use crate::LoadError;
use std::path::Path;
use symphonia::core::{
    audio::SampleBuffer,
    codecs::{DecoderOptions, CODEC_TYPE_NULL},
    errors::Error as SymError,
    formats::FormatOptions,
    io::MediaSourceStream,
    meta::{MetadataOptions, StandardTagKey, Tag},
    probe::Hint,
};

pub(crate) struct Decoded {
    /// Planar stereo. Mono sources are duplicated, extra channels dropped.
    pub channels: [Vec<f32>; 2],
    pub sample_rate: u32,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub genre: Option<String>,
}

/// What a file says about itself, without decoding a single packet.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tags {
    pub title: Option<String>,
    pub artist: Option<String>,
    /// Whatever the file's own genre tag says. Nothing here is inferred from the audio.
    pub genre: Option<String>,
}

fn tag_value(tags: &[Tag], key: StandardTagKey) -> Option<String> {
    tags.iter()
        .find(|t| t.std_key == Some(key))
        // RIFF INFO strings are NUL-terminated and their chunk is word-aligned, so a value
        // arrives with trailing NULs on it. Stored as they come they reach the track list.
        .map(|t| t.value.to_string().trim_matches(['\0', ' ']).to_string())
        .filter(|s| !s.is_empty())
}

fn probe(path: &Path) -> Result<symphonia::core::probe::ProbeResult, LoadError> {
    let file = std::fs::File::open(path).map_err(LoadError::Io)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| match e {
            SymError::IoError(io) if io.kind() != std::io::ErrorKind::UnexpectedEof => {
                LoadError::Io(io)
            }
            other => LoadError::Unsupported(other.to_string()),
        })
}

/// Everything the probe and the format reader know about the file's tags.
fn collect_tags(probed: &mut symphonia::core::probe::ProbeResult) -> Vec<Tag> {
    let mut tags: Vec<Tag> = probed
        .metadata
        .get()
        .and_then(|m| m.current().map(|r| r.tags().to_vec()))
        .unwrap_or_default();
    if let Some(rev) = probed.format.metadata().current() {
        tags.extend_from_slice(rev.tags());
    }
    tags
}

fn tags_from(tags: &[Tag]) -> Tags {
    Tags {
        title: tag_value(tags, StandardTagKey::TrackTitle),
        artist: tag_value(tags, StandardTagKey::Artist),
        genre: tag_value(tags, StandardTagKey::Genre),
    }
}

/// Probe the container, read the metadata the probe produced, and stop. No decoder is built
/// and no packet is read, so this costs a header read rather than a whole file.
pub(crate) fn tags(path: &Path) -> Result<Tags, LoadError> {
    let mut probed = probe(path)?;
    Ok(tags_from(&collect_tags(&mut probed)))
}

pub(crate) fn decode(path: &Path) -> Result<Decoded, LoadError> {
    let mut probed = probe(path)?;
    let tags = collect_tags(&mut probed);
    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| LoadError::Unsupported("no audio track".into()))?;
    let track_id = track.id;
    let mut sample_rate = track.codec_params.sample_rate.unwrap_or(0);
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| LoadError::Unsupported(e.to_string()))?;

    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut buf: Option<SampleBuffer<f32>> = None;
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymError::ResetRequired) => break,
            Err(e) => return Err(LoadError::Decode(e.to_string())),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let audio = match decoder.decode(&packet) {
            Ok(a) => a,
            // A corrupt frame is skipped rather than failing the whole track.
            Err(SymError::DecodeError(_)) => continue,
            Err(e) => return Err(LoadError::Decode(e.to_string())),
        };
        let spec = *audio.spec();
        sample_rate = spec.rate;
        let ch = spec.channels.count().max(1);
        let sb = match &mut buf {
            Some(b) if b.capacity() >= audio.capacity() * ch => b,
            _ => buf.insert(SampleBuffer::new(audio.capacity() as u64, spec)),
        };
        sb.copy_interleaved_ref(audio);
        for frame in sb.samples().chunks_exact(ch) {
            left.push(frame[0]);
            right.push(if ch > 1 { frame[1] } else { frame[0] });
        }
    }

    if sample_rate == 0 {
        return Err(LoadError::Unsupported("unknown sample rate".into()));
    }
    let Tags {
        title,
        artist,
        genre,
    } = tags_from(&tags);
    Ok(Decoded {
        channels: [left, right],
        sample_rate,
        title,
        artist,
        genre,
    })
}
