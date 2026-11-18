---
name: doctor
description: Report what this machine can do in this repo (build and test, play on desktop, web build, push) and how to fix what's missing. Use when the user asks what works on this machine, sets up a new device, or a tool turns out to be missing.
---

# Doctor

Run `scripts/doctor.sh` (or `scripts/doctor.sh <group>...` for some groups)
and summarize it for the user in a few lines: which kinds of work are
available here, which aren't, and the fix for each missing one. A missing
group is not a problem if the user never does that work on this machine;
don't install anything unless they ask.
