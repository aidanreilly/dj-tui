use crate::LoadError;
use rubato::{FftFixedIn, Resampler};

const CHUNK: usize = 1024;

/// Offline resample of planar stereo. Output is trimmed for the resampler's delay so
/// timing is preserved, and sized to exactly `len * to / from` frames.
pub(crate) fn resample(
    input: [Vec<f32>; 2],
    from: u32,
    to: u32,
) -> Result<[Vec<f32>; 2], LoadError> {
    let err = |e: &dyn std::fmt::Display| LoadError::Decode(format!("resampler: {e}"));
    let mut r =
        FftFixedIn::<f32>::new(from as usize, to as usize, CHUNK, 2, 2).map_err(|e| err(&e))?;
    let n = input[0].len();
    let expected = (n as f64 * to as f64 / from as f64).round() as usize;
    let delay = r.output_delay();
    let mut out = [
        Vec::with_capacity(expected + delay + CHUNK * 2),
        Vec::with_capacity(expected + delay + CHUNK * 2),
    ];
    let push = |o: Vec<Vec<f32>>, out: &mut [Vec<f32>; 2]| {
        out[0].extend_from_slice(&o[0]);
        out[1].extend_from_slice(&o[1]);
    };

    let mut pos = 0;
    while pos + r.input_frames_next() <= n {
        let need = r.input_frames_next();
        let chunk = [&input[0][pos..pos + need], &input[1][pos..pos + need]];
        push(r.process(&chunk, None).map_err(|e| err(&e))?, &mut out);
        pos += need;
    }
    if pos < n {
        let chunk = [&input[0][pos..], &input[1][pos..]];
        push(
            r.process_partial(Some(&chunk), None).map_err(|e| err(&e))?,
            &mut out,
        );
    }
    while out[0].len() < expected + delay {
        let o = r
            .process_partial(None::<&[&[f32]]>, None)
            .map_err(|e| err(&e))?;
        if o[0].is_empty() {
            break;
        }
        push(o, &mut out);
    }

    for ch in &mut out {
        ch.drain(..delay.min(ch.len()));
        ch.resize(expected, 0.0);
    }
    Ok(out)
}
