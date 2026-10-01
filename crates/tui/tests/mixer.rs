//! The mixer's controls, drawn as rails rather than as filled blocks.
//!
//! Six rows of a bar filled from the left turned the strip into one slab, and a slab says
//! nothing about where a control is sitting. These check the reading each bar is meant to
//! give: a quantity grows from the point it is measured against, which is the centre for a
//! control with a neutral position and the left end for one that runs from nothing.

use tui::mixer::{centre_bar, eq_bar, fill_bar, position_bar, CAP, RAIL, RUN, TICK};

const W: usize = 7;

fn run_cells(s: &str) -> usize {
    s.chars().filter(|&c| c == RUN || c == CAP).count()
}

#[test]
fn a_control_at_its_neutral_point_is_all_rail_and_a_tick() {
    let bar = centre_bar(0.0, W);
    assert_eq!(bar.chars().count(), W);
    assert_eq!(bar.chars().nth(W / 2), Some(TICK), "{bar}");
    assert_eq!(run_cells(&bar), 0, "nothing is lit at neutral: {bar}");
    assert_eq!(bar.chars().filter(|&c| c == RAIL).count(), W - 1, "{bar}");
}

#[test]
fn a_cut_and_a_boost_of_the_same_size_mirror_each_other() {
    let up = centre_bar(0.6, W);
    let down = centre_bar(-0.6, W);
    assert_eq!(run_cells(&up), run_cells(&down), "{up} {down}");
    assert!(run_cells(&up) > 0, "{up}");
    // The lit run sits on the side the value is on, which is the whole point of the shape.
    let centre = W / 2;
    let lit = |s: &str| -> Vec<usize> {
        s.chars()
            .enumerate()
            .filter(|&(_, c)| c == RUN || c == CAP)
            .map(|(i, _)| i)
            .collect()
    };
    assert!(lit(&up).iter().all(|&i| i > centre), "{up}");
    assert!(lit(&down).iter().all(|&i| i < centre), "{down}");
}

#[test]
fn a_control_hard_over_lights_its_whole_side() {
    let bar = centre_bar(1.0, W);
    assert_eq!(run_cells(&bar), W / 2, "{bar}");
    assert_eq!(bar.chars().last(), Some(RUN), "reaches the end: {bar}");
}

#[test]
fn the_eq_puts_zero_db_on_the_tick_despite_its_lopsided_range() {
    // Cut runs to -26 dB and boost only to +6, so a single scale would leave 0 dB off
    // centre and a flat EQ looking like a cut.
    let flat = eq_bar(0.0, false, W);
    assert_eq!(run_cells(&flat), 0, "a flat band is unlit: {flat}");
    assert_eq!(flat.chars().nth(W / 2), Some(TICK), "{flat}");

    let full_boost = eq_bar(6.0, false, W);
    let full_cut = eq_bar(-26.0, false, W);
    assert_eq!(run_cells(&full_boost), W / 2, "{full_boost}");
    assert_eq!(run_cells(&full_cut), W / 2, "{full_cut}");

    // Half of each range lights about half of its side, so the two directions read alike.
    assert_eq!(
        run_cells(&eq_bar(3.0, false, W)),
        run_cells(&eq_bar(-13.0, false, W))
    );
}

#[test]
fn a_killed_band_says_so_instead_of_drawing_a_bar() {
    let bar = eq_bar(0.0, true, W);
    assert!(bar.contains("KILL"), "{bar}");
    assert_eq!(bar.chars().count(), W, "{bar}");
}

#[test]
fn a_fader_fills_from_the_left_and_ends_in_a_cap() {
    let full = fill_bar(1.0, W);
    assert_eq!(full.chars().count(), W);
    assert_eq!(full.chars().last(), Some(CAP), "{full}");
    assert!(!full.contains(RAIL), "{full}");

    let empty = fill_bar(0.0, W);
    assert_eq!(empty.chars().filter(|&c| c == RAIL).count(), W, "{empty}");

    let half = fill_bar(0.5, W);
    assert!(half.contains(RUN) && half.contains(RAIL), "{half}");
    assert!(run_cells(&half) < run_cells(&full), "{half} against {full}");
}

#[test]
fn values_outside_the_range_clamp_rather_than_overflow_the_cell() {
    for bar in [centre_bar(4.0, W), centre_bar(-4.0, W), fill_bar(9.0, W)] {
        assert_eq!(bar.chars().count(), W, "{bar}");
    }
}

#[test]
fn the_rail_is_solid_and_lighter_than_the_run() {
    // A control row reads as one continuous bar with a heavier stretch lit on it. A dotted
    // rail made the trim and EQ rows look like a different kind of control from a fader.
    // The two cannot be the same character either: that would leave every value drawing the
    // same string, with nothing but colour to say where the control sits.
    assert_ne!(RAIL, RUN, "lit and unlit have to be tellable apart");
    for c in [RAIL, RUN] {
        assert!(
            !"┄┈╌╍".contains(c),
            "{c} is dashed, which is what this moved away from"
        );
    }
}

#[test]
fn a_position_bar_puts_one_marker_on_an_unbroken_rail() {
    // For a control whose value is a place rather than an amount, the way the crossfader
    // already reads: one marker on the same heavy rail, and nothing filled behind it.
    let bar = position_bar(0.5, W);
    assert_eq!(bar.chars().count(), W);
    assert_eq!(bar.chars().filter(|&c| c == TICK).count(), 1, "{bar}");
    assert_eq!(bar.chars().nth(W / 2), Some(TICK), "{bar}");
    assert!(!bar.contains(CAP), "nothing is filled: {bar}");
    assert!(!bar.contains(RAIL), "it is the heavy rail: {bar}");
}

#[test]
fn a_position_bar_reaches_both_ends() {
    assert_eq!(position_bar(0.0, W).chars().next(), Some(TICK));
    assert_eq!(position_bar(1.0, W).chars().last(), Some(TICK));
    for v in [-1.0, 2.0] {
        assert_eq!(position_bar(v, W).chars().count(), W, "{v} clamps");
    }
}
