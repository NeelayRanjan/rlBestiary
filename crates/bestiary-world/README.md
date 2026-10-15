# World simulation boundary

`World::step()` executes one fixed world tick (360 simulated seconds by default).
Bodies are evaluated by `bestiary-sim` when genomes enter the world. Movement,
posture and action costs use those measured traits; world ticks do not run Rapier.
A creature's recurrent ethology brain chooses movement and actions. Its separate
motor pools drive muscles only in physical evaluations and inspector arenas.

Energy is stored as integer millijoules and nutrients as integer microkilograms.
Solar input and metabolic/decay/feeding losses are explicit ledgers. Digested
nutrients not retained for growth return to the soil. A corpse retains both
resources until scavenging or decay transfers them. `World::conserved()` compares
exact totals against the initial inventory and external energy ledger.

All world rules run through `WorldRules::step`, currently `NaturalRules`.
The world owns no window, wall clock, async executor or rendering dependency.
Snapshots serialize the complete world including brain plasticity and config.
Hashes use canonical ordered storage; repeatability is within one binary/platform.
