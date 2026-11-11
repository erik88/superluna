//! Playable frontend. Placeholder art: every shape is drawn at the asset-contract sizes.
//!
//! Keys: arrows or A/D move, Z / Space / K jump, X / Shift / J run, R restart,
//! F2 reload content from disk (desktop), F5 save a replay of this attempt (desktop).

use macroquad::prelude::*;
use ns_core::{Buttons, Feel, Game, Level, SCREEN_H, SCREEN_W, SUB, Status, TILE_PX, Tile};
use ns_script_rhai::RhaiBackend;
use std::sync::Arc;

const SCALE: i32 = 3;
#[cfg(not(target_arch = "wasm32"))]
const LEVEL_ID: &str = "meadow-1";

/// Content compiled into the binary, so the web build needs no file access.
mod embedded {
    pub const FEEL: &str = include_str!("../../../content/feel.ron");
    pub const ENTITIES: &str = include_str!("../../../content/entities.ron");
    pub const LEVEL: &str = include_str!("../../../content/levels/meadow-1.ron");
    pub const SCRIPTS: [(&str, &str); 2] = [
        (
            "walker",
            include_str!("../../../content/scripts/walker.rhai"),
        ),
        (
            "hopper",
            include_str!("../../../content/scripts/hopper.rhai"),
        ),
    ];
}

struct Loaded {
    feel: Arc<Feel>,
    level: Arc<Level>,
    scripts: RhaiBackend,
}

fn build(
    feel: &str,
    entities: &str,
    level: &str,
    scripts: &[(String, String)],
) -> Result<Loaded, String> {
    let mut backend = RhaiBackend::new();
    for (id, src) in scripts {
        backend.load(id, src)?;
    }
    Ok(Loaded {
        feel: Arc::new(Feel::from_ron(feel)?),
        level: Arc::new(Level::parse(level, entities).map_err(|e| e.to_string())?),
        scripts: backend,
    })
}

fn load_embedded() -> Loaded {
    let scripts: Vec<(String, String)> = embedded::SCRIPTS
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
    build(
        embedded::FEEL,
        embedded::ENTITIES,
        embedded::LEVEL,
        &scripts,
    )
    .expect("embedded content is valid")
}

/// Desktop: read the live content directory so edits show up on F2.
#[cfg(not(target_arch = "wasm32"))]
fn load_from_disk() -> Result<Loaded, String> {
    let dir = std::path::Path::new("content");
    let read = |p: std::path::PathBuf| {
        std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))
    };
    let mut scripts = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(dir.join("scripts"))
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "rhai"))
        .collect();
    entries.sort();
    for p in entries {
        scripts.push((
            p.file_stem().unwrap().to_string_lossy().to_string(),
            read(p.clone())?,
        ));
    }
    build(
        &read(dir.join("feel.ron"))?,
        &read(dir.join("entities.ron"))?,
        &read(dir.join("levels").join(format!("{LEVEL_ID}.ron")))?,
        &scripts,
    )
}

#[cfg(target_arch = "wasm32")]
fn load_from_disk() -> Result<Loaded, String> {
    Err("no file access on the web".into())
}

fn read_buttons() -> Buttons {
    let any = |ks: &[KeyCode]| ks.iter().any(|k| is_key_down(*k));
    let mut b = Buttons::NONE;
    if any(&[KeyCode::Left, KeyCode::A]) {
        b = b.with(Buttons::LEFT);
    }
    if any(&[KeyCode::Right, KeyCode::D]) {
        b = b.with(Buttons::RIGHT);
    }
    if any(&[KeyCode::Up, KeyCode::W]) {
        b = b.with(Buttons::UP);
    }
    if any(&[KeyCode::Down, KeyCode::S]) {
        b = b.with(Buttons::DOWN);
    }
    if any(&[KeyCode::Z, KeyCode::Space, KeyCode::K]) {
        b = b.with(Buttons::JUMP);
    }
    if any(&[
        KeyCode::X,
        KeyCode::LeftShift,
        KeyCode::RightShift,
        KeyCode::J,
    ]) {
        b = b.with(Buttons::RUN);
    }
    b
}

// Placeholder palette. Deliberately plain: real art comes from the milestone 2 pipeline.
const SKY: Color = Color::new(0.55, 0.72, 0.86, 1.0);
const SOLID: Color = Color::new(0.42, 0.33, 0.27, 1.0);
const SOLID_TOP: Color = Color::new(0.36, 0.62, 0.36, 1.0);
const PLATFORM: Color = Color::new(0.80, 0.70, 0.45, 1.0);
const GOAL: Color = Color::new(0.95, 0.95, 0.95, 1.0);
const PLAYER: Color = Color::new(0.20, 0.30, 0.75, 1.0);
const TEXT: Color = Color::new(0.08, 0.08, 0.12, 1.0);

fn entity_color(kind: &str) -> Color {
    match kind {
        "walker" => Color::new(0.75, 0.30, 0.25, 1.0),
        "hopper" => Color::new(0.45, 0.25, 0.60, 1.0),
        _ => MAGENTA,
    }
}

fn draw_world(game: &Game) {
    clear_background(SKY);
    let s = &game.state;
    let (cx, cy) = (s.cam_x.div_euclid(SUB), s.cam_y.div_euclid(SUB));
    let t = TILE_PX as f32;
    let (c0, r0) = (cx.div_euclid(TILE_PX), cy.div_euclid(TILE_PX));
    for r in r0..=r0 + SCREEN_H / TILE_PX + 1 {
        for c in c0..=c0 + SCREEN_W / TILE_PX + 1 {
            let x = (c * TILE_PX - cx) as f32;
            let y = (r * TILE_PX - cy) as f32;
            match game.level.tile(c, r) {
                Tile::Solid => {
                    draw_rectangle(x, y, t, t, SOLID);
                    if game.level.tile(c, r - 1) != Tile::Solid {
                        draw_rectangle(x, y, t, 4.0, SOLID_TOP);
                    }
                }
                Tile::Platform => draw_rectangle(x, y, t, 5.0, PLATFORM),
                Tile::Goal => draw_rectangle(x + 6.0, y, 4.0, t, GOAL),
                Tile::Empty => {}
            }
        }
    }
    for e in &s.entities {
        let def = &game.level.defs[&e.kind];
        let (x, y) = (
            (e.x.div_euclid(SUB) - cx) as f32,
            (e.y.div_euclid(SUB) - cy) as f32,
        );
        draw_rectangle(x, y, def.w as f32, def.h as f32, entity_color(&e.kind));
        draw_rectangle(x + 3.0, y + 3.0, 2.0, 2.0, WHITE);
        draw_rectangle(x + def.w as f32 - 5.0, y + 3.0, 2.0, 2.0, WHITE);
    }
    let p = &s.player;
    let (pw, ph) = (game.feel.player_w as f32, game.feel.player_h as f32);
    let (x, y) = (
        (p.x.div_euclid(SUB) - cx) as f32,
        (p.y.div_euclid(SUB) - cy) as f32,
    );
    draw_rectangle(x, y, pw, ph, PLAYER);
    let eye_x = if p.facing > 0 { x + pw - 4.0 } else { x + 2.0 };
    draw_rectangle(eye_x, y + 5.0, 2.0, 3.0, WHITE);
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Untitled Platformer".to_string(),
        window_width: SCREEN_W * SCALE,
        window_height: SCREEN_H * SCALE,
        high_dpi: false,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut loaded = load_from_disk().unwrap_or_else(|_| load_embedded());
    let new_game = |l: &Loaded| Game::new(l.level.clone(), l.feel.clone(), 0, &l.scripts);
    let mut game = new_game(&loaded);
    let mut recording: Vec<Buttons> = Vec::new();
    let mut message = String::new();
    let mut message_until = 0.0;

    let target = render_target(SCREEN_W as u32, SCREEN_H as u32);
    target.texture.set_filter(FilterMode::Nearest);
    let mut cam =
        Camera2D::from_display_rect(Rect::new(0.0, 0.0, SCREEN_W as f32, SCREEN_H as f32));
    cam.render_target = Some(target.clone());
    // Render targets are flipped vertically in macroquad.
    cam.zoom.y = -cam.zoom.y;

    let dt = 1.0 / ns_core::FPS as f64;
    let mut acc = 0.0;
    loop {
        let now = get_time();
        if is_key_pressed(KeyCode::R) {
            game = new_game(&loaded);
            recording.clear();
        }
        if is_key_pressed(KeyCode::F2) {
            match load_from_disk() {
                Ok(l) => {
                    loaded = l;
                    game = new_game(&loaded);
                    recording.clear();
                    message = "content reloaded".into();
                }
                Err(e) => message = format!("reload failed: {e}"),
            }
            message_until = now + 4.0;
        }
        #[cfg(not(target_arch = "wasm32"))]
        if is_key_pressed(KeyCode::F5) {
            message = save_replay(&recording);
            message_until = now + 4.0;
        }

        // Fixed 60 Hz simulation, decoupled from the display rate.
        acc = (acc + get_frame_time() as f64).min(dt * 5.0);
        while acc >= dt {
            let b = read_buttons();
            if game.state.status == Status::Playing {
                game.step(b, &loaded.scripts);
                recording.push(b);
            }
            acc -= dt;
        }

        set_camera(&cam);
        draw_world(&game);
        let secs = game.state.frame as f32 / ns_core::FPS as f32;
        draw_text(format!("{secs:5.1}s"), 4.0, 12.0, 16.0, TEXT);
        let banner = match game.state.status {
            Status::Won => Some("CLEAR!  R to play again"),
            Status::Dead => Some("Ouch.  R to retry"),
            Status::Playing => None,
        };
        if let Some(b) = banner {
            draw_text(b, 40.0, 100.0, 20.0, TEXT);
        }
        if now < message_until {
            draw_text(&message, 4.0, SCREEN_H as f32 - 6.0, 14.0, TEXT);
        }
        if let Some(err) = game.state.script_errors.last() {
            draw_text(err, 4.0, 26.0, 12.0, RED);
        }

        // Scale the low-resolution image up by the largest whole number that fits.
        set_default_camera();
        clear_background(BLACK);
        let k = ((screen_width() / SCREEN_W as f32).min(screen_height() / SCREEN_H as f32))
            .floor()
            .max(1.0);
        let (w, h) = (SCREEN_W as f32 * k, SCREEN_H as f32 * k);
        draw_texture_ex(
            &target.texture,
            ((screen_width() - w) / 2.0).floor(),
            ((screen_height() - h) / 2.0).floor(),
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(w, h)),
                ..Default::default()
            },
        );
        next_frame().await;
    }
}

/// Save this attempt's inputs. Turn them into a checked replay with
/// `ns record meadow-1 "$(cat meadow-1.local.inputs)" --out FILE`.
#[cfg(not(target_arch = "wasm32"))]
fn save_replay(inputs: &[Buttons]) -> String {
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < inputs.len() {
        let b = inputs[i];
        let n = inputs[i..].iter().take_while(|x| **x == b).count();
        tokens.push(format!("{n}*{b}"));
        i += n;
    }
    let path = format!("{LEVEL_ID}.local.inputs");
    let body = format!("{}\n", tokens.join(" "));
    match std::fs::write(&path, body) {
        Ok(()) => format!("saved inputs to {path}"),
        Err(e) => format!("could not save: {e}"),
    }
}
