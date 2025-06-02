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

## Troubleshooting
You need to install the onnxruntime binary. Find the appropriate release: https://github.com/microsoft/onnxruntime/releases, unpack it, find the .so file (should look like `libonnxruntime.so.VERSION`, copy it to `/usr/local/lib/`, link the .so file to `libonnxruntime.so.1` and link `libonnxruntime.so.1` to `libonnxruntime.so` and then refresh with `sudo ldconfig`.

When you run `ldconfig -p | grep libonnxruntime`, you should see `libonnxruntime.so => /usr/local/lib/libonnxruntime.so`
