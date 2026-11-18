#!/usr/bin/env bash
# What this machine can do in this repo, grouped by kind of work.
#   scripts/doctor.sh             # every group; a report, always exits 0
#   scripts/doctor.sh web push    # just these; exits 1 if any check fails
# Groups: build play web push. scripts/ci.sh needs build and web.
set -u
cd "$(dirname "$0")/.."

all="build play web push"
failed=0

if [ -t 1 ]; then G=$'\033[32m' R=$'\033[31m' N=$'\033[0m'; else G= R= N=; fi
ok() { printf '  %s✓%s %s\n' "$G" "$N" "$1"; }
bad() { printf '  %s✗%s %s\n      → %s\n' "$R" "$N" "$1" "$2"; failed=1; }
have() { command -v "$1" >/dev/null 2>&1; }
pkg() { if [ "$(uname)" = Darwin ]; then echo "brew install $1"; else echo "sudo dnf install $1"; fi; }
tool() { if have "$1"; then ok "$1"; else bad "$1 not found" "$2"; fi; }

build() {
	echo "build: compile, test, format and lint (cargo test, the bot, the ns CLI)"
	tool cargo "install Rust: https://rustup.rs"
	if cargo fmt --version >/dev/null 2>&1; then ok "cargo fmt"; else bad "cargo fmt missing" "rustup component add rustfmt"; fi
	if cargo clippy --version >/dev/null 2>&1; then ok "cargo clippy"; else bad "cargo clippy missing" "rustup component add clippy"; fi
}

play() {
	echo "play: run the desktop game (cargo run -p ns-frontend)"
	if [ "$(uname)" = Darwin ]; then
		ok "macOS needs no extra libraries"
	elif ! have pkg-config; then
		bad "pkg-config not found" "$(pkg pkgconf)"
	elif pkg-config --exists alsa x11 xi gl; then
		ok "ALSA, X11, Xi and GL development libraries"
	else
		bad "missing ALSA/X11/Xi/GL development libraries" \
			"sudo dnf install alsa-lib-devel libX11-devel libXi-devel mesa-libGL-devel"
	fi
}

web() {
	echo "web: build for the browser and check replays under WebAssembly"
	if rustup target list --installed 2>/dev/null | grep -qx wasm32-unknown-unknown; then
		ok "Rust target wasm32-unknown-unknown"
	else
		bad "Rust target wasm32-unknown-unknown missing" "rustup target add wasm32-unknown-unknown"
	fi
	tool node "$(pkg fnm), then fnm install --lts"
	tool python3 "$(pkg python3) (only to serve web/dist locally)"
}

push() {
	echo "push: push to origin, including CI workflow changes"
	url=$(git remote get-url origin)
	case "$url" in
	https://*)
		if ! have gh; then
			bad "gh not found (origin is https)" "$(pkg gh), then gh auth login"
		elif ! scopes=$(gh auth status 2>&1 | grep -i 'token scopes'); then
			bad "gh not logged in" "gh auth login"
		elif echo "$scopes" | grep -q "'workflow'"; then
			ok "gh logged in, token has the workflow scope"
		else
			bad "gh token lacks the workflow scope (pushing .github/workflows fails)" "gh auth refresh -s workflow"
		fi
		;;
	*)
		if GIT_SSH_COMMAND="ssh -o BatchMode=yes -o ConnectTimeout=5" git ls-remote -q origin HEAD >/dev/null 2>&1; then
			ok "origin reachable ($url)"
		else
			bad "can't reach origin ($url)" "check the ssh key for GitHub"
		fi
		;;
	esac
}

groups="${*:-$all}"
for g in $groups; do
	case " $all " in
	*" $g "*) "$g"; echo ;;
	*) echo "unknown group '$g' (groups: $all)" >&2; exit 2 ;;
	esac
done

[ $# -gt 0 ] && exit $failed
exit 0
