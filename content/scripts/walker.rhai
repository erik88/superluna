// Walks back and forth, turning at walls and ledges.

const SPEED = 96; // sub-pixels per frame (0.375 px)

fn on_spawn(e) {
    e.set_var("dir", -1);
}

fn on_tick(e) {
    let dir = e.get_var("dir");
    if (dir < 0 && e.hit_wall_left) || (dir > 0 && e.hit_wall_right) {
        dir = -dir;
    } else if e.on_ground {
        // Probe the floor just past the leading foot.
        let probe_x = if dir < 0 { e.left - 1 } else { e.right + 1 };
        if !e.floor_at(probe_x, e.bottom + 1) {
            dir = -dir;
        }
    }
    e.set_var("dir", dir);
    e.set_vx(dir * global::SPEED);
}
