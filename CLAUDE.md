# Notes for agents

Read `README.md` first. It covers the layout, commands and the originality rules.

## Hard rules (public repo)
- Original content only. Never add code, assets, level layouts, names or data taken from another
  game. Genre mechanics and control feel are fine; specific designs are not.
- Never add ROMs, emulator code, emulator cores, save states, captured frames or raw measurement
  curves. Feel targets arrive from a separate private tool as **ranges only**, and go into
  `content/feel_targets.ron`.
- Never add features that read, import or patch ROMs or ROM hacks.
- Don't use other games' names in code, content, filenames or commit messages. The README's
  positioning paragraph and disclaimer are the only exception.

## Working rules
- The simulation (`crates/core`) must stay deterministic: integers only, no clock, no IO, no
  unordered iteration that affects state.
- Every level ships with a replay that completes it. After changing physics, scripts or levels,
  regenerate with `cargo run --release -p ns-bot -- <level> --write`, then run `scripts/ci.sh`.
- Script errors fail CI. Scripts read top-level constants as `global::NAME`.
