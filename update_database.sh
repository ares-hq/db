#!/bin/bash

BINARY_NAME="db"
LOG_FILE="update.log"
BRANCH="main"

check_rust() {
    if ! command -v cargo &> /dev/null; then
        echo "Rust/Cargo not found. Please install from https://rustup.rs/"
        exit 1
    else
        echo "Cargo is installed."
    fi
}

build_binary() {
    echo "Building Rust binary..."
    cargo build --release
    if [ $? -ne 0 ]; then
        echo "Build failed!"
        exit 1
    fi
    echo "Build complete"
}

stop_process() {
    local PIDS
    PIDS=$(pgrep -f "target/release/$BINARY_NAME")
    if [ -n "$PIDS" ]; then
        echo "Stopping running process: $PIDS"
        kill $PIDS
        wait $PIDS 2>/dev/null
    fi
}

start_process() {
    stop_process
    echo "Running $BINARY_NAME..."
    ./target/release/$BINARY_NAME
    echo "Script finished."
}

check_rust
build_binary
start_process

while true; do
    echo "[$(date)] Checking for Git updates..."

    git fetch origin $BRANCH > fetch_output.log 2>&1
    if [ $? -ne 0 ]; then
        echo "Git fetch failed! See fetch_output.log"
        sleep 150
        continue
    fi

    LOCAL=$(git rev-parse HEAD)
    REMOTE=$(git rev-parse origin/$BRANCH)

    echo "LOCAL: $LOCAL"
    echo "REMOTE: $REMOTE"

    if [ "$LOCAL" != "$REMOTE" ]; then
        echo "Changes detected. Pulling latest from $BRANCH..."
        git reset --hard origin/$BRANCH > pull_output.log 2>&1

        if [ $? -ne 0 ]; then
            echo "Git pull failed! See pull_output.log"
            sleep 300
            continue
        fi

        echo "Rebuilding binary..."
        build_binary
    else
        echo "No updates found."
    fi

    echo "Restarting process..."
    start_process
    echo "Process complete..."

    echo "Sleeping for 2.5 minutes..."
    sleep 300
done