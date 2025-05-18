#!/usr/bin/env bash
set -euo pipefail

# defaults
RT=false
N=1
TIMEOUT=5m  # how long before we send SIGINT

usage(){
  echo "Usage: $0 [--rt] [--n <trials>]"
  echo
  echo "  --rt          : enable real-time (chrt) mode"
  echo "  --n <count>   : number of trials to run (default: $N)"
  exit 1
}

# parse flags
while [[ $# -gt 0 ]]; do
  case "$1" in
    --rt)
      RT=true
      shift
      ;;
    --n)
      if [[ -n "${2-}" && "$2" =~ ^[0-9]+$ ]]; then
        N="$2"
        shift 2
      else
        echo "Error: --n requires a positive integer argument."
        usage
      fi
      ;;
    -h|--help)
      usage
      ;;
    *)
      echo "Unknown option: $1"
      usage
      ;;
  esac
done

# bring up CAN bus once

# run N trials
for trial in $(seq 1 "$N"); do
  echo
  echo "=== Trial $trial of $N ==="

  ./examples/set_can.sh
  # clean out the old log
  sudo rm -f logs/kbot.log

  if [ "$RT" = true ]; then
    echo "[Trial $trial] Building release…"
    cargo build --release

    echo "[Trial $trial] Running in real-time mode (will SIGINT after $TIMEOUT)…"
    printf '\n' | timeout --signal=SIGINT "$TIMEOUT" sudo chrt -f 80 \
      ./target/release/runtime \
      --model-path examples/kbot_standing.kinfer \
      --magnitude-factor 0.2 \
      --torque-scale 0.1 \
      --dt 20 \
      --file-logging \
      --torque-enabled \
    || true
  else
    echo "[Trial $trial] Running under cargo (will SIGINT after $TIMEOUT)…"
    printf '\n' | timeout --signal=SIGINT "$TIMEOUT" cargo run --release -- \
      --model-path examples/kbot_standing.kinfer \
      --magnitude-factor 0.2 \
      --torque-scale 0.1 \
      --file-logging \
      --torque-enabled \
    || true
  fi

  # rename the log for this trial
  if [ -f logs/kbot.log ]; then
    mv logs/kbot.log logs/kbot_trial${trial}.log
    echo "[Trial $trial] Saved log → logs/kbot_trial${trial}.log"
  else
    echo "[Trial $trial] No logs/kbot.log found to rename."
  fi

  sleep 20
done

echo
echo "All $N trials complete."

