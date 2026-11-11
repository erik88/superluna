use serde::{Deserialize, Serialize};

/// The feel spec: every number that shapes how the player moves.
///
/// Speeds are sub-pixels per frame, accelerations sub-pixels per frame per frame, at 60 frames per
/// second. Values live in `content/feel.ron` and are tuned to meet the design targets in
/// `content/feel_targets.ron`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Feel {
    pub walk_max: i32,
    pub run_max: i32,
    pub walk_accel: i32,
    pub run_accel: i32,
    pub air_accel: i32,
    /// Slowing down with no direction held (on the ground), or above the current top speed.
    pub release_decel: i32,
    /// Pressing against your current direction on the ground.
    pub skid_decel: i32,
    /// Gravity while rising with jump held: lower, so holding jump goes higher.
    pub gravity_held: i32,
    pub gravity: i32,
    pub max_fall: i32,
    pub jump_velocity: i32,
    /// Extra jump velocity at full run speed, scaled linearly with horizontal speed.
    pub jump_run_bonus: i32,
    /// Frames after leaving a ledge during which a jump still works.
    pub coyote_frames: u8,
    /// Frames a jump press is remembered before landing.
    pub buffer_frames: u8,
    pub stomp_bounce: i32,
    /// Player hitbox in pixels.
    pub player_w: i32,
    pub player_h: i32,
    /// How far ahead of the player the camera looks, pixels.
    pub camera_lead: i32,
    /// Max camera movement per frame, sub-pixels.
    pub camera_speed: i32,
}

impl Feel {
    pub fn from_ron(s: &str) -> Result<Feel, String> {
        ron::from_str(s).map_err(|e| format!("feel.ron: {e}"))
    }
}
