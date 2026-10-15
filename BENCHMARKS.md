# Benchmark record

## M1 physics gate — 2026-10-14

Host: macOS arm64; Rust 1.85.1; release optimization with thin LTO.
Command: `make bench`.

Twelve articulated parts, twenty antagonist muscles, nine sequential two-second
workloads, 240 physics ticks/second, full energy audit and trajectory recording:

- Worst of three wall-clock measurements: 0.282914 seconds (limit: 2 seconds).
- Throughput: 63.623 simulated creature-seconds per wall-clock second.
- Criterion estimate: 194.17–256.50 ms per nine-workload evaluation.

This is the M1 representative physics workload, not the M3 semantic combine.
The nine named trials, world throughput and viewer frame rate are unmeasured
until their respective milestones. Other compilation was active during this run;
the final release report must use an otherwise idle machine.

Mechanical regression: in six simulated seconds, the active fixture displaced
0.12885287 m versus 0.0027338206 m with activation disabled. Minimum root height
was 0.6585219 m. Recorded positive muscle work: 112.05236 J; holding energy:
0.30000004 J; no unexplained gain and no exploit flag in that run.

## M3 full combine gate — 2026-10-14

Command: `make bench`, same macOS arm64 host and Rust 1.85.1 toolchain.
Twelve-part genome, all nine named trials executed, including a three-angle
slope sweep: 22 creature-seconds at 240 ticks/second with two substeps per tick.

- Worst of three sequential evaluations: 0.535368 seconds (limit: 2 seconds).
- Throughput: 41.093 simulated creature-seconds per wall-clock second.
- Criterion estimate: 465.08–502.01 ms per full evaluation.

This supersedes M1's representative workload. World and viewer budgets remain
unmeasured. Serial/parallel equality, cached/recomputed equality, all eight zoo
fixture repeatability, lossless replay round-trips, a normal-genome audit and a
deliberate overlapping-body exploit test pass.
