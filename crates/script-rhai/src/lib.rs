//! Rhai backend for the script API.
//!
//! Script handlers receive one argument, conventionally `e`, with these members:
//!
//! - Reads (pixels): `left`, `top`, `right`, `bottom` (last pixel inside the hitbox),
//!   `center_x`, `center_y`, `w`, `h`; `player_x`, `player_y` (player centre).
//! - Reads (sub-pixels, 256 per pixel): `x`, `y`, `vx`, `vy`, `player_vx`, `player_vy`.
//! - Reads: `id`, `kind`, `frame`, `on_ground`, `hit_wall_left`, `hit_wall_right`.
//! - Script state: `e.get_var("name")` (0 if unset), `e.set_var("name", value)`.
//! - Commands: `e.set_vx(v)`, `e.set_vy(v)`, `e.emit("event")`, `e.remove()`.
//! - Queries: `e.solid_at(px, py)`, `e.floor_at(px, py)`, `e.rand(n)` (deterministic, 0..n).
//!
//! Handlers: `on_spawn(e)`, `on_tick(e)`, `on_event(e, name)`. All are optional. Top-level code
//! runs before every handler call, so use it only for `const` declarations, which handlers read as
//! `global::NAME`. Nothing else persists between calls: keep state in `set_var`.

use ns_script_api::{Command, Hook, HookInput, SUB, ScriptBackend, WorldQuery, det_rand};
use rhai::{AST, CallFnOptions, Dynamic, Engine, Scope};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

/// Per-call handle given to scripts. Clones share the same buffers.
#[derive(Clone)]
struct Ctx {
    input: Rc<HookInput>,
    world: Arc<dyn WorldQuery>,
    vars: Rc<RefCell<BTreeMap<String, i64>>>,
    cmds: Rc<RefCell<Vec<Command>>>,
    rand_calls: Rc<Cell<u64>>,
}

impl Ctx {
    fn push(&self, c: Command) {
        self.cmds.borrow_mut().push(c);
    }
    fn left(&self) -> i64 {
        i64::from(self.input.me.x.div_euclid(SUB))
    }
    fn top(&self) -> i64 {
        i64::from(self.input.me.y.div_euclid(SUB))
    }
}

/// Saturating conversion from script integers to engine integers.
fn to_i32(v: i64) -> i32 {
    v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

pub struct RhaiBackend {
    engine: Engine,
    scripts: BTreeMap<String, AST>,
}

impl Default for RhaiBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl RhaiBackend {
    pub fn new() -> RhaiBackend {
        let mut engine = Engine::new();
        // Sandbox: no file imports, no eval, no output, bounded work per call.
        engine.set_module_resolver(rhai::module_resolvers::DummyModuleResolver::new());
        engine.disable_symbol("eval");
        engine.on_print(|_| {});
        engine.on_debug(|_, _, _| {});
        engine.set_max_operations(100_000);
        engine.set_max_call_levels(32);
        engine.set_max_expr_depths(64, 32);
        engine.set_max_string_size(1024);
        engine.set_max_array_size(1024);
        engine.set_max_map_size(256);

        engine.register_type_with_name::<Ctx>("Entity");
        engine.register_get("id", |c: &mut Ctx| i64::from(c.input.me.id));
        engine.register_get("kind", |c: &mut Ctx| c.input.me.kind.clone());
        engine.register_get("frame", |c: &mut Ctx| c.input.frame as i64);
        engine.register_get("x", |c: &mut Ctx| i64::from(c.input.me.x));
        engine.register_get("y", |c: &mut Ctx| i64::from(c.input.me.y));
        engine.register_get("vx", |c: &mut Ctx| i64::from(c.input.me.vx));
        engine.register_get("vy", |c: &mut Ctx| i64::from(c.input.me.vy));
        engine.register_get("w", |c: &mut Ctx| i64::from(c.input.me.w));
        engine.register_get("h", |c: &mut Ctx| i64::from(c.input.me.h));
        engine.register_get("left", |c: &mut Ctx| c.left());
        engine.register_get("top", |c: &mut Ctx| c.top());
        engine.register_get("right", |c: &mut Ctx| {
            c.left() + i64::from(c.input.me.w) - 1
        });
        engine.register_get("bottom", |c: &mut Ctx| {
            c.top() + i64::from(c.input.me.h) - 1
        });
        engine.register_get("center_x", |c: &mut Ctx| {
            c.left() + i64::from(c.input.me.w) / 2
        });
        engine.register_get("center_y", |c: &mut Ctx| {
            c.top() + i64::from(c.input.me.h) / 2
        });
        engine.register_get("on_ground", |c: &mut Ctx| c.input.me.on_ground);
        engine.register_get("hit_wall_left", |c: &mut Ctx| c.input.me.hit_wall_left);
        engine.register_get("hit_wall_right", |c: &mut Ctx| c.input.me.hit_wall_right);
        engine.register_get("player_x", |c: &mut Ctx| {
            let p = &c.input.player;
            i64::from(p.x.div_euclid(SUB) + p.w / 2)
        });
        engine.register_get("player_y", |c: &mut Ctx| {
            let p = &c.input.player;
            i64::from(p.y.div_euclid(SUB) + p.h / 2)
        });
        engine.register_get("player_vx", |c: &mut Ctx| i64::from(c.input.player.vx));
        engine.register_get("player_vy", |c: &mut Ctx| i64::from(c.input.player.vy));

        engine.register_fn("get_var", |c: &mut Ctx, name: &str| {
            c.vars.borrow().get(name).copied().unwrap_or(0)
        });
        engine.register_fn("set_var", |c: &mut Ctx, name: &str, v: i64| {
            c.vars.borrow_mut().insert(name.to_string(), v);
            c.push(Command::SetVar(name.to_string(), v));
        });
        engine.register_fn("set_vx", |c: &mut Ctx, v: i64| {
            c.push(Command::SetVx(to_i32(v)))
        });
        engine.register_fn("set_vy", |c: &mut Ctx, v: i64| {
            c.push(Command::SetVy(to_i32(v)))
        });
        engine.register_fn("emit", |c: &mut Ctx, name: &str| {
            c.push(Command::Emit(name.to_string()))
        });
        engine.register_fn("remove", |c: &mut Ctx| c.push(Command::Remove));
        engine.register_fn("solid_at", |c: &mut Ctx, px: i64, py: i64| {
            c.world.solid_at(to_i32(px), to_i32(py))
        });
        engine.register_fn("floor_at", |c: &mut Ctx, px: i64, py: i64| {
            c.world.floor_at(to_i32(px), to_i32(py))
        });
        engine.register_fn("rand", |c: &mut Ctx, n: i64| {
            if n <= 0 {
                return 0;
            }
            let k = c.rand_calls.get();
            c.rand_calls.set(k + 1);
            let r = det_rand(c.input.seed, c.input.frame, c.input.me.id, k);
            (r % n as u64) as i64
        });

        RhaiBackend {
            engine,
            scripts: BTreeMap::new(),
        }
    }

    /// Compile and register a script under `id` (e.g. `"walker"`).
    pub fn load(&mut self, id: &str, source: &str) -> Result<(), String> {
        let ast = self
            .engine
            .compile(source)
            .map_err(|e| format!("{id}.rhai: {e}"))?;
        self.scripts.insert(id.to_string(), ast);
        Ok(())
    }

    pub fn has(&self, id: &str) -> bool {
        self.scripts.contains_key(id)
    }
}

impl ScriptBackend for RhaiBackend {
    fn call(
        &self,
        script_id: &str,
        hook: &Hook,
        input: &HookInput,
        world: Arc<dyn WorldQuery>,
    ) -> Result<Vec<Command>, String> {
        let ast = self
            .scripts
            .get(script_id)
            .ok_or_else(|| format!("no script named '{script_id}'"))?;
        let name = hook.handler_name();
        let arity = if matches!(hook, Hook::Event(_)) { 2 } else { 1 };
        if !ast
            .iter_functions()
            .any(|f| f.name == name && f.params.len() == arity)
        {
            return Ok(Vec::new());
        }
        let ctx = Ctx {
            input: Rc::new(input.clone()),
            world,
            vars: Rc::new(RefCell::new(input.me.vars.clone())),
            cmds: Rc::new(RefCell::new(Vec::new())),
            rand_calls: Rc::new(Cell::new(0)),
        };
        let opts = CallFnOptions::new().eval_ast(true).rewind_scope(true);
        let mut scope = Scope::new();
        let result = match hook {
            Hook::Event(ev) => self.engine.call_fn_with_options::<Dynamic>(
                opts,
                &mut scope,
                ast,
                name,
                (ctx.clone(), ev.clone()),
            ),
            _ => self.engine.call_fn_with_options::<Dynamic>(
                opts,
                &mut scope,
                ast,
                name,
                (ctx.clone(),),
            ),
        };
        result.map(drop).map_err(|e| e.to_string())?;
        Ok(ctx.cmds.take())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ns_script_api::{EntityView, PlayerView};

    struct Flat;
    impl WorldQuery for Flat {
        fn solid_at(&self, _: i32, py: i32) -> bool {
            py >= 100
        }
        fn floor_at(&self, px: i32, py: i32) -> bool {
            self.solid_at(px, py)
        }
    }

    fn input() -> HookInput {
        HookInput {
            frame: 7,
            seed: 1,
            me: EntityView {
                id: 3,
                x: 50 * SUB,
                y: 86 * SUB,
                w: 14,
                h: 14,
                ..Default::default()
            },
            player: PlayerView::default(),
        }
    }

    fn run(src: &str, hook: Hook) -> Result<Vec<Command>, String> {
        let mut b = RhaiBackend::new();
        b.load("t", src)?;
        b.call("t", &hook, &input(), Arc::new(Flat))
    }

    #[test]
    fn handlers_produce_commands() {
        let cmds = run(
            r#"fn on_tick(e) {
                   let n = e.get_var("n") + 1;
                   e.set_var("n", n);
                   if e.floor_at(e.center_x, e.bottom + 1) { e.set_vx(-n * 2); }
                   e.emit("hello");
               }"#,
            Hook::Tick,
        )
        .unwrap();
        assert_eq!(
            cmds,
            vec![
                Command::SetVar("n".into(), 1),
                Command::SetVx(-2),
                Command::Emit("hello".into())
            ]
        );
    }

    #[test]
    fn missing_handler_is_fine_and_events_get_a_name() {
        assert_eq!(run("fn on_tick(e) {}", Hook::Spawn).unwrap(), vec![]);
        let cmds = run(
            r#"fn on_event(e, name) { if name == "boom" { e.remove(); } }"#,
            Hook::Event("boom".into()),
        )
        .unwrap();
        assert_eq!(cmds, vec![Command::Remove]);
    }

    #[test]
    fn top_level_constants_are_visible() {
        let cmds = run(
            "const SPEED = 5; fn on_tick(e) { e.set_vx(global::SPEED); }",
            Hook::Tick,
        )
        .unwrap();
        assert_eq!(cmds, vec![Command::SetVx(5)]);
    }

    #[test]
    fn sandboxed() {
        assert!(
            run("fn on_tick(e) { loop {} }", Hook::Tick).is_err(),
            "infinite loop is cut off"
        );
        assert!(
            run(r#"fn on_tick(e) { import "x" as y; }"#, Hook::Tick).is_err(),
            "no imports"
        );
        assert!(
            run("fn on_tick(e) { let x = 1.5; }", Hook::Tick).is_err(),
            "no floats"
        );
    }

    #[test]
    fn rand_is_deterministic() {
        let src = "fn on_tick(e) { e.set_vx(e.rand(1000)); e.set_vy(e.rand(1000)); }";
        let a = run(src, Hook::Tick).unwrap();
        assert_eq!(a, run(src, Hook::Tick).unwrap());
        assert_ne!(
            a[0],
            Command::SetVx(match a[1] {
                Command::SetVy(v) => v,
                _ => -1,
            })
        );
    }
}
