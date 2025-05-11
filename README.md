# kinfer-kbot

This project provides the K-Infer runtime for the K-Bot.

## Usage

Run the model runtime:

```bash
cargo run --bin runtime -- \
  --model-path /path/to/model.kinfer \
  --magnitude-factor 1.0 \
  --slowdown-factor 1
```
