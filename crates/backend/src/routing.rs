use engine::OutputMode;

/// Our four output ports, in order. The first pair is the master in `Mix` and deck A in
/// `Decks`; the second is the headphone cue, or deck B.
pub fn output_ports(mode: OutputMode) -> [&'static str; 4] {
    match mode {
        OutputMode::Mix => ["master_L", "master_R", "cue_L", "cue_R"],
        OutputMode::Decks => ["deck_a_L", "deck_a_R", "deck_b_L", "deck_b_R"],
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Routing {
    /// Master to physical outputs 1–2, cue to 3–4 when the card has them.
    Auto,
    /// Leave connections to the user (qjackctl, Helvum, a session manager).
    Off,
    /// For a stereo-only card and a Y-splitter: mono master on output 1, mono cue on 2.
    /// The renderer carries both on the `master_L`/`master_R` ports in this mode.
    Split,
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
pub fn plan_connections(
    client: &str,
    routing: &Routing,
    physical: &[String],
    mode: OutputMode,
) -> Plan {
    let names = output_ports(mode);
    let ours = |i: usize| format!("{client}:{}", names[i]);
    let mut plan = Plan::default();
    match routing {
        Routing::Off => {}
        Routing::Split => {
            if physical.len() < 2 {
                plan.warnings
                    .push("split mode needs two playback outputs; none found".into());
                return plan;
            }
            plan.connections.push((ours(0), physical[0].clone()));
            plan.connections.push((ours(1), physical[1].clone()));
        }
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
                plan.warnings.push(match mode {
                    OutputMode::Mix => "soundcard has only two outputs, so headphone cue is not connected; set routing = \"split\" for mono master left and mono cue right".to_string(),
                    OutputMode::Decks => "soundcard has only two outputs, so deck B is not connected; connect it by hand or use output = \"mix\"".to_string(),
                });
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
            if mode == OutputMode::Decks && cue.is_none() {
                plan.warnings.push(
                    "output = \"decks\" needs a second pair: deck B is not connected, so set cue_ports to its outputs"
                        .into(),
                );
            }
        }
    }
    plan
}
