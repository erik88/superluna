use ns_core::{Game, SCREEN_H, SCREEN_W, SUB, TILE_PX, Tile};
use serde_json::{Value, json};

/// The state as agents see it. Positions in pixels (top-left of hitbox), speeds in sub-pixels.
pub fn observe(game: &Game, ascii: bool) -> Value {
    let s = &game.state;
    let p = &s.player;
    let entities: Vec<Value> = s
        .entities
        .iter()
        .map(|e| {
            json!({
                "id": e.id, "kind": e.kind,
                "x": e.x.div_euclid(SUB), "y": e.y.div_euclid(SUB),
                "vx": e.vx, "vy": e.vy, "on_ground": e.on_ground, "vars": e.vars,
            })
        })
        .collect();
    let mut v = json!({
        "frame": s.frame,
        "status": format!("{:?}", s.status).to_lowercase(),
        "level": { "id": game.level.id, "width": game.level.width_px(), "height": game.level.height_px() },
        "player": {
            "x": p.x.div_euclid(SUB), "y": p.y.div_euclid(SUB), "w": game.feel.player_w, "h": game.feel.player_h,
            "vx": p.vx, "vy": p.vy, "on_ground": p.on_ground, "facing": p.facing,
        },
        "entities": entities,
        "camera": { "x": s.cam_x.div_euclid(SUB), "y": s.cam_y.div_euclid(SUB) },
        "script_errors": s.script_errors,
        "hash": format!("{:016x}", game.hash()),
    });
    if ascii {
        v["ascii"] = json!(ascii_view(game));
    }
    v
}

/// The visible screen as text, one character per tile: `#` solid, `=` platform, `G` goal,
/// `@` player, first letter of the kind for entities.
pub fn ascii_view(game: &Game) -> Vec<String> {
    let s = &game.state;
    let c0 = s.cam_x.div_euclid(SUB * TILE_PX);
    let r0 = s.cam_y.div_euclid(SUB * TILE_PX);
    let cols = SCREEN_W / TILE_PX + 1;
    let rows = SCREEN_H / TILE_PX;
    let mut grid: Vec<Vec<char>> = (0..rows)
        .map(|r| {
            (0..cols)
                .map(|c| match game.level.tile(c0 + c, r0 + r) {
                    Tile::Empty => '.',
                    Tile::Solid => '#',
                    Tile::Platform => '=',
                    Tile::Goal => 'G',
                })
                .collect()
        })
        .collect();
    let mut mark = |x: i32, y: i32, w: i32, h: i32, ch: char| {
        let t = SUB * TILE_PX;
        for r in y.div_euclid(t)..=(y + h * SUB - 1).div_euclid(t) {
            for c in x.div_euclid(t)..=(x + w * SUB - 1).div_euclid(t) {
                let (gr, gc) = (r - r0, c - c0);
                if (0..rows).contains(&gr) && (0..cols).contains(&gc) {
                    grid[gr as usize][gc as usize] = ch;
                }
            }
        }
    };
    for e in &s.entities {
        let def = &game.level.defs[&e.kind];
        mark(e.x, e.y, def.w, def.h, e.kind.chars().next().unwrap_or('?'));
    }
    let p = &s.player;
    mark(p.x, p.y, game.feel.player_w, game.feel.player_h, '@');
    grid.into_iter().map(|r| r.into_iter().collect()).collect()
}
