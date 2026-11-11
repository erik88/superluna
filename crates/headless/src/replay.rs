//! Replays: the inputs that play through a level, plus state hashes that prove the run is
//! reproduced exactly.
//!
//! Text format (one directive per line, `#` starts a comment):
//!
//! ```text
//! # replay v1
//! level meadow-1
//! seed 0
//! inputs 30*R 12*RXJ -
//! inputs 5*LJ
//! check 60 1f3a...      (frame, state hash after that many frames)
//! end won 734           (final status and frame)
//! ```
//!
//! Inputs are run-length tokens `N*BUTTONS` (or just `BUTTONS` for one frame), buttons from
//! `L R U D J X`, `-` for none.

use crate::Content;
use ns_core::{Buttons, Game, Status};
use std::fmt::Write;

/// A state hash is recorded every this many frames.
pub const CHECKPOINT_EVERY: u64 = 60;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Replay {
    pub level: String,
    pub seed: u64,
    pub inputs: Vec<Buttons>,
    pub checkpoints: Vec<(u64, u64)>,
    pub end_status: Status,
    pub end_frame: u64,
    /// The first script error seen while recording (not part of the text format).
    pub first_script_error: Option<String>,
}

#[derive(Debug)]
pub struct ReplayCheck {
    pub ok: bool,
    pub problems: Vec<String>,
    pub status: Status,
    pub frames: u64,
}

/// Parse run-length input tokens, e.g. `"30*R 12*RXJ -"`.
pub fn parse_inputs(s: &str) -> Result<Vec<Buttons>, String> {
    let mut out = Vec::new();
    for tok in s.split_whitespace() {
        let (n, b) = match tok.split_once('*') {
            Some((n, b)) => (
                n.parse::<usize>()
                    .map_err(|_| format!("bad count in '{tok}'"))?,
                b,
            ),
            None => (1, tok),
        };
        if n > 1_000_000 {
            return Err(format!("count too large in '{tok}'"));
        }
        out.extend(std::iter::repeat_n(Buttons::parse(b)?, n));
    }
    Ok(out)
}

fn format_inputs(inputs: &[Buttons]) -> Vec<String> {
    let mut toks = Vec::new();
    let mut i = 0;
    while i < inputs.len() {
        let b = inputs[i];
        let n = inputs[i..].iter().take_while(|x| **x == b).count();
        toks.push(if n == 1 {
            b.to_string()
        } else {
            format!("{n}*{b}")
        });
        i += n;
    }
    toks.chunks(12).map(|c| c.join(" ")).collect()
}

fn status_name(s: Status) -> &'static str {
    match s {
        Status::Playing => "playing",
        Status::Won => "won",
        Status::Dead => "dead",
    }
}

impl Replay {
    /// Play `inputs` from the start of `level` and record checkpoints. Stops early if the level
    /// ends (won or dead); remaining inputs are dropped.
    pub fn record(
        content: &Content,
        level: &str,
        seed: u64,
        inputs: &[Buttons],
    ) -> Result<Replay, String> {
        let mut game = Game::new(
            content.level(level)?,
            content.feel.clone(),
            seed,
            &content.scripts,
        );
        let mut checkpoints = Vec::new();
        let mut used = 0;
        let mut first_script_error = None;
        for b in inputs {
            if game.state.status != Status::Playing {
                break;
            }
            game.step(*b, &content.scripts);
            used += 1;
            if first_script_error.is_none() {
                first_script_error = game.state.script_errors.first().cloned();
            }
            if game.state.frame.is_multiple_of(CHECKPOINT_EVERY) {
                checkpoints.push((game.state.frame, game.hash()));
            }
        }
        if checkpoints.last().map(|c| c.0) != Some(game.state.frame) {
            checkpoints.push((game.state.frame, game.hash()));
        }
        Ok(Replay {
            level: level.to_string(),
            seed,
            inputs: inputs[..used].to_vec(),
            checkpoints,
            end_status: game.state.status,
            end_frame: game.state.frame,
            first_script_error,
        })
    }

    pub fn to_text(&self) -> String {
        let mut s = String::from("# replay v1\n");
        let _ = writeln!(s, "level {}", self.level);
        let _ = writeln!(s, "seed {}", self.seed);
        for line in format_inputs(&self.inputs) {
            let _ = writeln!(s, "inputs {line}");
        }
        for (f, h) in &self.checkpoints {
            let _ = writeln!(s, "check {f} {h:016x}");
        }
        let _ = writeln!(s, "end {} {}", status_name(self.end_status), self.end_frame);
        s
    }

    pub fn parse(text: &str) -> Result<Replay, String> {
        let mut level = None;
        let mut seed = 0;
        let mut inputs = String::new();
        let mut checkpoints = Vec::new();
        let mut end = None;
        for (n, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let err = |m: &str| format!("replay line {}: {m}", n + 1);
            let (key, rest) = line.split_once(' ').unwrap_or((line, ""));
            match key {
                "level" => level = Some(rest.trim().to_string()),
                "seed" => seed = rest.trim().parse().map_err(|_| err("bad seed"))?,
                "inputs" => {
                    inputs.push(' ');
                    inputs.push_str(rest);
                }
                "check" => {
                    let (f, h) = rest
                        .trim()
                        .split_once(' ')
                        .ok_or_else(|| err("expected 'check FRAME HASH'"))?;
                    checkpoints.push((
                        f.parse().map_err(|_| err("bad frame"))?,
                        u64::from_str_radix(h.trim(), 16).map_err(|_| err("bad hash"))?,
                    ));
                }
                "end" => {
                    let (st, f) = rest
                        .trim()
                        .split_once(' ')
                        .ok_or_else(|| err("expected 'end STATUS FRAME'"))?;
                    let st = match st {
                        "won" => Status::Won,
                        "dead" => Status::Dead,
                        "playing" => Status::Playing,
                        _ => return Err(err("bad status")),
                    };
                    end = Some((st, f.trim().parse().map_err(|_| err("bad frame"))?));
                }
                _ => return Err(err(&format!("unknown directive '{key}'"))),
            }
        }
        let (end_status, end_frame) = end.ok_or("replay has no 'end' line")?;
        Ok(Replay {
            level: level.ok_or("replay has no 'level' line")?,
            seed,
            inputs: parse_inputs(&inputs)?,
            checkpoints,
            end_status,
            end_frame,
            first_script_error: None,
        })
    }

    /// Re-run the replay and compare every checkpoint and the ending.
    pub fn check(&self, content: &Content) -> ReplayCheck {
        let fresh = match Replay::record(content, &self.level, self.seed, &self.inputs) {
            Ok(r) => r,
            Err(e) => {
                return ReplayCheck {
                    ok: false,
                    problems: vec![e],
                    status: Status::Playing,
                    frames: 0,
                };
            }
        };
        let mut problems = Vec::new();
        if let Some(e) = &fresh.first_script_error {
            problems.push(format!("script error: {e}"));
        }
        for (f, h) in &self.checkpoints {
            match fresh.checkpoints.iter().find(|(ff, _)| ff == f) {
                Some((_, fh)) if fh == h => {}
                Some((_, fh)) => {
                    problems.push(format!(
                        "state differs at frame {f}: expected {h:016x}, got {fh:016x}"
                    ));
                    break; // later checkpoints will all differ too
                }
                None => problems.push(format!("run ended before checkpoint frame {f}")),
            }
        }
        if (fresh.end_status, fresh.end_frame) != (self.end_status, self.end_frame) {
            problems.push(format!(
                "expected to end {} at frame {}, ended {} at frame {}",
                status_name(self.end_status),
                self.end_frame,
                status_name(fresh.end_status),
                fresh.end_frame
            ));
        }
        ReplayCheck {
            ok: problems.is_empty(),
            problems,
            status: fresh.end_status,
            frames: fresh.end_frame,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inputs_round_trip() {
        let inputs = parse_inputs("30*R 2*RXJ - L 3*-").unwrap();
        assert_eq!(inputs.len(), 37);
        let text = format_inputs(&inputs).join(" ");
        assert_eq!(text, "30*R 2*RJX - L 3*-");
        assert_eq!(parse_inputs(&text).unwrap(), inputs);
    }
}
