//! Headless play: everything agents and CI need, with no window.

mod content;
mod observe;
mod protocol;
mod replay;

pub use content::Content;
pub use observe::{ascii_view, observe};
pub use protocol::Session;
pub use replay::{CHECKPOINT_EVERY, Replay, ReplayCheck, parse_inputs};
