//! Beam search over held-button "moves" to find a run that completes a level.
//!
//! Each move holds one button combination for a few frames. States are scored by progress
//! (rightward, with a little credit for height so walls get climbed), deduplicated on a coarse
//! key (position, speed, grounded, enemies left) and the best `beam` survive each round.

use ns_core::{Buttons, Game, SUB, State, Status};
use ns_headless::Content;
use std::collections::HashMap;

pub struct Options {
    pub beam: usize,
    pub frames_per_move: usize,
    pub max_moves: usize,
    pub seed: u64,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            beam: 400,
            frames_per_move: 6,
            max_moves: 600,
            seed: 0,
        }
    }
}

pub struct Found {
    pub inputs: Vec<Buttons>,
    pub explored: usize,
}

const MOVES: [&str; 10] = ["RX", "RXJ", "R", "RJ", "-", "J", "L", "LJ", "LX", "LXJ"];

fn score(s: &State) -> i64 {
    let p = &s.player;
    i64::from(p.x / SUB) * 4 - i64::from(p.y / SUB)
}

fn key(s: &State) -> (i32, i32, i32, i32, bool, usize) {
    let p = &s.player;
    (
        p.x / (4 * SUB),
        p.y / (4 * SUB),
        p.vx / 64,
        p.vy / 128,
        p.on_ground,
        s.entities.len(),
    )
}

pub fn search(content: &Content, level: &str, opt: &Options) -> Result<Found, String> {
    let level = content.level(level)?;
    let moves: Vec<Buttons> = MOVES.iter().map(|m| Buttons::parse(m).unwrap()).collect();
    let start = Game::new(
        level.clone(),
        content.feel.clone(),
        opt.seed,
        &content.scripts,
    );

    // Arena of (parent, move) so paths are shared rather than copied.
    let mut arena: Vec<(usize, usize)> = vec![(usize::MAX, usize::MAX)];
    let mut beam: Vec<(usize, State)> = vec![(0, start.state)];
    let mut explored = 0;
    let path = |arena: &[(usize, usize)], mut i: usize| {
        let mut ms = Vec::new();
        while arena[i].0 != usize::MAX {
            ms.push(arena[i].1);
            i = arena[i].0;
        }
        ms.reverse();
        ms
    };

    for _ in 0..opt.max_moves {
        let mut next: HashMap<_, (i64, usize, State)> = HashMap::new();
        for (node, state) in &beam {
            for (mi, &b) in moves.iter().enumerate() {
                let mut g = Game::with_state(level.clone(), content.feel.clone(), state.clone());
                for _ in 0..opt.frames_per_move {
                    g.step(b, &content.scripts);
                    if g.state.status != Status::Playing {
                        break;
                    }
                }
                explored += 1;
                match g.state.status {
                    Status::Dead => continue,
                    Status::Won => {
                        arena.push((*node, mi));
                        let mut inputs = Vec::new();
                        for m in path(&arena, arena.len() - 1) {
                            inputs.extend(std::iter::repeat_n(moves[m], opt.frames_per_move));
                        }
                        return Ok(Found { inputs, explored });
                    }
                    Status::Playing => {}
                }
                let sc = score(&g.state);
                let k = key(&g.state);
                if next.get(&k).is_none_or(|(old, _, _)| sc > *old) {
                    next.insert(k, (sc, *node, g.state));
                    // Remember which move got here; arena entry is created on survival below.
                    next.get_mut(&k).unwrap().1 = *node * MOVES.len() + mi;
                }
            }
        }
        if next.is_empty() {
            return Err(format!(
                "no surviving runs after exploring {explored} moves"
            ));
        }
        let mut ranked: Vec<_> = next.into_values().collect();
        // Sort by score, then by the encoded path step so ties are broken deterministically.
        ranked.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        ranked.truncate(opt.beam);
        beam = ranked
            .into_iter()
            .map(|(_, code, st)| {
                arena.push((code / MOVES.len(), code % MOVES.len()));
                (arena.len() - 1, st)
            })
            .collect();
    }
    Err(format!(
        "no completing run within {} moves ({explored} explored)",
        opt.max_moves
    ))
}
