//! JSON-lines protocol for agents: one request object per line in, one response per line out.
//!
//! Requests (`cmd` plus arguments):
//! - `{"cmd":"levels"}`: list level ids.
//! - `{"cmd":"load","level":"meadow-1","seed":0}`: start a level (seed optional).
//! - `{"cmd":"reset"}`: restart the current level.
//! - `{"cmd":"step","buttons":"RX","frames":10}` or `{"cmd":"step","inputs":"30*R 10*RJ"}`:
//!   advance; stops early when the level ends.
//! - `{"cmd":"observe","ascii":true}`: the current state (`ascii` adds a text view of the screen).
//! - `{"cmd":"save"}` -> `{"slot":N}`; `{"cmd":"restore","slot":N}`: save states.
//! - `{"cmd":"replay"}`: the inputs so far as replay text (with checkpoints).
//! - `{"cmd":"check_replay","text":"..."}`: verify replay text.
//! - `{"cmd":"feel"}`: the feel spec in use.
//!
//! Responses are `{"ok":true,...}` or `{"ok":false,"error":"..."}`. `step`, `load`, `reset` and
//! `restore` include an observation; add `"ascii":true` to any of them for the text view.

use crate::{Content, Replay, observe, parse_inputs};
use ns_core::{Buttons, Game, State, Status};
use serde_json::{Value, json};

pub struct Session<'a> {
    content: &'a Content,
    game: Option<Game>,
    seed: u64,
    /// Inputs since the level started.
    recording: Vec<Buttons>,
    saves: Vec<(State, usize)>,
}

impl<'a> Session<'a> {
    pub fn new(content: &'a Content) -> Session<'a> {
        Session {
            content,
            game: None,
            seed: 0,
            recording: Vec::new(),
            saves: Vec::new(),
        }
    }

    pub fn game(&self) -> Option<&Game> {
        self.game.as_ref()
    }

    /// Handle one request line and return the response.
    pub fn handle_line(&mut self, line: &str) -> Value {
        let req: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => return json!({"ok": false, "error": format!("invalid JSON: {e}")}),
        };
        match self.handle(&req) {
            Ok(mut v) => {
                v["ok"] = json!(true);
                v
            }
            Err(e) => json!({"ok": false, "error": e}),
        }
    }

    fn game_mut(&mut self) -> Result<&mut Game, String> {
        self.game
            .as_mut()
            .ok_or_else(|| "no level loaded: send {\"cmd\":\"load\",\"level\":...} first".into())
    }

    fn obs(&self, req: &Value) -> Value {
        let ascii = req["ascii"].as_bool().unwrap_or(false);
        json!({ "observation": self.game.as_ref().map(|g| observe(g, ascii)) })
    }

    fn handle(&mut self, req: &Value) -> Result<Value, String> {
        let cmd = req["cmd"].as_str().ok_or("missing \"cmd\"")?;
        match cmd {
            "levels" => Ok(json!({ "levels": self.content.levels.keys().collect::<Vec<_>>() })),
            "feel" => Ok(json!({ "feel": self.content.feel.as_ref() })),
            "load" => {
                let id = req["level"].as_str().ok_or("missing \"level\"")?;
                let level = self.content.level(id)?;
                self.seed = req["seed"].as_u64().unwrap_or(0);
                self.game = Some(Game::new(
                    level,
                    self.content.feel.clone(),
                    self.seed,
                    &self.content.scripts,
                ));
                self.recording.clear();
                self.saves.clear();
                Ok(self.obs(req))
            }
            "reset" => {
                let level = self.game_mut()?.level.clone();
                self.game = Some(Game::new(
                    level,
                    self.content.feel.clone(),
                    self.seed,
                    &self.content.scripts,
                ));
                self.recording.clear();
                Ok(self.obs(req))
            }
            "step" => {
                let inputs = if let Some(s) = req["inputs"].as_str() {
                    parse_inputs(s)?
                } else {
                    let b = Buttons::parse(req["buttons"].as_str().unwrap_or("-"))?;
                    let n = req["frames"].as_u64().unwrap_or(1).min(100_000) as usize;
                    vec![b; n]
                };
                let scripts = &self.content.scripts;
                let game = self.game.as_mut().ok_or("no level loaded")?;
                let mut stepped = 0;
                for b in inputs {
                    if game.state.status != Status::Playing {
                        break;
                    }
                    game.step(b, scripts);
                    self.recording.push(b);
                    stepped += 1;
                }
                let mut v = self.obs(req);
                v["stepped"] = json!(stepped);
                Ok(v)
            }
            "observe" => Ok(self.obs(req)),
            "save" => {
                let state = self.game_mut()?.state.clone();
                self.saves.push((state, self.recording.len()));
                Ok(json!({ "slot": self.saves.len() - 1 }))
            }
            "restore" => {
                let slot = req["slot"].as_u64().ok_or("missing \"slot\"")? as usize;
                let (state, len) = self.saves.get(slot).cloned().ok_or("no such slot")?;
                self.game_mut()?.state = state;
                self.recording.truncate(len);
                Ok(self.obs(req))
            }
            "replay" => {
                let game = self.game.as_ref().ok_or("no level loaded")?;
                let r = Replay::record(self.content, &game.level.id, self.seed, &self.recording)?;
                Ok(
                    json!({ "text": r.to_text(), "status": format!("{:?}", r.end_status).to_lowercase(), "first_script_error": r.first_script_error  }),
                )
            }
            "check_replay" => {
                let r = Replay::parse(req["text"].as_str().ok_or("missing \"text\"")?)?;
                let c = r.check(self.content);
                Ok(json!({ "passed": c.ok, "problems": c.problems, "frames": c.frames }))
            }
            other => Err(format!("unknown cmd '{other}'")),
        }
    }
}
