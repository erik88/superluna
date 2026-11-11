//! `ns`: headless command line for agents and CI.
//!
//! ```text
//! ns serve  [--content DIR]                  JSON-lines protocol on stdin/stdout (see protocol.rs)
//! ns check  [--content DIR]                  every level must have a passing replay
//! ns replay FILE [--content DIR]             verify one replay
//! ns record LEVEL INPUTS [--seed N] [--content DIR] [--out FILE]
//!                                            play run-length inputs, print or write the replay
//! ns view   LEVEL [INPUTS] [--content DIR]   print the screen as text after INPUTS
//! ```

use ns_core::{Game, Status};
use ns_headless::{Content, Replay, Session, ascii_view, parse_inputs};
use std::io::{BufRead, Write};
use std::process::ExitCode;

struct Args {
    positional: Vec<String>,
    content: String,
    seed: u64,
    out: Option<String>,
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args {
        positional: Vec::new(),
        content: "content".into(),
        seed: 0,
        out: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(x) = it.next() {
        match x.as_str() {
            "--content" => a.content = it.next().ok_or("--content needs a directory")?,
            "--seed" => {
                a.seed = it
                    .next()
                    .ok_or("--seed needs a number")?
                    .parse()
                    .map_err(|_| "bad --seed")?
            }
            "--out" => a.out = Some(it.next().ok_or("--out needs a file")?),
            "-h" | "--help" => {
                a.positional = vec!["help".into()];
                return Ok(a);
            }
            _ => a.positional.push(x),
        }
    }
    Ok(a)
}

fn run() -> Result<bool, String> {
    let args = parse_args()?;
    let pos: Vec<&str> = args.positional.iter().map(String::as_str).collect();
    let usage = include_str!("ns.rs")
        .lines()
        .skip(3)
        .take(8)
        .map(|l| l.trim_start_matches("//! "))
        .collect::<Vec<_>>()
        .join("\n");
    let Some(&cmd) = pos.first() else {
        eprintln!("{usage}");
        return Ok(false);
    };
    if cmd == "help" {
        println!("{usage}");
        return Ok(true);
    }
    let content = Content::load(&args.content)?;
    match (cmd, &pos[1..]) {
        ("serve", []) => {
            let mut session = Session::new(&content);
            let stdout = std::io::stdout();
            for line in std::io::stdin().lock().lines() {
                let line = line.map_err(|e| e.to_string())?;
                if line.trim().is_empty() {
                    continue;
                }
                let resp = session.handle_line(&line);
                let mut out = stdout.lock();
                writeln!(out, "{resp}").map_err(|e| e.to_string())?;
                out.flush().map_err(|e| e.to_string())?;
            }
            Ok(true)
        }
        ("check", []) => {
            let mut all_ok = true;
            for id in content.levels.keys() {
                let path = content.replay_path(id);
                let result = std::fs::read_to_string(&path)
                    .map_err(|e| format!("missing replay {}: {e}", path.display()))
                    .and_then(|t| Replay::parse(&t));
                match result {
                    Err(e) => {
                        all_ok = false;
                        println!("FAIL {id}: {e}");
                    }
                    Ok(r) => {
                        let c = r.check(&content);
                        let won = c.status == Status::Won;
                        if c.ok && won {
                            println!("ok   {id}: completed in {} frames", c.frames);
                        } else {
                            all_ok = false;
                            let mut why = c.problems;
                            if !won {
                                why.push("replay does not complete the level".into());
                            }
                            println!("FAIL {id}: {}", why.join("; "));
                        }
                    }
                }
            }
            Ok(all_ok)
        }
        ("replay", [file]) => {
            let r =
                Replay::parse(&std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?)?;
            let c = r.check(&content);
            println!(
                "{}: {:?} after {} frames",
                if c.ok { "ok" } else { "FAIL" },
                c.status,
                c.frames
            );
            for p in &c.problems {
                println!("  {p}");
            }
            Ok(c.ok)
        }
        ("record", [level, inputs]) => {
            let r = Replay::record(&content, level, args.seed, &parse_inputs(inputs)?)?;
            match &args.out {
                Some(path) => {
                    std::fs::write(path, r.to_text()).map_err(|e| format!("{path}: {e}"))?;
                    eprintln!("{:?} at frame {}; wrote {path}", r.end_status, r.end_frame);
                }
                None => print!("{}", r.to_text()),
            }
            Ok(true)
        }
        ("view", [level, rest @ ..]) if rest.len() <= 1 => {
            let mut g = Game::new(
                content.level(level)?,
                content.feel.clone(),
                args.seed,
                &content.scripts,
            );
            for b in parse_inputs(rest.first().copied().unwrap_or(""))? {
                g.step(b, &content.scripts);
            }
            for row in ascii_view(&g) {
                println!("{row}");
            }
            println!("frame {} {:?}", g.state.frame, g.state.status);
            Ok(true)
        }
        _ => Err(format!("unknown command or wrong arguments\n\n{usage}")),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}
