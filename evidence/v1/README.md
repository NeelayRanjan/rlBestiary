# Full-v1 validation evidence

Host: Apple M4 Pro (arm64), 24 GiB RAM, macOS 15.3.1 (24D70).
Toolchain: Cargo/Rust 1.85.1, pinned by rust-toolchain.toml.
Resumption base: 1f375ed08cba6410d732a632f9639246907ccca3.
Validation date: 2026-10-14 (America/Los_Angeles).

All worlds in this validation were created on this host from the current source.
Historical MVP databases and measurements do not establish current acceptance.
Logs are captured from commands, including failures; a failed quantitative gate
is not waived by ordinary unit tests passing.

M5 commands:

```
./scripts/cargo run --release -p bestiary-world --example trophic -- config/trophic.ron 0 10000
make ecology-longrun
make check
```

`trophic-baseline.log` used the pre-resumption config at the base commit.
`trophic-tuned.log` uses the committed trophic configuration.
`trophic-refuge.log` uses that configuration with attack_scale=2 and
refuge_fraction=0.98. This tests the opposite pressure direction after the tuned
scenario overpredated its prey. None changes the archive denominator or bins.

The trophic criterion checks producers, herbivores and flesh consumers on every
tick. Scavengers occupy the flesh tier; the optional intermediate diet bin is not
an additional trophic level. Founders must pass real combines. The million-tick
criterion also requires predation and actual births/deaths, with conservation
and a finite population bound.

M7 commands (fresh worlds; no archive bin or denominator change):

```
BESTIARY_DEMO_TICKS=1000000 BESTIARY_DEMO_OUT="$PWD/var/v1/demo-tuned.db" ./scripts/cargo test --release -p bestiary-world --test reproduction open_endedness_smoke -- --ignored --nocapture
make evolution-longrun
make longrun
```

The first demo and high-mutation exploratory runs were stopped at their saved
checkpoints for tuning; their `*-partial.log` files are not final acceptance.
Their configurations are preserved in `config/demo-baseline.ron` and
`config/exploration.ron`. The saved initial demo lives under
`var/v1/package-relative/v1/demo.db` (moved intact from the package-relative
`crates/bestiary-world/var/v1/demo.db` after that exploratory run stopped).
Cargo tests run in their package working directory. The final demo uses the absolute output path above.

`make longrun` uses `config/validation.ron`: sexual reproduction and mortality
remain enabled, mutation is disabled in this accounting/determinism control.
`make evolution-longrun` uses the separate mutation-enabled `config/demo.ron`.
Both invoke all five explicit seeds through `scripts/longrun`, with independent
processes and serial world ticks. A partition regression checks exact coverage
of seeds 0–4. Failed assertions propagate to Make's exit status.
