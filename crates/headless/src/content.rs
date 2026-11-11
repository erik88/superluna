use ns_core::{Feel, Level};
use ns_script_rhai::RhaiBackend;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// All game content, loaded from a content directory:
/// `feel.ron`, `entities.ron`, `levels/*.ron`, `scripts/*.rhai`.
pub struct Content {
    pub dir: PathBuf,
    pub feel: Arc<Feel>,
    pub levels: BTreeMap<String, Arc<Level>>,
    pub scripts: RhaiBackend,
}

fn read(p: &Path) -> Result<String, String> {
    std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))
}

fn files_with_ext(dir: &Path, ext: &str) -> Result<Vec<PathBuf>, String> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == ext))
        .collect();
    out.sort();
    Ok(out)
}

impl Content {
    /// Build from in-memory sources (used by the web build, which has no file access).
    pub fn from_sources(
        feel: &str,
        entities: &str,
        levels: &[&str],
        scripts: &[(&str, &str)],
    ) -> Result<Content, String> {
        let feel = Arc::new(Feel::from_ron(feel)?);
        let mut backend = RhaiBackend::new();
        for (id, src) in scripts {
            backend.load(id, src)?;
        }
        let mut map = BTreeMap::new();
        for src in levels {
            let level = Level::parse(src, entities).map_err(|e| e.to_string())?;
            map.insert(level.id.clone(), Arc::new(level));
        }
        Ok(Content {
            dir: PathBuf::new(),
            feel,
            levels: map,
            scripts: backend,
        })
    }

    pub fn load(dir: impl AsRef<Path>) -> Result<Content, String> {
        let dir = dir.as_ref().to_path_buf();
        let feel = Arc::new(Feel::from_ron(&read(&dir.join("feel.ron"))?)?);
        let entities = read(&dir.join("entities.ron"))?;

        let mut scripts = RhaiBackend::new();
        for p in files_with_ext(&dir.join("scripts"), "rhai")? {
            let id = p.file_stem().unwrap().to_string_lossy().to_string();
            scripts.load(&id, &read(&p)?)?;
        }

        let mut levels = BTreeMap::new();
        for p in files_with_ext(&dir.join("levels"), "ron")? {
            let level =
                Level::parse(&read(&p)?, &entities).map_err(|e| format!("{}: {e}", p.display()))?;
            for def in level.defs.values() {
                if !scripts.has(&def.script) {
                    return Err(format!(
                        "{}: entity uses missing script '{}'",
                        level.id, def.script
                    ));
                }
            }
            levels.insert(level.id.clone(), Arc::new(level));
        }
        Ok(Content {
            dir,
            feel,
            levels,
            scripts,
        })
    }

    pub fn level(&self, id: &str) -> Result<Arc<Level>, String> {
        self.levels.get(id).cloned().ok_or_else(|| {
            let known: Vec<&str> = self.levels.keys().map(String::as_str).collect();
            format!("unknown level '{id}' (known: {})", known.join(", "))
        })
    }

    /// Where a level's verified replay lives.
    pub fn replay_path(&self, level_id: &str) -> PathBuf {
        self.dir.join("levels").join(format!("{level_id}.replay"))
    }
}
