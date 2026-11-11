//! `ns-bot LEVEL [--content DIR] [--beam N] [--seed N] [--write]`
//!
//! Finds inputs that complete LEVEL and prints the replay, or with `--write` saves it next to
//! the level as `LEVEL.replay` (what `ns check` verifies).

use ns_bot::{Options, search};
use ns_headless::{Content, Replay};
use std::process::ExitCode;
use std::time::Instant;

fn run() -> Result<(), String> {
    let mut level = None;
    let mut dir = "content".to_string();
    let mut write = false;
    let mut opt = Options::default();
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut num = |name: &str| -> Result<u64, String> {
            it.next()
                .and_then(|v| v.parse().ok())
                .ok_or(format!("{name} needs a number"))
        };
        match a.as_str() {
            "--content" => dir = it.next().ok_or("--content needs a directory")?,
            "--beam" => opt.beam = num("--beam")? as usize,
            "--seed" => opt.seed = num("--seed")?,
            "--write" => write = true,
            _ if level.is_none() => level = Some(a),
            _ => return Err(format!("unexpected argument '{a}'")),
        }
    }
    let level =
        level.ok_or("usage: ns-bot LEVEL [--content DIR] [--beam N] [--seed N] [--write]")?;
    let content = Content::load(&dir)?;
    let t = Instant::now();
    let found = search(&content, &level, &opt)?;
    let replay = Replay::record(&content, &level, opt.seed, &found.inputs)?;
    eprintln!(
        "{level}: {:?} in {} frames ({} moves explored, {:.1}s)",
        replay.end_status,
        replay.end_frame,
        found.explored,
        t.elapsed().as_secs_f32()
    );
    if write {
        let path = content.replay_path(&level);
        std::fs::write(&path, replay.to_text()).map_err(|e| format!("{}: {e}", path.display()))?;
        eprintln!("wrote {}", path.display());
    } else {
        print!("{}", replay.to_text());
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
