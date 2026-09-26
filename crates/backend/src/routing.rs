/// Our output ports, in order: master left/right, headphone cue left/right.
pub const OUTPUT_PORTS: [&str; 4] = ["master_L", "master_R", "cue_L", "cue_R"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Routing {
    /// Master to physical outputs 1–2, cue to 3–4 when the card has them.
    Auto,
    /// Leave connections to the user (qjackctl, Helvum, a session manager).
    Off,
    Explicit {
        master: [String; 2],
        cue: Option<[String; 2]>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    /// (our full port name, their full port name)
    pub connections: Vec<(String, String)>,
    pub warnings: Vec<String>,
}

/// Decide connections for client `client` given the physical playback ports that exist.
pub fn plan_connections(client: &str, routing: &Routing, physical: &[String]) -> Plan {
    let ours = |i: usize| format!("{client}:{}", OUTPUT_PORTS[i]);
    let mut plan = Plan::default();
    match routing {
        Routing::Off => {}
        Routing::Auto => {
            if physical.len() < 2 {
                plan.warnings
                    .push("no stereo playback outputs found; connect dj-tui ports by hand".into());
                return plan;
            }
            for (i, port) in physical.iter().take(4).enumerate() {
                plan.connections.push((ours(i), port.clone()));
            }
            if physical.len() < 4 {
                plan.warnings.push(
                    "soundcard has only two outputs; headphone cue is not connected (split-mono mode arrives in M2)"
                        .into(),
                );
            }
        }
        Routing::Explicit { master, cue } => {
            let targets = master.iter().map(Some).chain(
                cue.iter()
                    .flat_map(|c| c.iter().map(Some))
                    .chain(std::iter::repeat(None)),
            );
            for (i, target) in targets.take(4).enumerate() {
                let Some(target) = target else { continue };
                if physical.iter().any(|p| p == target) {
                    plan.connections.push((ours(i), target.clone()));
                } else {
                    plan.warnings
                        .push(format!("configured port {target} does not exist"));
                }
            }
        }
    }
    plan
}
