use crate::{Buttons, Feel, Level, SCREEN_H, SCREEN_W, TILE, Tile, fnv1a};
use ns_script_api::{
    Command, EntityView, Hook, HookInput, PlayerView, SUB, ScriptBackend, WorldQuery,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    Playing,
    Won,
    Dead,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Player {
    /// Top-left of the hitbox, sub-pixels.
    pub x: i32,
    pub y: i32,
    pub vx: i32,
    pub vy: i32,
    pub on_ground: bool,
    /// -1 left, 1 right.
    pub facing: i32,
    /// Rising from a jump (gravity is lower while jump is held).
    pub jumping: bool,
    pub coyote: u8,
    pub buffer: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entity {
    pub id: u32,
    pub kind: String,
    pub x: i32,
    pub y: i32,
    pub vx: i32,
    pub vy: i32,
    pub on_ground: bool,
    pub hit_wall_left: bool,
    pub hit_wall_right: bool,
    /// Script state, owned by the engine so it can be saved, replayed and shared across languages.
    pub vars: BTreeMap<String, i64>,
}

/// Everything that changes during play. Serializable: this *is* a save state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    pub frame: u64,
    pub seed: u64,
    pub status: Status,
    pub player: Player,
    /// Alive entities in id order.
    pub entities: Vec<Entity>,
    /// Events emitted last frame, delivered this frame.
    pub pending_events: Vec<String>,
    /// Camera top-left, sub-pixels.
    pub cam_x: i32,
    pub cam_y: i32,
    pub prev_buttons: Buttons,
    /// Script errors (most recent last, capped), so agents and CI can see them.
    pub script_errors: Vec<String>,
}

pub struct Game {
    pub level: Arc<Level>,
    pub feel: Arc<Feel>,
    pub state: State,
}

#[derive(Default)]
struct Contacts {
    ground: bool,
    left: bool,
    right: bool,
}

/// Move a box through the level one axis at a time. Speeds must stay below one tile per frame.
fn move_body(
    level: &Level,
    x: &mut i32,
    y: &mut i32,
    w: i32,
    h: i32,
    vx: &mut i32,
    vy: &mut i32,
) -> Contacts {
    let mut c = Contacts::default();
    let rows = |y: i32| y.div_euclid(TILE)..=(y + h - 1).div_euclid(TILE);
    let cols = |x: i32| x.div_euclid(TILE)..=(x + w - 1).div_euclid(TILE);

    // Horizontal.
    let nx = *x + *vx;
    if *vx > 0 {
        let col = (nx + w - 1).div_euclid(TILE);
        if rows(*y).any(|r| level.tile(col, r) == Tile::Solid) {
            *x = col * TILE - w;
            *vx = 0;
            c.right = true;
        } else {
            *x = nx;
        }
    } else if *vx < 0 {
        let col = nx.div_euclid(TILE);
        if rows(*y).any(|r| level.tile(col, r) == Tile::Solid) {
            *x = (col + 1) * TILE;
            *vx = 0;
            c.left = true;
        } else {
            *x = nx;
        }
    }

    // Vertical.
    let ny = *y + *vy;
    if *vy > 0 {
        let old_bottom = *y + h - 1;
        let row = (ny + h - 1).div_euclid(TILE);
        let top = row * TILE;
        let lands = cols(*x).any(|col| match level.tile(col, row) {
            Tile::Solid => true,
            Tile::Platform => old_bottom < top,
            _ => false,
        });
        if lands {
            *y = top - h;
            *vy = 0;
            c.ground = true;
        } else {
            *y = ny;
        }
    } else if *vy < 0 {
        let row = ny.div_euclid(TILE);
        if cols(*x).any(|col| level.tile(col, row) == Tile::Solid) {
            *y = (row + 1) * TILE;
            *vy = 0;
        } else {
            *y = ny;
        }
    }
    c
}

fn approach(v: i32, target: i32, step: i32) -> i32 {
    if v < target {
        (v + step).min(target)
    } else {
        (v - step).max(target)
    }
}

fn overlaps(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

const MAX_SCRIPT_ERRORS: usize = 16;

impl Game {
    pub fn new(level: Arc<Level>, feel: Arc<Feel>, seed: u64, scripts: &dyn ScriptBackend) -> Game {
        let (sc, sr) = level.player_start;
        let player = Player {
            x: sc * TILE + (TILE - feel.player_w * SUB) / 2,
            y: (sr + 1) * TILE - feel.player_h * SUB,
            vx: 0,
            vy: 0,
            on_ground: false,
            facing: 1,
            jumping: false,
            coyote: 0,
            buffer: 0,
        };
        let entities = level
            .spawns
            .iter()
            .enumerate()
            .map(|(i, (kind, col, row))| {
                let def = &level.defs[kind];
                Entity {
                    id: i as u32 + 1,
                    kind: kind.clone(),
                    x: col * TILE + (TILE - def.w * SUB) / 2,
                    y: (row + 1) * TILE - def.h * SUB,
                    vx: 0,
                    vy: 0,
                    on_ground: false,
                    hit_wall_left: false,
                    hit_wall_right: false,
                    vars: BTreeMap::new(),
                }
            })
            .collect();
        let mut game = Game {
            level,
            feel,
            state: State {
                frame: 0,
                seed,
                status: Status::Playing,
                player,
                entities,
                pending_events: Vec::new(),
                cam_x: 0,
                cam_y: 0,
                prev_buttons: Buttons::NONE,
                script_errors: Vec::new(),
            },
        };
        let (cx, cy) = game.camera_target();
        game.state.cam_x = cx;
        game.state.cam_y = cy;
        let mut events = Vec::new();
        for i in 0..game.state.entities.len() {
            game.run_hook(i, &Hook::Spawn, scripts, &mut events);
        }
        game.state
            .entities
            .retain(|e| !e.vars.contains_key(REMOVED));
        game.state.pending_events = events;
        game
    }

    /// Restore a saved [`State`] for this level.
    pub fn with_state(level: Arc<Level>, feel: Arc<Feel>, state: State) -> Game {
        Game { level, feel, state }
    }

    /// Stable hash of the full state, for replay checkpoints.
    pub fn hash(&self) -> u64 {
        fnv1a(&serde_json::to_vec(&self.state).expect("state serializes"))
    }

    /// Advance one frame.
    pub fn step(&mut self, buttons: Buttons, scripts: &dyn ScriptBackend) {
        if self.state.status != Status::Playing {
            return;
        }
        let prev_bottom = self.state.player.y + self.feel.player_h * SUB;
        self.step_player(buttons);
        self.state.prev_buttons = buttons;
        self.step_entities(scripts);
        self.interact(buttons, prev_bottom);
        self.check_status();
        self.step_camera();
        self.state.frame += 1;
    }

    fn step_player(&mut self, b: Buttons) {
        let f = &*self.feel;
        let p = &mut self.state.player;
        let dir = b.dir_x();
        let running = b.has(Buttons::RUN);
        let max = if running { f.run_max } else { f.walk_max };
        if dir != 0 {
            p.facing = dir;
        }

        // Horizontal speed.
        if p.on_ground {
            if dir != 0 && p.vx * dir < 0 {
                p.vx += dir * f.skid_decel;
            } else if dir != 0 && p.vx * dir <= max {
                let accel = if running { f.run_accel } else { f.walk_accel };
                p.vx = (p.vx + dir * accel).clamp(-max, max);
            } else if dir != 0 {
                p.vx = approach(p.vx, dir * max, f.release_decel);
            } else {
                p.vx = approach(p.vx, 0, f.release_decel);
            }
        } else if dir != 0 && p.vx * dir < max {
            // In the air you can steer but never lose momentum above your top speed.
            p.vx = (p.vx + dir * f.air_accel).clamp(-max, max);
        }

        // Jump, with a short input buffer and coyote time.
        if b.has(Buttons::JUMP) && !self.state.prev_buttons.has(Buttons::JUMP) {
            // +1 because the buffer also counts down on the frame of the press.
            p.buffer = f.buffer_frames.saturating_add(1);
        }
        if p.buffer > 0 && (p.on_ground || p.coyote > 0) {
            p.vy = -(f.jump_velocity + f.jump_run_bonus * p.vx.abs() / f.run_max);
            p.jumping = true;
            p.buffer = 0;
            p.coyote = 0;
            p.on_ground = false;
        }

        // Gravity: lighter while rising with jump held, so jump height depends on hold time.
        let g = if p.jumping && p.vy < 0 && b.has(Buttons::JUMP) {
            f.gravity_held
        } else {
            f.gravity
        };
        p.vy = (p.vy + g).min(f.max_fall);
        if p.vy >= 0 {
            p.jumping = false;
        }

        let (w, h) = (f.player_w * SUB, f.player_h * SUB);
        let c = move_body(&self.level, &mut p.x, &mut p.y, w, h, &mut p.vx, &mut p.vy);
        p.on_ground = c.ground;
        p.coyote = if p.on_ground {
            f.coyote_frames
        } else {
            p.coyote.saturating_sub(1)
        };
        p.buffer = p.buffer.saturating_sub(1);
    }

    fn step_entities(&mut self, scripts: &dyn ScriptBackend) {
        let incoming = std::mem::take(&mut self.state.pending_events);
        let mut emitted = Vec::new();
        for i in 0..self.state.entities.len() {
            for ev in &incoming {
                self.run_hook(i, &Hook::Event(ev.clone()), scripts, &mut emitted);
            }
            self.run_hook(i, &Hook::Tick, scripts, &mut emitted);
            if self.state.entities[i].vars.contains_key(REMOVED) {
                continue;
            }
            let def = &self.level.defs[&self.state.entities[i].kind];
            let (w, h, gravity) = (def.w * SUB, def.h * SUB, def.gravity);
            let e = &mut self.state.entities[i];
            if gravity {
                e.vy = (e.vy + self.feel.gravity).min(self.feel.max_fall);
            }
            let c = move_body(&self.level, &mut e.x, &mut e.y, w, h, &mut e.vx, &mut e.vy);
            e.on_ground = c.ground;
            e.hit_wall_left = c.left;
            e.hit_wall_right = c.right;
            if e.y > self.level.height_px() * SUB {
                e.vars.insert(REMOVED.into(), 1);
            }
        }
        self.state
            .entities
            .retain(|e| !e.vars.contains_key(REMOVED));
        self.state.pending_events = emitted;
    }

    fn run_hook(
        &mut self,
        i: usize,
        hook: &Hook,
        scripts: &dyn ScriptBackend,
        emitted: &mut Vec<String>,
    ) {
        let e = &self.state.entities[i];
        if e.vars.contains_key(REMOVED) {
            return;
        }
        let def = &self.level.defs[&e.kind];
        let p = &self.state.player;
        let input = HookInput {
            frame: self.state.frame,
            seed: self.state.seed,
            me: EntityView {
                id: e.id,
                kind: e.kind.clone(),
                x: e.x,
                y: e.y,
                w: def.w,
                h: def.h,
                vx: e.vx,
                vy: e.vy,
                on_ground: e.on_ground,
                hit_wall_left: e.hit_wall_left,
                hit_wall_right: e.hit_wall_right,
                vars: e.vars.clone(),
            },
            player: PlayerView {
                x: p.x,
                y: p.y,
                w: self.feel.player_w,
                h: self.feel.player_h,
                vx: p.vx,
                vy: p.vy,
            },
        };
        let world: Arc<dyn WorldQuery> = self.level.clone();
        match scripts.call(&def.script, hook, &input, world) {
            Ok(cmds) => {
                let e = &mut self.state.entities[i];
                for cmd in cmds {
                    match cmd {
                        Command::SetVx(v) => e.vx = v.clamp(-MAX_SPEED, MAX_SPEED),
                        Command::SetVy(v) => e.vy = v.clamp(-MAX_SPEED, MAX_SPEED),
                        Command::SetVar(k, v) => {
                            e.vars.insert(k, v);
                        }
                        Command::Emit(ev) => emitted.push(ev),
                        Command::Remove => {
                            e.vars.insert(REMOVED.into(), 1);
                        }
                    }
                }
            }
            Err(msg) => {
                let errors = &mut self.state.script_errors;
                errors.push(format!(
                    "frame {} entity {} ({}) {}: {msg}",
                    self.state.frame,
                    e.id,
                    def.script,
                    hook.handler_name()
                ));
                if errors.len() > MAX_SCRIPT_ERRORS {
                    errors.remove(0);
                }
            }
        }
    }

    fn interact(&mut self, b: Buttons, prev_bottom: i32) {
        let f = &*self.feel;
        let p = &mut self.state.player;
        let pbox = (p.x, p.y, f.player_w * SUB, f.player_h * SUB);
        let mut stomped = false;
        for e in &mut self.state.entities {
            let def = &self.level.defs[&e.kind];
            if !overlaps(pbox, (e.x, e.y, def.w * SUB, def.h * SUB)) {
                continue;
            }
            // A stomp: falling, and the player's feet were above the enemy's upper half.
            if def.stompable && p.vy > 0 && prev_bottom <= e.y + def.h * SUB / 2 {
                e.vars.insert(REMOVED.into(), 1);
                stomped = true;
            } else if def.harmful && !stomped {
                self.state.status = Status::Dead;
                return;
            }
        }
        if stomped {
            p.vy = -f.stomp_bounce;
            // Holding jump through a stomp bounces higher, like a jump.
            p.jumping = b.has(Buttons::JUMP);
            self.state
                .entities
                .retain(|e| !e.vars.contains_key(REMOVED));
        }
    }

    fn check_status(&mut self) {
        let f = &*self.feel;
        let p = &self.state.player;
        if self.state.status != Status::Playing {
            return;
        }
        if p.y > self.level.height_px() * SUB {
            self.state.status = Status::Dead;
            return;
        }
        let (w, h) = (f.player_w * SUB, f.player_h * SUB);
        let cols = p.x.div_euclid(TILE)..=(p.x + w - 1).div_euclid(TILE);
        let rows = p.y.div_euclid(TILE)..=(p.y + h - 1).div_euclid(TILE);
        for c in cols {
            for r in rows.clone() {
                if self.level.tile(c, r) == Tile::Goal {
                    self.state.status = Status::Won;
                    return;
                }
            }
        }
    }

    fn camera_target(&self) -> (i32, i32) {
        let f = &*self.feel;
        let p = &self.state.player;
        let cx = p.x + f.player_w * SUB / 2 + p.facing * f.camera_lead * SUB - SCREEN_W * SUB / 2;
        let cy = p.y + f.player_h * SUB / 2 - SCREEN_H * SUB / 2;
        let max_x = ((self.level.width_px() - SCREEN_W) * SUB).max(0);
        let max_y = ((self.level.height_px() - SCREEN_H) * SUB).max(0);
        (cx.clamp(0, max_x), cy.clamp(0, max_y))
    }

    fn step_camera(&mut self) {
        let (tx, ty) = self.camera_target();
        let s = self.feel.camera_speed;
        self.state.cam_x = approach(self.state.cam_x, tx, s);
        self.state.cam_y = approach(self.state.cam_y, ty, s);
    }
}

/// Internal marker var for entities removed this frame.
const REMOVED: &str = "__removed";
/// Velocity cap for script-set speeds: below one tile per frame keeps collision exact.
const MAX_SPEED: i32 = TILE - 1;
