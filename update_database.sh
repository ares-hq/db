#!/usr/bin/env bash
# Runs the ARES database pipeline on a loop, rebuilding when `main` moves.
#
#   chmod +x ./update_database.sh
#   nohup ./update_database.sh > monitor.log 2>&1 &
#
# The pipeline is one-shot: each pass fetches, solves and upserts, then exits.
set -euo pipefail

readonly BRANCH="main"
# Seconds between passes, measured from the end of the previous one.
readonly RUN_INTERVAL=300

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

log() { echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*"; }

binary_path() {
    local dir
    dir=$(cargo metadata --no-deps --format-version 1 \
        | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')
    echo "${dir:-target}/release/db"
}

build() {
    log "Building..."
    cargo build --release --bin db
    log "Build complete"
}

# The pipeline exiting non-zero is a bad run, not a reason to stop looping.
run_pipeline() {
    local binary
    binary=$(binary_path)
    log "Running pipeline"
    if "$binary" "$@"; then
        log "Pipeline complete"
    else
        log "ERROR: pipeline exited $?"
    fi
}

check_for_updates() {
    git fetch origin "$BRANCH" --quiet || { log "WARNING: git fetch failed"; return; }

    local local_hash remote_hash
    local_hash=$(git rev-parse HEAD)
    remote_hash=$(git rev-parse "origin/$BRANCH")
    [ "$local_hash" = "$remote_hash" ] && return

    log "Update detected: $local_hash -> $remote_hash"
    git reset --hard "origin/$BRANCH" --quiet || { log "ERROR: reset failed"; return; }
    echo updated
}

command -v cargo >/dev/null || {
    echo "Rust/Cargo not found. Install it from https://rustup.rs/"
    exit 1
}

build
run_pipeline "$@"

while true; do
    log "Sleeping ${RUN_INTERVAL}s"
    sleep "$RUN_INTERVAL"

    [ -n "$(check_for_updates)" ] && build
    run_pipeline "$@"
done
