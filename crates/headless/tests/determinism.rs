//! Same inputs, same result: twice in a row, and across a save state written to JSON and back.

use ns_core::{Game, State};
use ns_headless::{Content, Replay};
use std::path::Path;

fn content() -> Content {
    Content::load(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content")).unwrap()
}

#[test]
fn replay_runs_identically_and_survives_save_states() {
    let content = content();
    let text = std::fs::read_to_string(content.replay_path("meadow-1")).unwrap();
    let replay = Replay::parse(&text).unwrap();
    let level = content.level(&replay.level).unwrap();

    let straight = |c: &Content| {
        let mut g = Game::new(level.clone(), c.feel.clone(), replay.seed, &c.scripts);
        for b in &replay.inputs {
            g.step(*b, &c.scripts);
        }
        g.hash()
    };
    let a = straight(&content);
    assert_eq!(a, straight(&content), "two runs differ");

    for split in [1, 137, replay.inputs.len() / 2, replay.inputs.len() - 1] {
        let mut g = Game::new(
            level.clone(),
            content.feel.clone(),
            replay.seed,
            &content.scripts,
        );
        for b in &replay.inputs[..split] {
            g.step(*b, &content.scripts);
        }
        let saved = serde_json::to_string(&g.state).unwrap();
        let state: State = serde_json::from_str(&saved).unwrap();
        let mut g = Game::with_state(level.clone(), content.feel.clone(), state);
        for b in &replay.inputs[split..] {
            g.step(*b, &content.scripts);
        }
        assert_eq!(
            g.hash(),
            a,
            "resuming from a save at frame {split} changed the outcome"
        );
    }
}
