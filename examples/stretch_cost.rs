//! How much of the audio thread's budget key lock uses, measured offline.
use engine::{channel, Command, DeckId, Engine, EngineProcessor, Track};
use std::sync::Arc;
use std::time::Instant;

const FS: u32 = 48_000;

fn track(secs: f32) -> Arc<Track> {
    let data = (0..(secs * FS as f32) as usize)
        .flat_map(|i| {
            let t = i as f32 / FS as f32;
            let s = 0.3 * (std::f32::consts::TAU * 110.0 * t).sin()
                + 0.2 * (std::f32::consts::TAU * 1700.0 * t).sin();
            [s, s]
        })
        .collect();
    Arc::new(Track::from_interleaved(data, FS))
}

fn run(key_lock: bool) -> f64 {
    let (mut h, mut p): (_, EngineProcessor) = channel(Engine::with_sample_rate(FS), 32);
    for d in [DeckId::A, DeckId::B] {
        h.send(Command::Load(d, track(30.0))).unwrap();
        h.send(Command::PlayPause(d)).unwrap();
        h.send(Command::SetRate(d, 1.08)).unwrap();
        h.send(Command::SetKeyLock(d, key_lock)).unwrap();
    }
    let (mut m, mut c) = (vec![0.0; 256 * 2], vec![0.0; 256 * 2]);
    let blocks = 20 * FS as usize / 256;
    let start = Instant::now();
    for _ in 0..blocks {
        p.process(&mut m, &mut c);
    }
    let elapsed = start.elapsed().as_secs_f64();
    elapsed / (blocks as f64 * 256.0 / FS as f64)
}

fn main() {
    println!("two decks, 256 frame blocks, rate 1.08");
    println!("  key lock off: {:.3}% of real time", run(false) * 100.0);
    println!("  key lock on:  {:.3}% of real time", run(true) * 100.0);
}
