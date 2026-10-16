# Running bestiary

The native viewer, creature designer, eight presets, local daemon, CLI and recorded
clip export are implemented. This remains an untagged v1 candidate: the fresh
million-tick ecological and evolutionary targets have not all passed. See
[Measured acceptance](#measured-acceptance) and [PLAN.md](PLAN.md) before treating a
run as release evidence. No `v0.1.0` release tag is permitted while those items are open.

## Build on the host platform

Rust 1.85.1, rustfmt and Clippy are selected by `rust-toolchain.toml`. Direct crate
versions and `Cargo.lock` are pinned. Install Rust through rustup and a native C/C++
linker. Python 3.9 or newer and Git are required for the validation and packaging
scripts; Unix workflows also use Make and a POSIX shell. Build-time dependency
downloads are allowed; the running application uses
no internet, accounts, telemetry, remote services or downloaded assets. Terrain,
bodies and textures are procedural. Fonts and shaders are included in the build.

### macOS

Install Apple's command-line developer tools, then build from the repository root:

```sh
xcode-select --install
rustup toolchain install 1.85.1 --component rustfmt --component clippy
make setup
make check
make dist
```

The developer-tools installer is needed only when those tools are absent. The
validated host is Apple M4 Pro, arm64, 24 GiB RAM, macOS 15.3.1 (24D70). The viewer
requires access to the native graphics/window service, including for offscreen tests.

### Linux

Use a C/C++ compiler, linker, `pkg-config`, X11 development libraries and a working
Vulkan-capable graphics driver. For a Debian/Ubuntu development machine, an example
prerequisite installation is:

```sh
sudo apt-get install build-essential python3 git pkg-config libx11-dev libxrandr-dev libxi-dev libxcursor-dev libxinerama-dev libxkbcommon-dev libudev-dev
rustup toolchain install 1.85.1 --component rustfmt --component clippy
make setup
make check
make dist
```

Run the viewer in an X11 session or through XWayland. The daemon and CLI's ordinary
world commands do not require a display. This build uses Bevy's X11 backend; no
Wayland-only build is claimed. Linux build/runtime validation has not been performed
on this macOS host.

### Windows

Install Visual Studio Build Tools with **Desktop development with C++**, the MSVC
linker and a Windows SDK. Install Rust's MSVC toolchain, Git and Python 3.9 or newer.
In a developer PowerShell
terminal at the repository root:

```powershell
rustup toolchain install 1.85.1 --profile minimal --component rustfmt --component clippy
cargo fetch --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --release --locked
python scripts/test-dist.py
python scripts/dist
```

Use `.exe` and Windows path separators in the commands below. The Unix `make run`,
benchmark and long-run shell wrappers require a POSIX shell; the native binaries
use Windows named pipes for IPC. Windows build/runtime validation has not been
performed on this macOS host.

`./scripts/cargo` uses the repository-local ignored `.tools/` toolchain when present,
otherwise the installed Cargo. Keep all three release executables together: the
CLI launches its sibling viewer for PNG replay export. `make dist` builds the host
binaries; it does not cross-compile or install them system-wide.

The package contains the three executables, the human-editable `config/*.ron`
files, `README.md`, `RUNBOOK.md`, `TUNING.md`, `ASSUMPTIONS.md`, `SPEC.md`,
`AGENTS.md`, `PLAN.md`, `Cargo.lock`, `manifest.json` and `SHA256SUMS`. Defaults
remain embedded in the executables; configuration overrides work as documented
below. The manifest records the source commit, dirty status, host platform,
version and each binary's SHA-256. Source provenance is captured at packaging
time, so build immediately before packaging. This candidate is explicitly marked
`unreleased`; packaging never creates a release tag.

Verify the copied files with `python3 scripts/dist --verify dist` (use `python`
on Windows). Packaging verifies a temporary staged directory before replacing
generated outputs, preserves unrelated files, and refuses to discard an edited
shipped configuration or overwrite an unrelated file at a generated path.

## Start a scratch demo

On macOS/Linux:

```sh
make run
```

The launcher creates a unique directory under `worlds/`, prints the world path,
starts its daemon and opens the viewer. Closing this launched viewer saves and
stops that daemon. The world and daemon log remain in the printed directory.
`make run` uses the baseline configuration; use the explicit demo configuration
below to reproduce the recorded diversification experiment.

## Create and run a persistent world

Run these commands from the repository root, choosing a new output path:

```sh
mkdir -p worlds exports
./target/release/bestiary world new --seed 42 --preset temperate --population 24 --config config/demo.ron --out worlds/my-world.db
./target/release/bestiary world run worlds/my-world.db --speed 10 --snapshot-every 100
```

In a second terminal:

```sh
./target/release/bestiary world attach worlds/my-world.db
./target/release/bestiary-viewer worlds/my-world.db
./target/release/bestiary world status worlds/my-world.db --json
```

`world attach` prints the IPC endpoint; the viewer finds it automatically. A running
daemon provides live snapshots. Without a daemon, the same viewer command opens
the database read-only. Closing a separately started viewer leaves the daemon
running. Ctrl-C in the daemon terminal saves its final snapshot and stops it.
A world permits one writer; multiple viewers may inspect it.

`--speed N` is a pacing multiplier: each completed tick sleeps for `0.1 / N`
seconds, so the ceiling before simulation work is about `10 × N` ticks/second.
It does not change the fixed simulated time per tick. `--speed max` removes pacing.
`--until N` is an absolute world tick, so a saved world at tick 5,000 resumes to tick 10,000 with
`--until 10000`. `--snapshot-every N` saves every N ticks and must be positive.
The standalone daemon also accepts `bestiary-daemon WORLD.db [UNTIL_TICK]`, using
maximum speed and a 100-tick snapshot interval.

Scenario names are `temperate`, `islands` and `harsh`. `temperate` leaves the selected
configuration unchanged. `islands` sets water level 0.1 and terrain frequency 2;
`harsh` halves solar input and sets seasonal amplitude 0.7. `--config` loads a full
RON configuration before those scenario overrides. Configuration is recorded in
the new world; editing a RON file does not change an existing world.

## Leave a run going for days

Run the daemon without the viewer in a persistent terminal session such as tmux,
or a dedicated terminal on Windows. Keep the machine awake and retain the database
on a local writable disk. For example, after creating the world:

```sh
tmux new -s bestiary
./target/release/bestiary world run worlds/my-world.db --speed max --snapshot-every 10000
```

Detach from tmux with Ctrl-B then D; return with `tmux attach -t bestiary`. Omit
`--until` for an unattended run, or set it for a reproducible horizon. Check progress
with `world status --json` from another terminal, and attach/detach the viewer as
needed. Stop with Ctrl-C before copying the database for a standalone backup;
SQLite may have active `-wal` and `-shm` sidecars while it is open. A stopped world
can resume with the same command. The latest saved snapshot is the restart point
after an interrupted process; a larger snapshot interval increases potential lost
work. Physics evaluations for new genomes can make tick throughput uneven.

The world runs offline. Local sockets/named pipes are process communication, with
no network listener. Do not infer ecosystem persistence from elapsed wall time:
use population, species, coverage and conservation evidence over recorded ticks.

## Viewer controls and recorded history

- Right-drag to orbit, scroll to zoom, and WASD to move the free world camera.
  Free, Follow and Director camera modes are in the bottom bar. Director follows
  detected events; it does not author selection pressure.
- Select a living creature, then **Open body & brain**. The inspector shows its
  actual genome, health, energy, age, lineage, brains and measured traits.
  Historical or never-born genomes are labeled separately from living creatures.
- **Live physics** runs an isolated arena and displays measured muscle activation.
  Recorded Sprint/Strike modes scrub saved poses; those recordings have no
  activation channel, so activation is not invented. **Collision shapes** exposes
  the physical geometry beneath the procedural anatomy.
- World overlays show biomass, temperature, moisture, pheromones, species and
  predation. The archive selects two descriptor axes and fixed bins for the others;
  hover occupied cells for recorded bodies, then open a genome in the inspector.
- Phylogeny branches, sampled charts and event highlights use stored data. A fresh
  world needs ticks and snapshots to accumulate history. Historical scrubbing
  covers recorded combine trials and sampled statistics; full world-state rewind
  is an explicit v1 deviation. Offline world snapshots do not advance.
- Pause and speed controls affect the live daemon. Inspector and designer arena
  playback are separate from world time.

Choose an initial panel with `--panel world|inspector|designer|archive|phylogeny|charts|highlights`.
Use `--colliders` for collision geometry or
`--appearance config/viewer.ron` for an appearance override.

## Design, combine and release a creature

```sh
./target/release/bestiary-viewer worlds/my-world.db --panel designer
```

1. Load a zoo preset or **Copy inspected creature**. The editable draft is separate
   from the world; changing it cannot rewrite an existing creature or cached trait.
2. In **Bones and recursion**, select a bone gene, add/remove connected bones, set
   box/capsule shape, dimensions, density, color and recursion count. Repeated
   instances inherit edits to their shared gene.
3. In **Joints and attachment points**, connect a child bone, select fixed, hinge,
   universal or ball constraints, and edit orientation, angular limits, subtree
   scale, reflection and terminal-only attachment. Drag the 3D attachment handles,
   or select a surface face and its two free coordinates in the panel.
4. Shift-click an origin and insertion on the 3D body to draw a muscle, or choose
   expanded part indices and **Connect muscle**. Endpoints must be on distinct
   reachable parts within `genome.max_span` joints. Set cross-section and material
   scales, or remove/redraw a muscle to change its span. Attach sensors and edit
   life-history, resistance, pheromone and mate-preference genes in their panels.
5. Read **Validity**: expanded part count, mass, adult basal-cost estimate, maximum
   isometric holding cost and any invalid values or clipped expansion branches.
   Actual posture/work costs require the combine. Invalid values remain visible;
   missing actuated-axis leverage is supplied by the genesis guarantee.
6. Press **Run combine**. It runs the nine physical trials in the background and
   shows the trait vector, viability, audit result and Sprint recording. **Live
   muscle test** shows actual activation; **Edit body** returns to the rest pose.
   Any genome edit invalidates its previous evaluation, and stale worker results
   cannot authorize a release.
7. With this world's daemon running, enter the founding count and world X/Z location,
   then press **Release into running world**. Release requires a current viable
   combine and clear energy audit. The daemon also checks population capacity and
   available resources; founder energy/nutrients come from existing plants/soil.
   Coordinates are metres; the world wraps at its configured extent.
8. Save a binary `.genome` file, load one, or **Copy share string** to the native
   clipboard. Paste a string into the editor and use **Load share string**. Saved
   genes and valid share strings round-trip exactly; they are also CLI inputs.

The zoo contains grazer, sprinter, ambush predator, scavenger, burrower, swimmer,
serpent (recursive serial body) and radial drifter. Preset names do not guarantee
speed rankings or million-tick ecological persistence. The first-generation test
uses two exact preset founders released into a fresh seed-42 demo world with 24
residents at `[51.2, 19.2]`: at least one naturally conceived descendant with two
parents must reach its inherited maturity alive at growth >=0.8. Compatible
outcrossing is allowed. All eight passed at ticks 7,000 or 12,000; see
[evidence/v1/zoo-first-generation.log](evidence/v1/zoo-first-generation.log).

## Train a body

Training evolves one body's motor brain for one physics task, generation by
generation, in the manner of Keiwan's Evolution. The body stays fixed unless body
mutation is enabled. Each individual runs one real physics trial; the best fifth
carry on unchanged and the rest of the next generation are their mutated copies.

```sh
./target/release/bestiary train swimmer --task swim --generations 30 --out exports/swimmer.genome --replay exports/swimmer.rpl
./target/release/bestiary combine exports/swimmer.genome
```

Tasks are `sprint`, `swim`, `turn`, `slope`, `rough` and `strike`; fitness is the
same measurement the combine uses for that trial. `--json` prints one JSON object
per generation. `--population`, `--generations`, `--seed` and `--body-mutation`
override `config/training.ron`; `--training PATH` loads a whole settings file.
Runs are deterministic for a given body, task, settings and seed.

In the viewer, open **Training** (or start with `--panel training`), choose a
body (a starter preset, the Designer draft or the selected creature) and a task,
then press **Start**. Each generation's runs are drawn side by side, with the
champion in full anatomy and the rest in grey; the chart shows best and median
fitness per generation. **Pause** takes effect after the generation in progress.
Drag the generation slider to replay an earlier champion, then **Save genome**
(to `exports/training/`), **Open in designer**, or **Release into world** when a
daemon is attached.

## Export camera clips

In the inspector or designer, choose a recorded replay. In the world view, wait
for the displayed bodies' available recordings to load. Set **From** and **For**,
then **Export camera clip**. Each export creates a new directory under `exports/`
with 30 fps camera PNGs, `clip.json` metadata and `ffmpeg.txt`. Editing is held fixed
during export; interface overlays are omitted and the world snapshot stays fixed.
The sidecar gives an optional ffmpeg command; ffmpeg is neither required nor run
by the application.

CLI export uses an existing recorded replay ID from an archive row's `replay_id`:

```sh
./target/release/bestiary archive ls worlds/my-world.db --sort viability --json
./target/release/bestiary replay REPLAY_ID --world worlds/my-world.db --export exports/my-clip
```

Replace `REPLAY_ID` with the returned ID, such as a genome hash followed by `-0`.
The suffix is a storage index; the recorded trial kind is read from the data.
A replay file can be used instead of an ID. PNG export still needs `--world` to
identify and verify its recorded anatomy, a sibling viewer executable and a GPU.
An unrelated standalone combine replay cannot be exported against an arbitrary
world. Existing output directories are rejected. Successful CLI export verifies
completion metadata and every expected numbered PNG, including the final pose.
Without `--export`, `bestiary replay FILE.rpl` reports its trial, frame count and
fixed timestep without opening the viewer.

## Query genomes, archive and phylogeny

```sh
./target/release/bestiary world status worlds/my-world.db --json
./target/release/bestiary combine grazer --json --replay exports/grazer.rpl
./target/release/bestiary genome validate grazer --json
./target/release/bestiary genome show grazer --json
./target/release/bestiary genome mutate grazer --seed 7 --operator 0 --json
./target/release/bestiary genome cross grazer sprinter --seed 7 --json
./target/release/bestiary archive ls worlds/my-world.db --sort coverage --export exports/archive --json
./target/release/bestiary phylo worlds/my-world.db --format dot > exports/phylogeny.dot
./target/release/bestiary phylo worlds/my-world.db --format json > exports/phylogeny.json
./target/release/bestiary bench --report --json
```

Genome inputs accept a binary file, base64url share string, share-string text file,
or exact zoo name; quote names containing spaces, such as `"ambush predator"`.
Mutation operator indices 0–22 follow `Operator::ALL` in
`crates/bestiary-core/src/genome/genetics.rs`; output records identify the operator
by name. `archive ls` returns coverage, denominator and elite rows. `--sort coverage`
uses cell order; `--sort viability` orders elites by viability. Export writes
`archive.json` plus each elite's `.genome` and `.rpl` files. DOT output is data for
optional Graphviz use; no Graphviz installation is required. `bestiary --help`
lists commands. Commands accept `--json`; genome inspection, archive, replay and
benchmark reports already print JSON by default. `bestiary bench` checks only the
combine; `make bench` runs all three throughput gates.

SQLite persists genomes, creature lifespans, species, phylogeny edges, events,
time series, archive cells, exploit flags and mutation records, including
never-born proposals. Query a stopped copy with a SQLite tool for custom analysis.
The saved world also contains its configuration, RNG-relevant state and recurrent
brain state; archive exports alone are not complete world backups.

## Configuration, determinism and extension boundary

[TUNING.md](TUNING.md) documents every `config/world.ron` field and the other
configuration files. Pass a full file with `--config PATH` at world creation or
combine time. `config/demo.ron`, `config/trophic.ron` and `config/validation.ron`
serve distinct documented experiments; they do not silently replace the baseline.
`config/starter.ron` and `config/zoo.ron` are embedded at build time and copied into
new genomes; changing those source files requires rebuilding to alter new presets.
`config/viewer.ron` is cosmetic and can be loaded without rebuilding.

Physics uses a fixed 1/240-second tick and deterministic substeps. World ticks use
stored configuration, ordered state and explicit seeded streams. **Bitwise
re-simulation is promised only within one platform and one binary.** Preserve the
executable, source revision, genome, config and seed with an experiment. Across
platforms or physics versions, reproduce the recorded trajectory by replay rather
than claiming matching re-simulation hashes. `world status` prints the canonical
world-state hash; viewing does not mutate it.

Database schema 3 rejects development schemas 1 and 2. Those databases need their
original binaries. Compatible-schema worlds with an old physics revision remain
readable for recorded history but cannot resume against changed physics. Start a
fresh world for changed simulation rules and retain old evidence with its binary.

`bestiary-world` advances measured traits, metabolism and ethology; it does not run
Rapier each tick. `bestiary-sim` evaluates actual bodies and drives their motor
networks. The viewer can run isolated body physics without changing the world.
See [the world boundary](crates/bestiary-world/README.md). Simulation crates have no
Bevy, UI or async dependency. Extension points are `WorldRules::step` for world
rules and the public combine API for additional experiments. Scenario plugins,
selection-pressure director mode, sweep runner and learning systems remain outside
v1 scope.

## Validation commands

```sh
make check
make bench
make longrun
scripts/check-viewer-render
python3 scripts/test-run-demo.py
make ecology-longrun
make evolution-longrun
rustup component add llvm-tools-preview --toolchain 1.85.1
make coverage
```

`make check` runs format checks, warnings-denied Clippy, workspace tests and release
build. GPU tests are separate because they need native graphics access.
`make longrun` runs seeds 0–4 for one million ticks each, independently twice,
compares hashes at 1k/100k/1M, checks conservation and population bounds every tick,
and requires actual births, deaths and the deaths of every original founder. Its
`config/validation.ron` disables mutation to isolate reproductive accounting and
stability. `make evolution-longrun` uses mutation-enabled `config/demo.ron`, records
trait variance and archive coverage at 750k/1M, and fails when either stops growing.
Both wrappers retain per-seed logs under `var/acceptance/` and fail if any seed fails.
`make ecology-longrun` exercises trophic persistence and the plant/herbivore series.
Those numerical tests remain failing; ordinary `make check` does not claim they pass.
`make coverage` uses the pinned toolchain's LLVM tools and checks the 80% production
line-coverage floor separately for core, sim and world. Its measurements and
uncovered lines are written to `target/coverage/report.json` and `report.txt`.
Coverage has not yet been measured for this candidate.

To repeat the full demo test from a fresh world:

```sh
mkdir -p var/v1
BESTIARY_DEMO_TICKS=1000000 BESTIARY_DEMO_OUT="$PWD/var/v1/demo-repeat.db" ./scripts/cargo test --release -p bestiary-world --test reproduction open_endedness_smoke --locked -- --ignored --nocapture
```

Cargo tests execute from their crate directory; the absolute database path above
avoids package-relative output. Use a fresh output filename for each experiment.
A fresh-clone check after committing all source changes can be run with:

```sh
bestiary_clean=$(mktemp -d "${TMPDIR:-/tmp}/bestiary-clean.XXXXXX")
git clone --no-local . "$bestiary_clean/repo"
cd "$bestiary_clean/repo"
make setup
make check
```

This procedure requires an installed toolchain because ignored `.tools/` and build
outputs are not copied. Preserve command output and `git rev-parse HEAD` with the
results. Generated `var/`, `worlds/`, `exports/`, `dist/` and local toolchains are
ignored; commit source and `Cargo.lock`, not runtime databases.

## Measured acceptance

Measurements below were recorded on 2026-10-14 (America/Los_Angeles), on Apple M4
Pro / arm64, 24 GiB RAM, macOS 15.3.1 (24D70), Rust/Cargo 1.85.1. Fresh-world commands
and logs are collected under [evidence/v1](evidence/v1/README.md).

The final current-binary `make bench` passed: the full nine-trial 12-part combine
took 0.628458 seconds worst-of-three (2-second limit), the single-core world ran
5,000 creatures at 13,700.3 ticks/minute (10,000 floor), and the viewer reached
128.0886 fps (60 floor). The viewer fixture contains 2,020 actual agents, 2,000
visible proxies and 20 articulated bodies, renders the full UI at 2880×1920,
warms up 120 frames and measures 600 GPU-completed frames. The host log is
`/tmp/bestiary-v1-bench-proxy.log`; retained release evidence belongs under
`evidence/v1/`. These timings supersede earlier attempts.

Linux host, 2026-10-16: Intel i9-13900H (20 threads), 31 GiB, Fedora 44 x86_64,
NVIDIA RTX 4070 Laptop (Vulkan), Rust 1.85.1, after adding training mode.
`make bench` passed: combine 0.582 seconds worst-of-three, world 11,993.4
ticks/minute with 5,000 creatures (one unrelated daemon was running on the host),
viewer 350.8 fps on the same 2,000-proxy/20-body fixture. A fresh clone of
1f1e11e passed `make check` on this host
([fresh-clone-check.log](evidence/linux/fresh-clone-check.log)).

The fresh stability run passed all five seeds for 1M ticks, with exact resource
accounting, matching repeated hashes at 1k/100k/1M, no collapse and finite original
founders. It recorded 10,785–11,417 births per seed and final populations 23–45;
see [longrun.log](evidence/v1/longrun.log). M9 `make check`, actual designer GPU
rendering and combine/release over IPC passed; evidence is in `m9-check.log`,
`m9-designer-render.log`, `m9-designer-release.log` and `zoo-first-generation.log`.

The demo uses **seed 42**, `config/demo.ron`, **24 founders** and **1,000,000 ticks**.
It finished with **56 creatures, five living species, a 15-part living species
founder versus 13-part original founders, and 5/128 archive cells (3.90625%)**.
Every stored elite recombined exactly. See
[demo-million-ticks.log](evidence/v1/demo-million-ticks.log) and
[demo-database-audit.json](evidence/v1/demo-database-audit.json). The 15% target would
require at least 20 of the current 128 cells; it has not been reached. The
conservative Cartesian denominator and all bin edges remain unchanged. The old
10.2% report came from earlier bodies/physics and is not current-binary acceptance.

The five mutation-enabled evolutionary seeds all reached 1M ticks without collapse,
but every archive curve plateaued from 750k to 1M; only seed 3's trait variance rose.
Thus **0/5 stagnation guards passed**. The trophic study recorded **0/5** persistent
food chains, and the herbivore-only study collapsed by 800k without an evolved
resistance increase. Full failed runs and tuning attempts are preserved in
`evolution-million-ticks.log`, `ecology-longrun.log` and [ASSUMPTIONS.md](ASSUMPTIONS.md).
These unmet items remain open and prohibit tagging `v0.1.0`.
The 80% coverage measurement, fresh-clone `make check`, and full acceptance reruns
against the final binary also remain unverified. The user requested documenting
and pushing this candidate without starting another long experiment; this is not
a completed M10 release.
