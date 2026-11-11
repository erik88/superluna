//! Built for `wasm32-unknown-unknown` with no imports, then run by `scripts/web-check.mjs`
//! under Node. It replays every embedded level replay and reports whether each one matches its
//! recorded checkpoints, which were produced by the desktop build.

use ns_headless::{Content, Replay};

const FEEL: &str = include_str!("../../../content/feel.ron");
const ENTITIES: &str = include_str!("../../../content/entities.ron");
const LEVELS: [&str; 1] = [include_str!("../../../content/levels/meadow-1.ron")];
const REPLAYS: [&str; 1] = [include_str!("../../../content/levels/meadow-1.replay")];
const SCRIPTS: [(&str, &str); 2] = [
    (
        "walker",
        include_str!("../../../content/scripts/walker.rhai"),
    ),
    (
        "hopper",
        include_str!("../../../content/scripts/hopper.rhai"),
    ),
];

/// Number of embedded replays that pass, or -1 if the content fails to load.
#[unsafe(no_mangle)]
pub extern "C" fn passing_replays() -> i32 {
    let Ok(content) = Content::from_sources(FEEL, ENTITIES, &LEVELS, &SCRIPTS) else {
        return -1;
    };
    REPLAYS
        .iter()
        .filter(|text| Replay::parse(text).is_ok_and(|r| r.check(&content).ok))
        .count() as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn total_replays() -> i32 {
    REPLAYS.len() as i32
}

#[cfg(test)]
mod tests {
    #[test]
    fn natively_all_pass() {
        assert_eq!(super::passing_replays(), super::total_replays());
    }
}
