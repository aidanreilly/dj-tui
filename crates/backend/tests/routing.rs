//! Which JACK ports our four outputs connect to (spec 3.6, routing mode 1).

use backend::{plan_connections, Routing, OUTPUT_PORTS};

fn physical(n: usize) -> Vec<String> {
    (1..=n).map(|i| format!("system:playback_{i}")).collect()
}

fn pairs(plan: &backend::Plan) -> Vec<(&str, &str)> {
    plan.connections.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect()
}

#[test]
fn our_port_names_match_the_spec() {
    assert_eq!(OUTPUT_PORTS, ["master_L", "master_R", "cue_L", "cue_R"]);
}

#[test]
fn auto_routing_uses_outputs_one_to_four() {
    let plan = plan_connections("dj-tui", &Routing::Auto, &physical(4));
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
    let plan = plan_connections("dj-tui", &Routing::Auto, &physical(2));
    assert_eq!(plan.connections.len(), 2);
    assert_eq!(plan.warnings.len(), 1);
    assert!(plan.warnings[0].contains("headphone"), "{:?}", plan.warnings);
}

#[test]
fn no_physical_outputs_means_no_connections() {
    let plan = plan_connections("dj-tui", &Routing::Auto, &[]);
    assert!(plan.connections.is_empty());
    assert!(!plan.warnings.is_empty());
}

#[test]
fn explicit_routing_is_followed_and_missing_ports_are_reported() {
    let routing = Routing::Explicit {
        master: ["system:playback_3".into(), "system:playback_4".into()],
        cue: Some(["usb:out_1".into(), "usb:out_2".into()]),
    };
    let plan = plan_connections("dj-tui", &routing, &physical(4));
    assert_eq!(
        pairs(&plan),
        vec![("dj-tui:master_L", "system:playback_3"), ("dj-tui:master_R", "system:playback_4")]
    );
    assert_eq!(plan.warnings.len(), 2);
    assert!(plan.warnings[0].contains("usb:out_1"));
}

#[test]
fn explicit_routing_without_cue_leaves_cue_unconnected_silently() {
    let routing = Routing::Explicit { master: ["system:playback_1".into(), "system:playback_2".into()], cue: None };
    let plan = plan_connections("x", &routing, &physical(4));
    assert_eq!(plan.connections.len(), 2);
    assert!(plan.warnings.is_empty());
}

#[test]
fn routing_off_connects_nothing() {
    let plan = plan_connections("x", &Routing::Off, &physical(4));
    assert!(plan.connections.is_empty() && plan.warnings.is_empty());
}
