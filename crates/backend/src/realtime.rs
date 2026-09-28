//! What scheduling the thread doing the audio actually got.
//!
//! A backend that owns its thread (ALSA) has to ask for realtime itself. A backend whose
//! callback runs on someone else's thread (JACK, and so PipeWire) cannot ask at all: the
//! server grants it, or does not, and the only thing dj-tui can do is look and report.
//!
//! Asking is expected to fail. Taking SCHED_FIFO needs `RLIMIT_RTPRIO`, which on most desktops
//! is granted to a group rather than to everyone, so a refusal is the ordinary case and never
//! a reason to stop playing.

/// Priority asked for on a thread we own. High enough to beat ordinary work, low enough to
/// stay out of the way of the kernel's own threads and of a sound server that wants 70 or more.
pub const AUDIO_PRIORITY: i32 = 10;

/// How a thread is scheduled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheduling {
    /// SCHED_FIFO at this priority: runs until it yields.
    Fifo(i32),
    /// SCHED_RR at this priority: realtime, but shares with its equals.
    RoundRobin(i32),
    /// Anything else, which in practice means SCHED_OTHER and no guarantee at all.
    Other,
}

impl Scheduling {
    pub fn is_realtime(&self) -> bool {
        matches!(self, Scheduling::Fifo(_) | Scheduling::RoundRobin(_))
    }

    pub fn priority(&self) -> Option<i32> {
        match self {
            Scheduling::Fifo(p) | Scheduling::RoundRobin(p) => Some(*p),
            Scheduling::Other => None,
        }
    }

    /// For the status line, where it sits beside the rate and the xrun count.
    pub fn label(&self) -> String {
        match self {
            Scheduling::Fifo(p) => format!("realtime FIFO {p}"),
            Scheduling::RoundRobin(p) => format!("realtime RR {p}"),
            Scheduling::Other => "not realtime".into(),
        }
    }

    /// Pack into a non-zero integer, so an audio thread can report through an atomic and zero
    /// still means "has not run yet".
    pub fn to_bits(&self) -> u32 {
        let (tag, prio) = match self {
            Scheduling::Other => (1u32, 0),
            Scheduling::Fifo(p) => (2, *p),
            Scheduling::RoundRobin(p) => (3, *p),
        };
        tag | ((prio.clamp(0, 0xFF) as u32) << 8)
    }

    pub fn from_bits(bits: u32) -> Option<Scheduling> {
        let prio = ((bits >> 8) & 0xFF) as i32;
        match bits & 0xFF {
            1 => Some(Scheduling::Other),
            2 => Some(Scheduling::Fifo(prio)),
            3 => Some(Scheduling::RoundRobin(prio)),
            _ => None,
        }
    }
}

/// How the calling thread is scheduled right now. Two cheap syscalls and no allocation, so it
/// is safe to call once from inside an audio callback.
pub fn scheduling() -> Scheduling {
    // SAFETY: both calls take the calling thread (0) and only read.
    let policy = unsafe { libc::sched_getscheduler(0) };
    let mut param: libc::sched_param = unsafe { std::mem::zeroed() };
    let got = unsafe { libc::sched_getparam(0, &mut param) };
    let prio = if got == 0 { param.sched_priority } else { 0 };
    match policy {
        libc::SCHED_FIFO => Scheduling::Fifo(prio),
        libc::SCHED_RR => Scheduling::RoundRobin(prio),
        _ => Scheduling::Other,
    }
}

/// Ask for SCHED_FIFO at `priority` on the calling thread. Returns what it ended up with, or
/// why it could not, which the caller reports rather than treats as fatal.
pub fn request_realtime(priority: i32) -> Result<Scheduling, String> {
    let param = libc::sched_param {
        sched_priority: priority,
    };
    // SAFETY: `param` outlives the call and 0 means the calling thread.
    let rc = unsafe { libc::sched_setscheduler(0, libc::SCHED_FIFO, &param) };
    if rc != 0 {
        let e = std::io::Error::last_os_error();
        return Err(match e.raw_os_error() {
            // The usual one: the user has no RLIMIT_RTPRIO to spend.
            Some(libc::EPERM) => {
                "not permitted to take realtime priority (see the audio group and \
                 /etc/security/limits.d)"
                    .to_string()
            }
            _ => format!("could not take realtime priority: {e}"),
        });
    }
    Ok(scheduling())
}
