# Offline GFSK telemetry simulation (teaching flowgraph)

This directory holds a GNU Radio Companion flowgraph that exercises:

`Vector Source -> GFSK Mod -> Channel Model (AWGN) -> GFSK Demod -> File Sink`

The bitstream input can be generated from `TelemetryFrame` wire bytes. For fast integration testing without GNU Radio, use:

```bash
./dsp-core/scripts/run_sim.sh --replay-last
```

Hardware SDR sources (RTL-SDR / HackRF) can replace the Vector Source when available.
