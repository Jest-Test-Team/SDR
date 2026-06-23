# DSP Core (Simulation Track)

Software-only RF/protocol simulation that feeds the same ZMQ endpoint as `edge-gateway`.

## Mode A — Protocol injection (recommended)

```bash
# Terminal 1
cargo run -p control-plane --release

# Terminal 2
python3 dsp-core/scripts/inject_zmq.py --replay-last
```

## Mode B — GNU Radio container (optional)

```bash
docker build -f dsp-core/docker/Dockerfile.gnuradio -t sdr-gnuradio dsp-core
docker run --rm -it --network host sdr-gnuradio
```

See `flowgraphs/README.md` for the offline GFSK chain.
