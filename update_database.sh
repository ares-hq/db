#!/bin/bash

SCRIPT_NAME="manage_database.py"
LOG_FILE="update.log"
VENV_DIR=".venv"
BRANCH="main"

install_pip() {
    if ! command -v pip3 &> /dev/null; then
        echo "pip3 not found. Installing pip3..."
        sudo apt update
        sudo apt install -y python3-pip
    else
        echo "✅ pip3 is already installed."
    fi
}

create_venv() {
    if [ ! -d "$VENV_DIR" ]; then
        echo "🧪 Creating virtual environment..."
        python3 -m venv $VENV_DIR
    fi
    echo "✅ Activating virtual environment..."
    source $VENV_DIR/bin/activate
}

install_requirements() {
    if [ -f "requirements.txt" ]; then
        echo "📦 Installing dependencies from requirements.txt..."
        pip install -r requirements.txt
    else
        echo "⚠️ requirements.txt not found. Skipping installation."
    fi
}

stop_process() {
    local PIDS
    PIDS=$(pgrep -f "$SCRIPT_NAME")
    if [ -n "$PIDS" ]; then
        echo "🛑 Stopping running process: $PIDS"
        kill $PIDS
        wait $PIDS 2>/dev/null
    fi
}

start_process() {
    stop_process
    echo "🚀 Running $SCRIPT_NAME..."
    nohup python3 $SCRIPT_NAME > $LOG_FILE 2>&1 &
    echo "✅ Script started in background."
}

install_pip
create_venv
install_requirements
start_process

while true; do
    echo "[$(date)] 🔍 Checking for Git updates..."

    git fetch origin $BRANCH > fetch_output.log 2>&1
    if [ $? -ne 0 ]; then
        echo "❌ Git fetch failed! See fetch_output.log"
        sleep 300
        continue
    fi

    LOCAL=$(git rev-parse HEAD)
    REMOTE=$(git rev-parse origin/$BRANCH)

    echo "🔍 LOCAL: $LOCAL"
    echo "🔍 REMOTE: $REMOTE"

    if [ "$LOCAL" != "$REMOTE" ]; then
        echo "📥 Changes detected. Pulling latest from $BRANCH..."
        git reset --hard origin/$BRANCH > pull_output.log 2>&1

        if [ $? -ne 0 ]; then
            echo "❌ Git pull failed! See pull_output.log"
            sleep 300
            continue
        fi

        echo "📦 Updating dependencies..."
        install_requirements
    else
        echo "✅ No updates found."
    fi

    echo "🔄 Restarting process..."
    start_process
    echo "✅ Process complete..."

    echo "⏱ Sleeping for 5 minutes..."
    sleep 300
done