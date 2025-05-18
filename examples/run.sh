#!/usr/bin/env bash
set -euo pipefail

# default: no real-time
RT=false

# parse args
for arg in "$@"; do
  case "$arg" in
    --rt) RT=true ;;
    *) 
      echo "Usage: $0 [--rt]"
      exit 1
      ;;
  esac
done

sudo rm -f ./logs/kbot.log

./examples/set_can.sh

if [ "$RT" = true ]; then
  echo "Building in release mode…"
  cargo build --release

  echo "Launching real-time runtime with chrt…"
  sudo chrt -f 80 \
    ./target/release/runtime \
    --model-path examples/kbot_standing.kinfer \
    --magnitude-factor 0.2 \
    --torque-scale 0.1 \
    --dt 20 \
    --file-logging \
    --torque-enabled
else
  echo "Running under cargo…"
  cargo run --release -- \
    --model-path examples/kbot_standing.kinfer \
    --magnitude-factor 0.2 \
    --torque-scale 0.1 \
    --file-logging \
    --torque-enabled
fi

