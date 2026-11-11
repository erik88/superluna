use serde::{Deserialize, Serialize};
use std::fmt;

/// Buttons held during one frame.
///
/// Text form (used in replays and the agent interface): any of `L R U D J X`, or `-` for none.
/// `J` is jump, `X` is run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Buttons(pub u8);

impl Buttons {
    pub const NONE: Buttons = Buttons(0);
    pub const LEFT: Buttons = Buttons(1);
    pub const RIGHT: Buttons = Buttons(2);
    pub const UP: Buttons = Buttons(4);
    pub const DOWN: Buttons = Buttons(8);
    pub const JUMP: Buttons = Buttons(16);
    pub const RUN: Buttons = Buttons(32);

    const LETTERS: [(char, Buttons); 6] = [
        ('L', Buttons::LEFT),
        ('R', Buttons::RIGHT),
        ('U', Buttons::UP),
        ('D', Buttons::DOWN),
        ('J', Buttons::JUMP),
        ('X', Buttons::RUN),
    ];

    pub fn has(self, b: Buttons) -> bool {
        self.0 & b.0 == b.0 && b.0 != 0
    }

    pub fn with(self, b: Buttons) -> Buttons {
        Buttons(self.0 | b.0)
    }

    /// -1, 0 or 1. Pressing both directions cancels out.
    pub fn dir_x(self) -> i32 {
        i32::from(self.has(Buttons::RIGHT)) - i32::from(self.has(Buttons::LEFT))
    }

    pub fn parse(s: &str) -> Result<Buttons, String> {
        let mut out = Buttons::NONE;
        for c in s.chars() {
            if c == '-' {
                continue;
            }
            let (_, b) = Self::LETTERS
                .iter()
                .find(|(l, _)| *l == c.to_ascii_uppercase())
                .ok_or_else(|| format!("unknown button '{c}' (use L R U D J X or -)"))?;
            out = out.with(*b);
        }
        Ok(out)
    }
}

impl fmt::Display for Buttons {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 == 0 {
            return f.write_str("-");
        }
        for (c, b) in Self::LETTERS {
            if self.has(b) {
                write!(f, "{c}")?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        for s in ["-", "R", "RJX", "LDJ"] {
            assert_eq!(Buttons::parse(s).unwrap().to_string(), s);
        }
        assert!(Buttons::parse("Q").is_err());
    }
}
