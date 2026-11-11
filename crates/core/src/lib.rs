//! Deterministic simulation core. No IO, no clock, no floating point in game state.
//!
//! Callers load content as strings and pass them in; see [`Content`].

mod buttons;
mod feel;
mod level;
mod sim;

pub use buttons::Buttons;
pub use feel::Feel;
pub use level::{EntityDef, Level, ParseError, Tile};
pub use ns_script_api::{self as script_api, SUB, TILE_PX};
pub use sim::{Entity, Game, Player, State, Status};

/// Sub-pixel units per tile.
pub const TILE: i32 = TILE_PX * SUB;
/// Simulation rate.
pub const FPS: u32 = 60;
/// Visible area in pixels (asset contract).
pub const SCREEN_W: i32 = 256;
pub const SCREEN_H: i32 = 224;

/// 64-bit FNV-1a, used for state hashes in replays. Stable across platforms.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}
