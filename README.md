# kinfer-kbot

This project provides the K-Infer runtime for the K-Bot.

## Usage

Set CAN:

```bash
./examples/set_can.sh
```

Run the model runtime:

```bash
cargo run --bin runtime -- \
  --model-path examples/kbot_standing.kinfer \
  --magnitude-factor 1.0 \
  --slowdown-factor 1
```
