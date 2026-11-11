//! Checks that `content/feel.ron` produces the measurable results in `content/feel_targets.ron`.

use ns_core::script_api::NoScripts;
use ns_core::{Buttons, Feel, Game, Level, SUB, TILE_PX};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
struct Targets {
    full_jump_height_px: (i32, i32),
    tap_jump_height_px: (i32, i32),
    run_jump_height_px: (i32, i32),
    frames_to_run_max: (i32, i32),
    run_jump_distance_px: (i32, i32),
    coyote_frames_work: u32,
    coyote_frames_fail: u32,
    buffered_jump_frames: u32,
}

fn feel() -> Arc<Feel> {
    Arc::new(Feel::from_ron(include_str!("../../../content/feel.ron")).unwrap())
}

fn targets() -> Targets {
    ron::from_str(include_str!("../../../content/feel_targets.ron")).unwrap()
}

/// A long flat floor; optionally with a ledge (floor ends) at `ledge_col`.
fn flat(ledge_col: Option<usize>) -> Arc<Level> {
    let w = 400;
    let mut rows = vec![".".repeat(w); 30];
    rows[27].replace_range(2..3, "P");
    let floor: String = (0..w)
        .map(|c| {
            if ledge_col.is_some_and(|l| c >= l) {
                '.'
            } else {
                '#'
            }
        })
        .collect();
    rows[28] = floor.clone();
    rows[29] = floor;
    let file = format!(
        "(id: \"flat\", name: \"Flat\", rows: [{}])",
        rows.iter()
            .map(|r| format!("{r:?}"))
            .collect::<Vec<_>>()
            .join(",")
    );
    Arc::new(Level::parse(&file, "{}").unwrap())
}

fn game(level: Arc<Level>) -> Game {
    let mut g = Game::new(level, feel(), 0, &NoScripts);
    // Settle onto the floor.
    for _ in 0..4 {
        g.step(Buttons::NONE, &NoScripts);
    }
    assert!(
        g.state.player.on_ground,
        "player should start on the ground"
    );
    g
}

fn b(s: &str) -> Buttons {
    Buttons::parse(s).unwrap()
}

/// Run `hold` for `frames`, return the highest the player's feet rose, in pixels.
fn jump_height(g: &mut Game, press_frames: u32, steer: &str) -> i32 {
    let ground = g.state.player.y;
    let mut best = ground;
    for i in 0..120 {
        let jump = if i < press_frames { "J" } else { "" };
        g.step(b(&format!("{steer}{jump}")), &NoScripts);
        best = best.min(g.state.player.y);
        if i > 2 && g.state.player.on_ground {
            break;
        }
    }
    (ground - best) / SUB
}

fn run_up(g: &mut Game) {
    for _ in 0..80 {
        g.step(b("RX"), &NoScripts);
    }
    assert_eq!(g.state.player.vx, feel().run_max);
}

fn in_range(name: &str, v: i32, (lo, hi): (i32, i32)) {
    assert!(lo <= v && v <= hi, "{name} = {v}, target {lo}..={hi}");
}

#[test]
fn full_jump_height() {
    let mut g = game(flat(None));
    in_range(
        "full jump",
        jump_height(&mut g, 200, ""),
        targets().full_jump_height_px,
    );
}

#[test]
fn tap_jump_height() {
    let mut g = game(flat(None));
    in_range(
        "tap jump",
        jump_height(&mut g, 1, ""),
        targets().tap_jump_height_px,
    );
}

#[test]
fn holding_longer_jumps_higher() {
    let heights: Vec<i32> = [1, 4, 8, 16, 200]
        .iter()
        .map(|&n| jump_height(&mut game(flat(None)), n, ""))
        .collect();
    assert!(heights.windows(2).all(|w| w[0] <= w[1]), "{heights:?}");
    assert!(heights[0] < heights[4]);
}

#[test]
fn run_jump_height() {
    let mut g = game(flat(None));
    run_up(&mut g);
    in_range(
        "run jump",
        jump_height(&mut g, 200, "RX"),
        targets().run_jump_height_px,
    );
}

#[test]
fn frames_to_run_max() {
    let mut g = game(flat(None));
    let max = feel().run_max;
    let mut frames = 0;
    while g.state.player.vx < max {
        g.step(b("RX"), &NoScripts);
        frames += 1;
        assert!(frames < 1000);
    }
    in_range("frames to run max", frames, targets().frames_to_run_max);
}

#[test]
fn run_jump_distance() {
    let mut g = game(flat(None));
    run_up(&mut g);
    let x0 = g.state.player.x;
    let mut i = 0;
    loop {
        g.step(b("RXJ"), &NoScripts);
        i += 1;
        if i > 2 && g.state.player.on_ground {
            break;
        }
    }
    in_range(
        "run jump distance",
        (g.state.player.x - x0) / SUB,
        targets().run_jump_distance_px,
    );
}

/// Walk off a ledge, wait `after` frames in the air, press jump: did it jump?
fn jump_after_ledge(after: u32) -> bool {
    let ledge = 10;
    let mut g = game(flat(Some(ledge)));
    let edge = (ledge as i32 * TILE_PX) * SUB;
    while g.state.player.on_ground {
        g.step(b("R"), &NoScripts);
        assert!(g.state.player.x < edge + 20 * SUB);
    }
    for _ in 1..after {
        g.step(b("R"), &NoScripts);
    }
    g.step(b("RJ"), &NoScripts);
    g.state.player.vy < 0
}

#[test]
fn coyote_time() {
    let t = targets();
    assert!(
        jump_after_ledge(1),
        "jumping on the first airborne frame works"
    );
    assert!(
        jump_after_ledge(t.coyote_frames_work),
        "late jump should still work"
    );
    assert!(
        !jump_after_ledge(t.coyote_frames_fail),
        "very late jump should not work"
    );
}

#[test]
fn buffered_jump() {
    let t = targets();
    let mut g = game(flat(None));
    // Jump, then on the way down press jump again shortly before landing.
    // First find the landing frame of a plain full jump.
    let mut probe = Game::with_state(g.level.clone(), g.feel.clone(), g.state.clone());
    let mut land = 0;
    for i in 0..200 {
        probe.step(if i == 0 { b("J") } else { Buttons::NONE }, &NoScripts);
        if i > 2 && probe.state.player.on_ground {
            land = i;
            break;
        }
    }
    let press = land - t.buffered_jump_frames;
    for i in 0..=land + 1 {
        let input = if i == 0 || i == press {
            b("J")
        } else {
            Buttons::NONE
        };
        g.step(input, &NoScripts);
    }
    assert!(
        g.state.player.vy < 0,
        "buffered press should jump right after landing"
    );
}

#[test]
fn deterministic() {
    let run = || {
        let mut g = game(flat(None));
        for i in 0..600u32 {
            let s = ["RX", "RXJ", "L", "-", "RJ"][(i / 37 % 5) as usize];
            g.step(b(s), &NoScripts);
        }
        g.hash()
    };
    assert_eq!(run(), run());
}
