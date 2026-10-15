# Running the MVP

This checkpoint includes the simulation, local daemon and CLI, eight starter genomes, native viewer, inspector, archive, phylogeny, charts, highlights and recorded PNG export. The designer, preset first-generation survival acceptance, million-tick ecological/evolutionary acceptance, coverage target and release packaging remain open in `PLAN.md`. This is not the completed v1 specification.

## Build and start

Validated on Apple M4 Pro, macOS 15.3.1. Install a native C/C++ linker and Rust through rustup. On macOS, Apple's developer tools are available through `xcode-select --install`. `rust-toolchain.toml` selects Rust 1.85.1, rustfmt and Clippy; dependencies are locked. Downloads happen during the build. The running binaries require no network, accounts, downloaded models or textures.

```sh
make setup
make check
make run
```

`make run` creates a unique scratch world, starts its local daemon and opens the viewer. Closing the viewer stops and saves that daemon. Its printed world path remains available; existing worlds are never replaced.

Linux needs a C linker, X11 development libraries and a working graphics driver. Windows needs the MSVC C++ build tools and Rust MSVC toolchain; build with `cargo build --workspace --release --locked`, then use the commands below with `.exe` suffixes. Linux/Windows runtime validation is pending. Unix convenience scripts target macOS/Linux; the binaries use local Unix sockets or Windows named pipes.

## A persistent world

In one terminal:

```sh
target/release/bestiary world new --seed 42 --out worlds/my-world.db
target/release/bestiary world run worlds/my-world.db --speed 10 --snapshot-every 100
```

In another:

```sh
target/release/bestiary-viewer worlds/my-world.db
```

The viewer attaches automatically when that world's daemon is running, otherwise it opens the saved world read-only. Closing a separately launched viewer leaves the daemon running. Ctrl-C in the daemon terminal saves and stops it. `--speed max` runs as quickly as possible; `--until 10000` stops at an absolute tick. Use a terminal session manager for unattended runs; do not start two writers for one world.

The world advances from physics-measured traits. The inspector can run fixed-step body physics or recorded combine poses. This boundary is documented in `crates/bestiary-world/README.md`.

## Viewer controls

- Right-drag to orbit, scroll to zoom, WASD to move the free camera. Free, Follow and Director modes are in the bottom bar.
- Select a living creature and open **Body & brain**. Live physics shows muscle activation; recorded Sprint/Strike modes allow scrubbing. Collision shapes expose the physical geometry.
- Overlays show temperature, moisture, biomass, pheromones, species and recent predation. Archive selectors choose two displayed descriptor axes and fixed bins for the others; hover occupied cells for recorded bodies.
- Phylogeny, charts and highlights use stored history. New worlds need daemon steps and snapshots to accumulate data. Historical scrubbing covers recorded trials and sampled data; full world-state rewind is not implemented.
- Pause/speed controls act on the daemon. Offline snapshots do not advance.

Select a recorded replay, set **From** and **For**, then **Export camera clip**. It writes a fresh directory under `exports/`: 30 fps camera PNGs, metadata and an ffmpeg command sidecar. The world snapshot stays fixed, interface panels are omitted, and required recordings must finish loading first. ffmpeg is optional and is not installed or invoked by the app.

## CLI examples

```sh
mkdir -p exports
target/release/bestiary world status worlds/my-world.db --json
target/release/bestiary combine grazer --json --replay exports/grazer.rpl
target/release/bestiary genome validate grazer
target/release/bestiary genome show grazer --json
target/release/bestiary archive ls worlds/my-world.db --export exports/archive
target/release/bestiary phylo worlds/my-world.db --format dot
target/release/bestiary replay GENOME_HASH-0 --world worlds/my-world.db --export exports/my-clip
```

Replace `GENOME_HASH-0` with a replay ID from the archive export. Replay-file export also requires `--world` to find and verify matching recorded anatomy. The sibling viewer executable and a GPU are required. Existing output directories are rejected; success requires every expected PNG. The recorded trial is read from data rather than inferred from its suffix. Genome commands accept a zoo name, binary genome file or share string. `bestiary --help` lists commands.

## Configuration and reproducibility

`config/world.ron` documents simulation, muscle, metabolism, ecology, reproduction and archive parameters. Its configuration is copied into each new world; editing the file does not change existing worlds. Pass `--config PATH` to `world new` or `combine` for another configuration. `config/demo.ron` is a development diversification scenario; `config/trophic.ron` is an experimental diagnostic, not a proven balanced ecosystem.

`config/starter.ron` contains authored founder anatomy and motor weights, embedded at build time and copied into genomes. `config/viewer.ron` controls appearance only; pass `--appearance config/viewer.ron` to apply edits without rebuilding. Mutated bodies still face viability and energy-audit gates; walking is not guaranteed for arbitrary genomes.

Bitwise determinism is within one platform and one binary, not across platforms or versions. Preserve the executable, configuration and genome for an experiment; cross-platform reproduction uses recorded poses. Database schema 3 rejects development schemas 1/2, which require their earlier binaries. Compatible-schema worlds with stale physics remain readable for recorded history but cannot resume using stale traits. Start a fresh world after changing physics.

## Verification and remaining work

```sh
make check                     # format, Clippy, workspace tests, release build
scripts/check-viewer-render    # actual GPU panels, local IPC and PNG sequences
python3 scripts/test-run-demo.py # Unix launcher lifecycle regression
make bench                     # combine, 5,000-agent world and viewer gates
```

GPU checks need access to the native graphics service. The viewer gate renders 2880×1920 with 2,020 actual agents, at least 2,000 visible proxies and 20 articulated bodies, the full UI, 120 warmup frames and 600 measured frames, waiting for GPU completion each frame.

MVP measurement on Apple M4 Pro, 24 GiB RAM, macOS 15.3.1 (`make bench`, 2026-10-14): full 12-part nine-trial combine **0.755 seconds** worst of three (budget <2 seconds); 5,000-agent world **12,767 ticks/minute** (budget ≥10,000); viewer **116.47 fps** (budget ≥60). `make check` and the actual GPU panel/IPC/replay-export tests passed. These measurements supersede earlier development results.

`make longrun` currently selects the earlier world-core fixture; it is not the final turnover/trophic/stagnation harness. Ignored evolution tests and other unfinished requirements remain explicit in `PLAN.md`. The full `v0.1.0` tag is reserved until the specification checklist passes.

Worlds, exports, toolchains and generated binaries are ignored by Git. Commit source and `Cargo.lock`, not databases. Benchmark speed is not evidence of long-run ecological stability or open-ended evolution.
