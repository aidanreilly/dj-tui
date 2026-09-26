//! One binary file per analysed source track, under the XDG cache directory.
//!
//! Every failure here is survivable: a miss costs an analysis pass and nothing more,
//! so no function in this module returns an error or panics on bad input.

use crate::WavePoint;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const MAGIC: [u8; 4] = *b"DJTW";
const FORMAT_VERSION: u16 = 1;

/// Bumped whenever filter corners, normalisation or `POINTS_PER_SECOND` change, which
/// retires every file already written without deleting one.
pub const ANALYSIS_VERSION: u16 = 1;

/// Bytes before the path, and bytes per point.
const HEADER: usize = 34;
const POINT: usize = 20;

/// `$XDG_CACHE_HOME/dj-tui/analysis`, or `$HOME/.cache/dj-tui/analysis`.
pub fn dir(xdg: Option<&str>, home: Option<&str>) -> Option<PathBuf> {
    let base = match (xdg, home) {
        (Some(x), _) if !x.is_empty() => PathBuf::from(x),
        (_, Some(h)) if !h.is_empty() => PathBuf::from(h).join(".cache"),
        _ => return None,
    };
    Some(base.join("dj-tui").join("analysis"))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Where `source`'s analysis is kept. The hash only spreads files out; the header
/// carries the path that decides whether a file is the right one.
pub fn path_for(dir: &Path, source: &Path) -> PathBuf {
    let key = source.to_string_lossy();
    dir.join(format!("{:016x}.wave", fnv1a64(key.as_bytes())))
}

/// Source mtime as seconds and nanoseconds, plus its length.
fn stamp(source: &Path) -> Option<(u64, u32, u64)> {
    let meta = std::fs::metadata(source).ok()?;
    let mtime = meta.modified().ok()?;
    let since = mtime.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some((since.as_secs(), since.subsec_nanos(), meta.len()))
}

/// The cached analysis for `source`, or `None` for anything at all unexpected.
pub fn read(dir: &Path, source: &Path) -> Option<Vec<WavePoint>> {
    let path = source.to_str()?;
    let (secs, nanos, size) = stamp(source)?;
    let bytes = std::fs::read(path_for(dir, source)).ok()?;
    if bytes.len() < HEADER + 4 || bytes[0..4] != MAGIC {
        return None;
    }
    let u16_at = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]);
    let u32_at = |i: usize| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
    let u64_at = |i: usize| u64::from_le_bytes(bytes[i..i + 8].try_into().unwrap());

    if u16_at(4) != FORMAT_VERSION || u16_at(6) != ANALYSIS_VERSION {
        return None;
    }
    if u64_at(8) != secs || u32_at(16) != nanos || u64_at(20) != size {
        return None;
    }
    let count = u32_at(28) as usize;
    let path_len = u16_at(32) as usize;
    let payload_end = HEADER + path_len + count * POINT;
    if bytes.len() != payload_end + 4 {
        return None;
    }
    if &bytes[HEADER..HEADER + path_len] != path.as_bytes() {
        return None;
    }
    if u32_at(payload_end) != crc32(&bytes[..payload_end]) {
        return None;
    }

    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let at = HEADER + path_len + i * POINT;
        let f =
            |k: usize| f32::from_le_bytes(bytes[at + k * 4..at + k * 4 + 4].try_into().unwrap());
        out.push(WavePoint {
            range: [f(0), f(1)],
            bands: [f(2), f(3), f(4)],
        });
    }
    Some(out)
}

/// Store `points` for `source`, doing nothing at all if that is not possible.
pub fn write(dir: &Path, source: &Path, points: &[WavePoint]) {
    let Some(path) = source.to_str() else { return };
    let Some((secs, nanos, size)) = stamp(source) else {
        return;
    };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }

    let mut buf = Vec::with_capacity(HEADER + path.len() + points.len() * POINT + 4);
    buf.extend_from_slice(&MAGIC);
    buf.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    buf.extend_from_slice(&ANALYSIS_VERSION.to_le_bytes());
    buf.extend_from_slice(&secs.to_le_bytes());
    buf.extend_from_slice(&nanos.to_le_bytes());
    buf.extend_from_slice(&size.to_le_bytes());
    buf.extend_from_slice(&(points.len() as u32).to_le_bytes());
    buf.extend_from_slice(&(path.len() as u16).to_le_bytes());
    debug_assert_eq!(buf.len(), HEADER);
    buf.extend_from_slice(path.as_bytes());
    for p in points {
        for v in [p.range[0], p.range[1], p.bands[0], p.bands[1], p.bands[2]] {
            buf.extend_from_slice(&v.to_le_bytes());
        }
    }
    let sum = crc32(&buf);
    buf.extend_from_slice(&sum.to_le_bytes());

    // A counter alongside the pid keeps two decks loading the same file from picking
    // the same temporary name.
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let final_path = path_for(dir, source);
    let temp = final_path.with_extension(format!(
        "tmp{}.{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    if std::fs::write(&temp, &buf).is_err() {
        let _ = std::fs::remove_file(&temp);
        return;
    }
    if std::fs::rename(&temp, &final_path).is_err() {
        let _ = std::fs::remove_file(&temp);
    }
}
