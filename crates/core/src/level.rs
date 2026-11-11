use ns_script_api::{TILE_PX, WorldQuery};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tile {
    Empty,
    Solid,
    /// Can be jumped up through and stood on.
    Platform,
    /// Touching it completes the level.
    Goal,
}

/// How an entity kind behaves. Loaded from `content/entities.ron`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityDef {
    /// Script id, e.g. `"walker"` for `content/scripts/walker.rhai`.
    pub script: String,
    /// Hitbox in pixels.
    pub w: i32,
    pub h: i32,
    #[serde(default = "yes")]
    pub gravity: bool,
    #[serde(default = "yes")]
    pub stompable: bool,
    #[serde(default = "yes")]
    pub harmful: bool,
}

fn yes() -> bool {
    true
}

/// The on-disk level format: an ASCII grid plus a legend for entities.
///
/// Grid characters: `.` empty, `#` solid, `=` platform, `G` goal, `P` player start. Any other
/// character must appear in `legend`, which maps it to an entity kind.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LevelFile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub legend: BTreeMap<char, String>,
    pub rows: Vec<String>,
}

#[derive(Debug)]
pub struct ParseError(pub String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ParseError {}

/// Static level data. Never changes during play, so it is shared (`Arc`) rather than saved.
#[derive(Clone, Debug)]
pub struct Level {
    pub id: String,
    pub name: String,
    /// Size in tiles.
    pub width: i32,
    pub height: i32,
    tiles: Vec<Tile>,
    /// Tile column and row of the player start.
    pub player_start: (i32, i32),
    /// Entity kind and tile position, in grid reading order.
    pub spawns: Vec<(String, i32, i32)>,
    pub defs: BTreeMap<String, EntityDef>,
}

impl Level {
    pub fn parse(level_ron: &str, entities_ron: &str) -> Result<Level, ParseError> {
        let file: LevelFile =
            ron::from_str(level_ron).map_err(|e| ParseError(format!("level: {e}")))?;
        let defs: BTreeMap<String, EntityDef> =
            ron::from_str(entities_ron).map_err(|e| ParseError(format!("entities.ron: {e}")))?;
        Level::from_file(file, defs)
    }

    pub fn from_file(
        file: LevelFile,
        defs: BTreeMap<String, EntityDef>,
    ) -> Result<Level, ParseError> {
        let height = file.rows.len() as i32;
        let width = file
            .rows
            .iter()
            .map(|r| r.chars().count())
            .max()
            .unwrap_or(0) as i32;
        if width == 0 || height == 0 {
            return Err(ParseError(format!("{}: empty level", file.id)));
        }
        let mut tiles = vec![Tile::Empty; (width * height) as usize];
        let mut player_start = None;
        let mut spawns = Vec::new();
        for (row, line) in file.rows.iter().enumerate() {
            for (col, c) in line.chars().enumerate() {
                let (col, row) = (col as i32, row as i32);
                let tile = match c {
                    '.' | ' ' => Tile::Empty,
                    '#' => Tile::Solid,
                    '=' => Tile::Platform,
                    'G' => Tile::Goal,
                    'P' => {
                        if player_start.replace((col, row)).is_some() {
                            return Err(ParseError(format!("{}: more than one 'P'", file.id)));
                        }
                        Tile::Empty
                    }
                    other => {
                        let kind = file.legend.get(&other).ok_or_else(|| {
                            ParseError(format!(
                                "{}: unknown character '{other}' at column {col}, row {row}",
                                file.id
                            ))
                        })?;
                        if !defs.contains_key(kind) {
                            return Err(ParseError(format!(
                                "{}: legend maps '{other}' to unknown entity kind '{kind}'",
                                file.id
                            )));
                        }
                        spawns.push((kind.clone(), col, row));
                        Tile::Empty
                    }
                };
                tiles[(row * width + col) as usize] = tile;
            }
        }
        let player_start =
            player_start.ok_or_else(|| ParseError(format!("{}: no player start 'P'", file.id)))?;
        Ok(Level {
            id: file.id,
            name: file.name,
            width,
            height,
            tiles,
            player_start,
            spawns,
            defs,
        })
    }

    /// Tile at a tile coordinate. Outside the level: solid to the left and right (walls), empty
    /// above and below (so you can jump off the top and fall into pits).
    pub fn tile(&self, col: i32, row: i32) -> Tile {
        if col < 0 || col >= self.width {
            return Tile::Solid;
        }
        if row < 0 || row >= self.height {
            return Tile::Empty;
        }
        self.tiles[(row * self.width + col) as usize]
    }

    pub fn width_px(&self) -> i32 {
        self.width * TILE_PX
    }

    pub fn height_px(&self) -> i32 {
        self.height * TILE_PX
    }
}

impl WorldQuery for Level {
    fn solid_at(&self, px: i32, py: i32) -> bool {
        self.tile(px.div_euclid(TILE_PX), py.div_euclid(TILE_PX)) == Tile::Solid
    }

    fn floor_at(&self, px: i32, py: i32) -> bool {
        matches!(
            self.tile(px.div_euclid(TILE_PX), py.div_euclid(TILE_PX)),
            Tile::Solid | Tile::Platform
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFS: &str = r#"{ "walker": (script: "walker", w: 14, h: 14) }"#;

    #[test]
    fn parses_grid_and_legend() {
        let lvl = Level::parse(
            "(id: \"t\", name: \"T\", legend: {'w': \"walker\"}, rows: [
                \"....G\",
                \"P.w=.\",
                \"#####\",
            ])",
            DEFS,
        )
        .unwrap();
        assert_eq!((lvl.width, lvl.height), (5, 3));
        assert_eq!(lvl.player_start, (0, 1));
        assert_eq!(lvl.spawns, vec![("walker".to_string(), 2, 1)]);
        assert_eq!(lvl.tile(3, 1), Tile::Platform);
        assert_eq!(lvl.tile(4, 0), Tile::Goal);
        assert_eq!(lvl.tile(-1, 0), Tile::Solid);
        assert_eq!(lvl.tile(0, 99), Tile::Empty);
        assert!(lvl.solid_at(5, 40));
        assert!(lvl.floor_at(50, 16));
    }

    #[test]
    fn rejects_unknown_characters_and_missing_start() {
        assert!(Level::parse(r#"(id: "t", name: "T", rows: ["P?"])"#, DEFS).is_err());
        assert!(Level::parse("(id: \"t\", name: \"T\", rows: [\"##\"])", DEFS).is_err());
    }
}
