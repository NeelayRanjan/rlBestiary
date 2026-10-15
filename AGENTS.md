# Operating rules

These rules govern the whole build of `bestiary`. `SPEC.md` is the specification;
this file is how you work. Read both in full before your first action, and re-read
both after any restart or loss of context.

1. Never ask the user a question. When the spec is silent or ambiguous, decide,
   implement, and record the decision with a one-line rationale in `ASSUMPTIONS.md`.
   When something in the spec is impossible as written, implement the closest
   feasible version and record it under a "Deviations" heading in the same file.
   Never stop early for any reason other than the Completion checklist being
   fully satisfied.
2. No runtime network, no secrets, no downloaded assets. The finished app must run
   with the network unplugged. Terrain, creatures, and textures are procedural;
   there are no model files, texture files, or font downloads. Build-time crate
   and npm installs are fine. If a build-time download fails, vendor or generate
   what you need and note it.
3. Determinism is a correctness property, not a nice-to-have. Every rule in the
   Determinism section of `SPEC.md` is binding. If you cannot make something
   deterministic, make it reproducible from a recording and say so in
   `ASSUMPTIONS.md`.
4. The performance gate in M1 comes before everything built on top of it. If the
   measured throughput misses the budget in `SPEC.md`, change the trial design
   or the world tiering until it passes, and record what you changed. Do not
   proceed to M2 with a failing benchmark and a plan to optimize later.
5. Work milestone by milestone, in order. Do not start milestone N+1 until
   `make check` passes at milestone N. Commit at the end of each milestone with
   the message `M<n>: <summary>`. Commit more often if useful.
6. Keep `PLAN.md` current: one checkbox per task under each milestone, ticked as
   you go, with a "Current task" line at the top. On any restart or loss of
   context, read `AGENTS.md`, `SPEC.md`, `PLAN.md`, `ASSUMPTIONS.md`, and
   `git log --oneline` before doing anything else.
7. Every feature ships with tests. A feature without tests is not done. Where the
   spec gives an acceptance criterion, there is a test with that criterion's name
   in it.
8. You cannot evaluate whether the simulation is interesting, beautiful, or fun.
   Do not try, and do not claim it. Your criteria are the mechanical ones in the
   spec: determinism, conservation, coverage, stability, throughput. Tuning for
   emergent behavior is the user's job after the build, so every tunable constant
   lives in one documented config file rather than scattered through the code.
9. Prefer boring, widely used crates, pinned versions, and as few dependencies as
   the job allows. Pin exact versions in `Cargo.toml` and commit `Cargo.lock`.
10. Quality bar at every milestone: `cargo fmt --check` clean, `cargo clippy
    --all-targets -- -D warnings` clean, `cargo test --workspace` green, release
    build succeeds.
11. Keep the simulation crates free of rendering, UI, and async. The viewer
    depends on the simulation; the simulation never depends on the viewer. A
    violation of this is a bug even if it compiles.
12. Documentation is for running and extending the thing, not describing it.
    `README.md` stays a short pointer to `RUNBOOK.md` once that exists.
13. If a milestone is taking far longer than its neighbors, cut scope within that
    milestone (fewer trial types, plainer UI, fewer world systems), note the cut
    in `ASSUMPTIONS.md`, and move on. A finished, narrower v1 beats an unfinished
    broad one. Never cut determinism, conservation, or the test suite to save time.
