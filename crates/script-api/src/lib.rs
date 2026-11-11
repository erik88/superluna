//! The boundary between the simulation and any scripting language.
//!
//! Rules every backend must follow (see the project plan):
//! 1. Scripts only provide handlers (`on_spawn`, `on_tick`, `on_event`); they never call each other.
//! 2. Handlers receive a read-only [`HookInput`] and return [`Command`]s. The core applies them in a
//!    fixed order, so the language that produced them does not matter.
//! 3. Persistent script state lives in engine-owned `vars`, never in interpreter globals.
//! 4. Backends must be deterministic: no clock, no unseeded randomness, no order-dependent iteration.
//!
//! Units: positions and velocities are integers in sub-pixels ([`SUB`] per pixel).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Sub-pixel units per pixel.
pub const SUB: i32 = 256;
/// Tile size in pixels.
pub const TILE_PX: i32 = 16;

/// Which handler is being invoked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hook {
    Spawn,
    Tick,
    /// An event emitted (by any entity) during the previous frame.
    Event(String),
}

impl Hook {
    pub fn handler_name(&self) -> &'static str {
        match self {
            Hook::Spawn => "on_spawn",
            Hook::Tick => "on_tick",
            Hook::Event(_) => "on_event",
        }
    }
}

/// Read-only view of the entity running the script.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityView {
    pub id: u32,
    pub kind: String,
    /// Top-left corner, sub-pixels.
    pub x: i32,
    pub y: i32,
    /// Hitbox size, pixels.
    pub w: i32,
    pub h: i32,
    pub vx: i32,
    pub vy: i32,
    pub on_ground: bool,
    pub hit_wall_left: bool,
    pub hit_wall_right: bool,
    pub vars: BTreeMap<String, i64>,
}

/// Read-only view of the player.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerView {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub vx: i32,
    pub vy: i32,
}

/// Everything a handler may read.
#[derive(Clone, Debug)]
pub struct HookInput {
    pub frame: u64,
    pub seed: u64,
    pub me: EntityView,
    pub player: PlayerView,
}

/// Queries against the static level geometry (pixel coordinates).
pub trait WorldQuery: Send + Sync {
    /// A fully solid tile covers this pixel.
    fn solid_at(&self, px: i32, py: i32) -> bool;
    /// Something can be stood on at this pixel (solid or a jump-through platform).
    fn floor_at(&self, px: i32, py: i32) -> bool;
}

/// Everything a handler may ask the core to do. Applied in the order returned.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    SetVx(i32),
    SetVy(i32),
    SetVar(String, i64),
    /// Broadcast an event to every entity's `on_event` next frame.
    Emit(String),
    /// Remove this entity from the level.
    Remove,
}

/// A scripting language implementation.
///
/// Implementations hold only compiled code, never game state, so they can be shared and the
/// simulation stays fully serializable.
pub trait ScriptBackend {
    /// Run `hook` for the script `script_id`. A missing handler is not an error: return no commands.
    fn call(
        &self,
        script_id: &str,
        hook: &Hook,
        input: &HookInput,
        world: Arc<dyn WorldQuery>,
    ) -> Result<Vec<Command>, String>;
}

/// A backend that runs no scripts. Useful for physics-only tests.
pub struct NoScripts;

impl ScriptBackend for NoScripts {
    fn call(
        &self,
        _: &str,
        _: &Hook,
        _: &HookInput,
        _: Arc<dyn WorldQuery>,
    ) -> Result<Vec<Command>, String> {
        Ok(Vec::new())
    }
}

/// Deterministic random number for scripts: a pure function of its inputs (splitmix64).
pub fn det_rand(seed: u64, frame: u64, entity: u32, counter: u64) -> u64 {
    let mut z = seed
        ^ frame.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (u64::from(entity)).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ counter.wrapping_mul(0x1656_67B1_9E37_79F9);
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}
