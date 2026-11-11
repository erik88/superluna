// Waits, then hops toward the player. The wait varies a little (deterministically).

const HOP_X = 160;  // sub-pixels per frame
const HOP_Y = 900;

fn on_spawn(e) {
    e.set_var("wait", 50 + e.rand(30));
}

fn on_tick(e) {
    if !e.on_ground {
        return;
    }
    let wait = e.get_var("wait") - 1;
    if wait > 0 {
        e.set_var("wait", wait);
        e.set_vx(0);
        return;
    }
    let dir = if e.player_x < e.center_x { -1 } else { 1 };
    e.set_vx(dir * global::HOP_X);
    e.set_vy(-global::HOP_Y);
    e.set_var("wait", 50 + e.rand(30));
}
