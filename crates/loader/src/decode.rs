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
}

fn tag_value(tags: &[Tag], key: StandardTagKey) -> Option<String> {
    tags.iter()
        .find(|t| t.std_key == Some(key))
        .map(|t| t.value.to_string())
        .filter(|s| !s.trim().is_empty())
}

pub(crate) fn decode(path: &Path) -> Result<Decoded, LoadError> {
    let file = std::fs::File::open(path).map_err(LoadError::Io)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mut probed = symphonia::default::get_probe()
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
        })?;

    let mut tags: Vec<Tag> = probed
        .metadata
        .get()
        .and_then(|m| m.current().map(|r| r.tags().to_vec()))
        .unwrap_or_default();
    let mut format = probed.format;
    if let Some(rev) = format.metadata().current() {
        tags.extend_from_slice(rev.tags());
    }

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
    Ok(Decoded {
        channels: [left, right],
        sample_rate,
        title: tag_value(&tags, StandardTagKey::TrackTitle),
        artist: tag_value(&tags, StandardTagKey::Artist),
    })
}
