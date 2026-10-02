//! Which JACK ports our four outputs connect to (spec 3.6, routing mode 1).

use backend::{output_ports, plan_connections, Routing};
use engine::OutputMode;

fn physical(n: usize) -> Vec<String> {
    (1..=n).map(|i| format!("system:playback_{i}")).collect()
}

fn pairs(plan: &backend::Plan) -> Vec<(&str, &str)> {
    plan.connections
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect()
}

#[test]
fn mix_mode_port_names_match_the_spec() {
    assert_eq!(
        output_ports(OutputMode::Mix),
        ["master_L", "master_R", "cue_L", "cue_R"]
    );
}

#[test]
fn deck_mode_names_the_ports_after_the_decks() {
    assert_eq!(
        output_ports(OutputMode::Decks),
        ["deck_a_L", "deck_a_R", "deck_b_L", "deck_b_R"]
    );
}

#[test]
fn auto_routing_sends_deck_a_to_one_two_and_deck_b_to_three_four() {
    let plan = plan_connections("dj-tui", &Routing::Auto, &physical(4), OutputMode::Decks);
    assert_eq!(
        pairs(&plan),
        vec![
            ("dj-tui:deck_a_L", "system:playback_1"),
            ("dj-tui:deck_a_R", "system:playback_2"),
            ("dj-tui:deck_b_L", "system:playback_3"),
            ("dj-tui:deck_b_R", "system:playback_4"),
        ]
    );
    assert!(plan.warnings.is_empty(), "{:?}", plan.warnings);
}

#[test]
fn deck_mode_warns_when_the_card_has_only_two_outputs() {
    let plan = plan_connections("dj-tui", &Routing::Auto, &physical(2), OutputMode::Decks);
    assert_eq!(
        pairs(&plan),
        vec![
            ("dj-tui:deck_a_L", "system:playback_1"),
            ("dj-tui:deck_a_R", "system:playback_2"),
        ]
    );
    assert!(
        plan.warnings.iter().any(|w| w.contains("deck B")),
        "should name deck B: {:?}",
        plan.warnings
    );
}

#[test]
fn deck_mode_explicit_without_a_second_pair_warns_rather_than_doubling_up() {
    let plan = plan_connections(
        "dj-tui",
        &Routing::Explicit {
            master: ["system:playback_1".into(), "system:playback_2".into()],
            cue: None,
        },
        &physical(4),
        OutputMode::Decks,
    );
    assert_eq!(
        pairs(&plan),
        vec![
            ("dj-tui:deck_a_L", "system:playback_1"),
            ("dj-tui:deck_a_R", "system:playback_2"),
        ]
    );
    assert!(
        plan.warnings.iter().any(|w| w.contains("deck B")),
        "should name deck B: {:?}",
        plan.warnings
    );
}

#[test]
fn auto_routing_uses_outputs_one_to_four() {
    let plan = plan_connections("dj-tui", &Routing::Auto, &physical(4), OutputMode::Mix);
    assert_eq!(
        pairs(&plan),
        vec![
            ("dj-tui:master_L", "system:playback_1"),
            ("dj-tui:master_R", "system:playback_2"),
            ("dj-tui:cue_L", "system:playback_3"),
            ("dj-tui:cue_R", "system:playback_4"),
        ]
    );
    assert!(plan.warnings.is_empty());
}

#[test]
fn auto_routing_on_a_stereo_card_connects_master_and_warns_about_cue() {
    let plan = plan_connections("dj-tui", &Routing::Auto, &physical(2), OutputMode::Mix);
    assert_eq!(plan.connections.len(), 2);
    assert_eq!(plan.warnings.len(), 1);
    assert!(
        plan.warnings[0].contains("headphone"),
        "{:?}",
        plan.warnings
    );
}

#[test]
fn no_physical_outputs_means_no_connections() {
    let plan = plan_connections("dj-tui", &Routing::Auto, &[], OutputMode::Mix);
    assert!(plan.connections.is_empty());
    assert!(!plan.warnings.is_empty());
}

#[test]
fn explicit_routing_is_followed_and_missing_ports_are_reported() {
    let routing = Routing::Explicit {
        master: ["system:playback_3".into(), "system:playback_4".into()],
        cue: Some(["usb:out_1".into(), "usb:out_2".into()]),
    };
    let plan = plan_connections("dj-tui", &routing, &physical(4), OutputMode::Mix);
    assert_eq!(
        pairs(&plan),
        vec![
            ("dj-tui:master_L", "system:playback_3"),
            ("dj-tui:master_R", "system:playback_4")
        ]
    );
    assert_eq!(plan.warnings.len(), 2);
    assert!(plan.warnings[0].contains("usb:out_1"));
}

#[test]
fn explicit_routing_without_cue_leaves_cue_unconnected_silently() {
    let routing = Routing::Explicit {
        master: ["system:playback_1".into(), "system:playback_2".into()],
        cue: None,
    };
    let plan = plan_connections("x", &routing, &physical(4), OutputMode::Mix);
    assert_eq!(plan.connections.len(), 2);
    assert!(plan.warnings.is_empty());
}

#[test]
fn routing_off_connects_nothing() {
    let plan = plan_connections("x", &Routing::Off, &physical(4), OutputMode::Mix);
    assert!(plan.connections.is_empty() && plan.warnings.is_empty());
}

#[test]
fn split_mono_puts_master_left_and_cue_right_on_a_stereo_card() {
    let plan = plan_connections("dj-tui", &Routing::Split, &physical(2), OutputMode::Mix);
    assert_eq!(
        pairs(&plan),
        vec![
            ("dj-tui:master_L", "system:playback_1"),
            ("dj-tui:master_R", "system:playback_2")
        ]
    );
    assert!(plan.warnings.is_empty());
}

#[test]
fn split_mono_without_outputs_warns() {
    assert!(
        !plan_connections("x", &Routing::Split, &[], OutputMode::Mix)
            .warnings
            .is_empty()
    );
}

#[test]
fn auto_on_a_stereo_card_suggests_split_mode() {
    let plan = plan_connections("dj-tui", &Routing::Auto, &physical(2), OutputMode::Mix);
    assert!(
        plan.warnings[0].contains("routing = \"split\""),
        "{:?}",
        plan.warnings
    );
}
