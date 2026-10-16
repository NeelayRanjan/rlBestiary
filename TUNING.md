# Tuning bestiary

Change one group of parameters at a time, create a fresh world with the same seed,
and compare recorded measurements at equal ticks. `config/world.ron` is the full
baseline schema; its comments give units and the tables below give the purpose and
direction of each field. Pass a modified copy with `bestiary world new --config PATH`
or `bestiary combine GENOME --config PATH`. The entire configuration is stored in
each world. Editing the source RON does not retune an existing saved world.

## Muscles and body support first

Start with a single genome in the combine or Designer live arena. Inspect ground
contact, posture cost, displacement and the energy audit. Changing appearance
cannot change physical traction, leverage, force or work. Muscle material genes
multiply the global contraction, stiffness and damping baselines, so inspect both
the gene and its world setting.

| Field | Baseline | Effect and symptom to investigate |
| --- | --- | --- |
| `physics.force_per_area` | 120 | Maximum active force per muscle cross-section. Raise cautiously when a body collapses under its weight; lower when contractions cause vibration or extreme impulses. Increasing gene area also raises force and holding cost. |
| `physics.length_tension_width` | 0.8 | Relative width of the parabolic force-versus-length curve. Increase when muscles lose force through ordinary joint travel; narrow it to restrict force to lengths near rest. Check that extra force does not destabilize contact. |
| `physics.max_contraction_fraction` | 0.55 | Rest-length fraction a fully activated muscle may shorten. Increase for inadequate travel, decrease for excessive shortening or folded joints; the inherited contraction scale also applies. |
| `physics.passive_stiffness` | 20 | Stretch-only passive spring stiffness in N/m. Raise for excessive stretch; lower when bodies vibrate. Passive elastic energy remains in the audit ledger. |
| `physics.damping` | 0.5 | Passive axial damping. Raise to dissipate repeated oscillation; lower if damping prevents useful contraction. It cannot supply active energy. |
| `genome.max_span` | 2 | Maximum joints crossed by a muscle, valid range 1–2. If a creature never leaves the ground because leverage is too local, allow span 2 and draw/test endpoints across two joints. A longer permitted span alone does not create those genes. |
| `physics.isometric_cost` | 0.05 | Watts per area and activation, in addition to mechanical work. Raise when large muscles can hold posture too cheaply; lower only when measured holding cost overwhelms feeding. |
| `genome.attachment_jitter` | 0.2 | Normalized surface offset for inherited genesis attachments, range 0–0.5. Increase separation when attachments have weak moment arms; inspect the resulting geometry and combine audit. Expansion also uses deterministic geometry-aware placement to cover missing actuated axes. |
| `physics.gravity` | 9.81 | Gravitational acceleration in m/s². Keep fixed for comparable experiments; a deliberate reduction lowers support demand and changes potential energy. |
| `physics.substeps` | 2 | Solver substeps within the fixed 1/240-second physics tick, range 1–16. Increase for unresolved contact/constraint instability, at a throughput cost. Re-run the energy audit and benchmark. |
| `physics.friction` | 0.9 | Ground Coulomb friction. Raise for observed stance slip; lower if contacts lock or trip a body. Use measured displacement and contact slip to distinguish traction from a gait problem. |
| `physics.audit_absolute_tolerance` | 0.01 | Absolute allowed numerical energy error per substep in joules. This is a correctness threshold, not a tuning shortcut; investigate solver/contact energy before any justified change. |
| `physics.audit_relative_tolerance` | 0.005 | Relative numerical energy allowance per substep. Preserve exploit rejection when changing numerical integration; do not widen it to admit a failed body. |
| `physics.fixture_frequency` | 1.5 | Controller frequency in Hz for the historical M1 physics fixture. This does not retime authored zoo or evolved motor networks. |
| `physics.fixture_muscle_area` | 0.1 | Muscle area for the M1 fixture and newly drawn Designer muscles. The Designer can then edit the area gene. Existing genes are unchanged. |
| `physics.benchmark_trial_seconds` | 2 | Simulated seconds per workload in the historical M1 fixture. The full combine gate uses the actual `combine` trials; shortening this fixture is not evidence for the final combine budget. |

The Designer's basal estimate excludes posture and locomotion. Its maximum holding
estimate is `sum(expanded muscle area) × physics.isometric_cost` at full activation.
After a combine, `standing_cost` includes measured posture work and isometric cost;
`energy_per_meter` measures locomotion expenditure. A body that cannot keep its root
collider clear of the ground fails the stand gate. All active forces remain
contractile muscle forces; there are no joint motors or kinematic foot pins.

## Required failure modes

| Observed failure mode | First configuration experiments | What to measure |
| --- | --- | --- |
| **Sessile-sipper equilibrium**: stationary feeding pays indefinitely | Make patches deplete more readily by lowering `world.solar_per_cell` relative to `world.bite_energy`; reduce `world.plant_energy` to lower each patch's storage capacity and initial buffer. Raise `world.season_amplitude` so local food availability varies more strongly. The existing regrowth field is heterogeneous through moisture, temperature and evolving plant genes; there is no separate regrowth-patchiness slider. Adjust `world.cell_metres` to change patch spacing, then test whether movement can reach alternative food. | Net reserves of stationary and moving creatures, local depletion, movement cost, mortality and exact conservation. Raising all metabolic costs alone does not make movement affordable. |
| **Monoculture**: one genotype or species dominates | Raise `genome.mutation_floor` and, if proposals are rare, `reproduction.mutation_probability`. Fragment habitat more strongly with `world.terrain_frequency`, `ecology.sea_amplitude` and/or `ecology.uplift`; reduce `ecology.tectonic_epoch` for more frequent geographic updates. | Registered living species, actual compatible mating, descendant lineages after fragmentation, trait variance and coverage. More mutations may instead create nonviable proposals. |
| **Predator-prey oscillation to extinction** | Raise `ecology.refuge_fraction`; lower `ecology.attack_scale` to reduce damage/success at comparable body strength. Consider a higher `ecology.attack_cost` and enough prey energy supply. Slower carcass decay can improve scavenger access but cannot replace live prey. | Per-tick trophic presence, actual strikes/deaths, population troughs, births and resource ledgers across five seeds. These adjustments have not yet achieved the million-tick trophic acceptance. |
| **Stagnation after a few thousand generations** | Raise `ecology.antigen_drift`, which changes antigen traits and also drifts virulence/transmission at one tenth that amplitude. Reduce `ecology.pathogen_epoch` for faster pathogen turnover. Broaden rare-lineage opportunities by enabling `archive.reseed_interval` and adjusting `archive.reseed_count`; retain measured viable elites and resource transfers. | Tail-window growth of both normalized trait variance and archive coverage. More pathogen pressure or reseeding is an experiment, not proof that either metric will rise. |

Never lower the acceptance thresholds, disable conservation, remove determinism
checks or admit energy exploits to make an experiment pass. Archive bin changes
change the question being measured and require a documented reachability argument.

## World geography, resources and metabolism

All rows in this section are fields of `world` in `config/world.ron`.

| Field | Baseline | Meaning and tuning direction |
| --- | --- | --- |
| `world.grid` | 32 | Cells per side, range 2–256. Increase for more spatial habitats and larger initial inventories, at a cost in memory and per-tick work. Keep total area/inventory changes explicit when comparing runs. |
| `world.cell_metres` | 32 | Cell width in metres. Increase for larger distances between patches; decrease when measured locomotion cannot traverse a patch within useful time. World extent is `grid × cell_metres`. |
| `world.seconds_per_tick` | 360 | Simulated seconds per fixed world tick, default ten ticks/hour. This scales measured movement and energy expenditure; lower it when a coarse tick overshoots ecological interactions. Tick-count ages and epochs do not automatically rescale. |
| `world.decision_interval` | 10 | Deterministically staggered ticks between recurrent ethology decisions. Lower for more responsive behavior and higher CPU cost; raise to reduce brain evaluation frequency. |
| `world.season_ticks` | 3600 | Ticks per seasonal cycle. Lower for faster seasonal turnover; compare lifecycle length against the cycle before interpreting selection. |
| `world.plant_epoch` | 100 | Interval for plant selection, dispersal and mutation. Lower to accelerate plant-trait turnover; check the toxicity/resistance time series rather than assuming an arms race. |
| `world.solar_per_cell` | 10000000 | Base incoming energy units per cell/tick, multiplied by seasonal light, temperature, moisture and regrowth; baseline units are millijoules. Raise for energy-limited food production, lower for faster patch depletion. Capacity and soil nutrients limit accepted input, which remains explicit in the ledger. |
| `world.soil_per_cell` | 2000000 | Initial nutrient units per cell; baseline units are microkilograms. Raise when plant/animal growth is nutrient-limited. This changes the initial closed inventory. |
| `world.plant_energy` | 2000000000 | Initial plant energy and maximum stored plant energy per cell. Raise for larger food buffers/capacity; lower for quicker patch depletion and less seasonal storage. |
| `world.plant_nutrients` | 2000000 | Initial nutrients in each plant cell. Raise for a larger immediately edible nutrient pool; distinguish it from soil nutrients. |
| `world.initial_energy` | 200000000 | Initial individual energy reserve. Raise to lengthen a founder's feeding/mating runway; releases withdraw this amount from existing plants. It does not improve steady-state feeding efficiency. |
| `world.reserve_per_kg` | 500000000 | Energy reserve capacity per kg of grown mass. Raise for larger starvation buffers and higher absolute mating thresholds; lower for more frequent feeding pressure. |
| `world.bite_energy` | 50000000 | Maximum energy removed by one foraging action. Raise when food is present but intake cannot meet measured costs; account for plant toughness, assimilation and toxin dose. |
| `world.basal_per_kg` | 0.2 | Basal watts/kg in addition to measured posture. Raise to increase body-mass maintenance pressure; lower for demonstrated basal starvation. |
| `world.sensor_cost` | 0.002 | Watts per sensor gene. Raise to penalize unused sensing investment; lower if sensor overhead alone starves small bodies. |
| `world.brain_cost` | 0.001 | Watts per motor/ethology neuron gene. Raise to make larger brains costly; lower only after measuring organ-cost pressure. |
| `world.growth_per_tick` | 0.005 | Maximum fraction of adult mass grown per tick, constrained by assimilated nutrients. Raise when juveniles remain small despite available food; faster growth still transfers real nutrients. |
| `world.decay_fraction` | 0.01 | Fraction of carcass resources recycled each tick, range 0–1. Lower to keep scavengable carcasses longer; raise for faster soil return and energy dissipation. |
| `world.starvation_damage` | 0.02 | Health loss for an unpaid metabolic tick. Lower to lengthen the recovery window; zero disables this cause of death and is not a legitimate ecological acceptance setting. |
| `world.healing` | 0.01 | Health recovered on a fully paid metabolic tick. Raise for quicker recovery; toxin damage is applied after healing and senescence still caps health. |
| `world.water_level` | -0.4 | Base height threshold separating land from water. Raise to submerge and fragment land; lower to connect terrestrial habitat. Terrain and sea-level oscillation also affect the boundary. |
| `world.terrain_frequency` | 1 | Number of procedural height waves per map axis. Increase for more geographic subdivisions and islands; inspect actual connected land regions. |
| `world.season_amplitude` | 0.3 | Seasonal variation in temperature and solar availability. Raise for stronger local resource cycles; lower if seasonal minima erase all populations. |
| `world.population_limit` | 10000 | Finite maximum admitted population, valid up to 100000. Raise for more individuals only after measuring throughput/resource demand; this bound is not a mechanism for sustainable coexistence. |
| `world.energy_units_per_joule` | 1000 | Integer energy resolution. Keep fixed for ordinary tuning; changing it requires scaling all energy inventories and costs consistently. |
| `world.nutrient_units_per_kg` | 1000000 | Integer nutrient resolution. Keep fixed unless deliberately changing ledger precision and consistently rescaling inventories. |

World energy expenditure includes measured posture, basal mass cost, sensors,
neurons, immune investment and measured movement. Nutrient transfers cover growth,
birth, eating, carcasses and soil. `World::conserved()` compares exact integer
ledgers against initial inventory, solar input and explicit dissipation.

## Ecology

| Field | Baseline | Meaning and tuning direction |
| --- | --- | --- |
| `ecology.toxin_damage` | 0.05 | Health lost for a full bite at toxicity one and zero immune investment, range 0–1. Raise to strengthen detoxification selection; lower when every herbivore dies before reproduction. Dose depends on food actually eaten. |
| `ecology.attack_scale` | 20 | Damage coefficient for measured strike impulse relative to defending bulk. Lower for excessive prey mortality; raise if real contact never produces damage. It does not grant a missing physical strike trait. |
| `ecology.attack_cost` | 10000 | Energy units paid per attempted strike. Raise to discourage repeated unprofitable attacks; lower only after measuring pursuit/attack budgets. |
| `ecology.injury_fraction` | 0.2 | Fraction of damage retained as persistent movement injury, range 0–1. Lower if nonfatal hits lead to an unavoidable feeding collapse; raise for longer consequences of injury. |
| `ecology.refuge_fraction` | 0.3 | Terrain refuge fraction reducing attack success, range 0–1. Raise to protect prey troughs; monitor predator starvation as well. |
| `ecology.pathogen_count` | 4 | Number of evolving strains, range 0–32. Increase for more antigen diversity and cost; zero removes pathogen pressure and changes the experiment. |
| `ecology.pathogen_epoch` | 1000 | Ticks between pathogen selection/drift updates. Lower for faster adaptation; raise when pathogen turnover overwhelms host generations. |
| `ecology.transmission` | 0.002 | Initial per-encounter transmission probability, range 0–1. Raise for sparse infection; lower if infection spreads before hosts can reproduce. Evolving strains inherit later values. |
| `ecology.virulence` | 0.001 | Initial per-tick strain damage, moderated by immunity/resistance. Raise for stronger disease pressure; lower after widespread disease extinction. |
| `ecology.antigen_drift` | 0.01 | Mutation amplitude for strain antigen traits, range 0–1; virulence and transmission also drift at 0.1 times this amplitude. Raise for faster changing host pressure; monitor extinction risk through the same mechanical tests. |
| `ecology.pheromone_diffusion` | 0.15 | Four-channel spatial diffusion coefficient per tick, range 0–1. Raise to spread signals farther; lower to retain sharper local gradients. |
| `ecology.pheromone_decay` | 0.05 | Fraction of signal removed each tick, range 0–1. Raise when old trails dominate; lower when trails vanish before another agent encounters them. |
| `ecology.pheromone_cost` | 1000 | Energy units per emitted unit. Raise to make signaling investment costly; lower when all inherited emitters starve. |
| `ecology.tectonic_epoch` | 1000 | Geographic update interval in ticks. Lower for faster terrain/sea changes; preserve enough generations to distinguish geographic isolation from immediate die-off. |
| `ecology.uplift` | 0.001 | Incremental terrain-change amplitude. Raise for faster habitat change; lower when geography changes faster than movement/reproduction. |
| `ecology.sea_amplitude` | 0.15 | Sea-level oscillation amplitude. Raise for stronger fragmentation/reconnection; inspect the recorded region-count events. |
| `ecology.shock_probability` | 0.002 | Chance of cold, drought or disease shock per geographic epoch, range 0–1. Raise to test recovery from disturbances; lower if shocks repeatedly erase populations. |

## Reproduction and species

| Field | Baseline | Meaning and tuning direction |
| --- | --- | --- |
| `reproduction.epoch` | 1000 | Fixed interval between mating/evaluation opportunities. Lower for more opportunities and more combine work; raise to reduce proposal load, while retaining opportunities before senescence. |
| `reproduction.mate_radius` | 128 | Maximum mate-search distance in metres. Increase for sparse populations, decrease for stronger local mating. Terrestrial pairs must also occupy the same connected land region. |
| `reproduction.compatibility_threshold` | 0.2 | Maximum genetic/trait distance for mating and species-similarity normalization. Raise for more compatible outcrossing; lower for stronger reproductive isolation, at a risk of mate scarcity. |
| `reproduction.energy_fraction` | 0.65 | Fraction of adult reserve required to attempt mating, range 0–1. Lower when mature compatible adults never reproduce; raise if mating drains parents before recovery. |
| `reproduction.gestation_loss` | 0.1 | Fraction of reproductive energy dissipated during gestation, range 0–1. Raise reproductive cost, or lower it when measured energy budgets cannot support offspring. All transfers remain conserved. |
| `reproduction.mutation_probability` | 0.1 | Per-copy probability of entering the mutation pass. Each inherited operator rate is tested separately inside that pass. Raise for more proposals and physics evaluations; lower if mostly failed proposals dominate run time. |
| `reproduction.founder_maturity` | 10000 | Initial `World::new` founder maturity in ticks. Lower to allow earlier reproduction; pair it with adequate growth/energy and a later senescence age. Descendants inherit life-history genes. |
| `reproduction.founder_senescence` | 100000 | Initial founder senescence onset, strictly after maturity. Lower for quicker turnover; increase only as an explicit life-history experiment, never to replace birth/death acceptance with immortal founders. |
| `reproduction.species_epoch` | 1000 | Interval between compatibility clustering/history updates. Lower for finer registration timing and higher cost; raise to sample species more sparsely. |
| `reproduction.species_min_population` | 2 | Minimum cluster population for registration. Raise to reject tiny transient clusters; lower to record rare lineages sooner. |
| `reproduction.species_min_duration` | 1000 | Persistence in ticks required for a candidate cluster. Raise to reject short-lived splits; lower to observe brief reproductive isolation. |

Founder controls in this section apply to `World::new`. Designer releases carry
their own saved life-history genes; zoo releases use `config/zoo.ron`. Do not assume
changing a world's founder defaults rewrites an imported genome.

## Genome bounds and inherited mutation

The muscle-span and attachment-jitter controls are documented first above.

| Field | Baseline | Meaning and tuning direction |
| --- | --- | --- |
| `genome.max_parts` | 32 | Maximum expanded rigid parts, range 1–32. Lower for cheaper bodies; increasing up to 32 permits structural growth but does not force it. |
| `genome.max_depth` | 12 | Maximum expanded edge depth, range 1–12. Lower to limit long chains; raise to allow recursive serial bodies. |
| `genome.max_mass` | 100 | Maximum expanded mass in kg. Lower to constrain bulk and physics demand; raise only while support/energy budgets remain measured. |
| `genome.min_size` | 0.02 | Minimum full part dimension in metres. Raise to avoid tiny segments and bound scale cascades; lower permits finer parts but may stress contact numerics. |
| `genome.mutation_floor` | 0.001 | Minimum inherited per-operator mutation rate. Raise to prevent operators from becoming arbitrarily rare. Initial rates are 0.1 clamped to this interval, so changes below 0.1 primarily constrain later evolved rates. |
| `genome.mutation_ceiling` | 0.5 | Maximum inherited per-operator rate, at most one and no lower than the floor. Lower when copies change too many features; raise for broader structural proposals and more failed-combine risk. |

Expansion clips branches cleanly at part/depth/mass/minimum-size bounds. Recursion
counts limit repeated visits along each branch. Clipped routes may disappear, but
the instantiated body still must have a spanning muscle for every actuated joint
axis. Invalid local dimensions, joint limits or gene references are errors rather
than an excuse to weaken the bound or genesis tests.

## Combine trials and measured traits

| Field | Baseline | Meaning and tuning direction |
| --- | --- | --- |
| `combine.trial_seconds` | 2 | Measured trial interval, valid 2–60 seconds. Increase to distinguish sustained movement from short transients; it raises evaluation cost. Locomotion uses net centre-of-mass travel after settling. |
| `combine.stand_seconds` | 2 | Stand duration and unscored locomotion settling interval, at least two seconds and no longer than `trial_seconds`. Increase to expose delayed collapse before scoring movement. |
| `combine.root_clearance` | 0.05 | Minimum root collider surface clearance above terrain in metres. This is a viability criterion; lowering it changes what counts as supported posture. Test grounded and collapsing controls. |
| `combine.rough_height` | 0.03 | Procedural rough-ground bump height in metres. Raise for a harder rough-terrain test; keep fixed across trait comparisons. |
| `combine.slope_angles` | `[0.1, 0.2, 0.35]` | Ascending sampled radians. Add steeper grades to resolve climbing ability, at the cost of extra trials. Preserve sorted, finite entries. |
| `combine.water_density` | 2 | Fluid density in the simulator's SI mass convention. Raise buoyancy, lower it for harder flotation; do not substitute real-water values without reconciling body density. |
| `combine.water_drag` | 4 | Linear drag per immersed volume. Raise hydrodynamic resistance; lower it for easier motion. Fluid work remains part of the energy audit. |
| `combine.carry_fraction` | 0.5 | Added load as a fraction of body mass. Raise to test heavier loads; success requires at least half of the unloaded measured Sprint speed. |

Every configuration change affecting physics changes the combine cache key. Test
new genomes with the world's configuration, not an unrelated baseline file. Failed
stand gates can omit locomotion recordings. Full combine benchmarks use a 12-part
fixture with all nine trials, independent of the current founder's part count.

## Archive grid and reseeding

| Field | Baseline | Meaning and tuning direction |
| --- | --- | --- |
| `archive.thresholds` | `([8], [2], [0.75], [0.6], [0.5], [1], [0.05])` | Interior cuts for parts, limbs, mass kg, reach m, symmetry, gait Hz and swim m/s, in that order. Each axis has one more bin than cuts. More bins resolve more differences and increase the denominator; changing cuts can move old measurements between cells. Keep the grid fixed when comparing coverage. |
| `archive.reseed_interval` | 0 | Zero disables reseeding. Otherwise, every N ticks choose the viable elite whose exact genome is least represented among living creatures, with deterministic hash tie-breaking, and release at a seeded position. Lower N increases reintroduction frequency. |
| `archive.reseed_count` | 2 | Founders per reseeding attempt, range 1–1000. Raise to improve establishment opportunities, but releases still require population headroom and existing soil/plant resources. |

The seven binary axes produce **128 Cartesian cells**. This denominator is
conservative and includes combinations that may be unreachable; it is not the
number discovered so far. Current coverage is reported against that fixed grid.
No bin or denominator was changed to meet a target. Any revised reachable-cell
model needs a biological/physical reachability argument recorded under Deviations
and a new, explicitly comparable experiment. It cannot retroactively turn a failed
run into a passing one.

Reseeding copies existing elites, including genotypes natural selection removed;
it does not itself discover a new cell. Its descendants can mutate under ordinary
rules. A failed release due to capacity/resources is not a successful reseeding.
Keep reseeding off for natural-selection-only runs, and record it explicitly when
comparing experiments with assisted reintroduction.

## Authored genes and appearance files

These files separate inherited genome templates and cosmetic rendering from the
world-rule schema. Saved genomes contain their own genes; changing source templates
does not rewrite them.

| File | When it applies | Controls |
| --- | --- | --- |
| `config/starter.ron` | Embedded when building; copied into newly created starter genomes | Six-leg articulated anatomy and ordinary motor-network weights. `upper_shape`/`lower_shape`, `upper_size`/`lower_size` set colliders; `hip_rest`/`knee_rest` and `hip_limits`/`knee_limits` set rest geometry and hard angular excursions; `hip_area`/`knee_area` set muscle areas; `knee_origin`/`knee_insertion` set normalized attachments. The `hip` and `knee` blocks each provide `frequency`, `phase_offset`, paired `biases`, `oscillator`, `proprioception`, `angular_velocity` and `contact` weights. Raise feedback gradually; phase or gain changes must be measured for support, slip and energy exploits. |
| `config/zoo.ron` | Embedded when building; applied to newly requested zoo presets | `maturity`, `senescence`, `clutch_size`, `offspring_fraction` supply inherited lifecycle defaults. `burrower_clutch_size`, `burrower_offspring_fraction`, `predator_offspring_fraction` specialize juvenile investment. `forager`, `predator` and `scavenger` are inherited ethology networks: `frequency`/`phase`, neuron `bias`/`activation`, and each connection's `input`, `weight`, `learning_rate`, `decay`. The comments identify outputs and key sensors. More positive bias/weight favors an action only through that activation function; descendants still pay measured costs. These templates do not alter `World::new` rules. |
| `config/viewer.ron` | Embedded default; load an edited copy with viewer `--appearance PATH` | Cosmetic geometry, colors and contact visualization only. It does not enter genome hashes, world hashes, traits or physics caches. |

All appearance fields are documented here so display tuning is distinguishable
from physical tuning:

| Viewer field | Effect |
| --- | --- |
| `muscle_radius_scale` | Rendered muscle radius relative to `sqrt(gene area / pi)`; smaller draws thinner tissue without changing force. |
| `muscle_max_radius_fraction` | Caps belly radius relative to attachment distance. |
| `muscle_taper_power` | Controls tendon-to-belly taper. |
| `stretch_min`, `stretch_max` | Bound visual volume-preserving bulging at extreme stretch; physical lengths remain unchanged. |
| `joint_radius_fraction` | Joint-surface radius relative to the adjacent bone dimension. |
| `radial_segments`, `bone_rings` | Procedural mesh resolution; lower for cheaper geometry, higher for smoother surfaces. |
| `proxy_radial_segments`, `proxy_stacks` | Distant world-proxy sphere resolution (defaults 8 and 6). Lower for less GPU work while retaining every creature; these do not simplify inspected or nearby articulated anatomy. |
| `bone_waist`, `torso_rounding` | Cosmetic bone taper and torso bevel. |
| `bone_color`, `gene_tint` | Neutral bone pigment and inherited-color contribution. |
| `muscle_color`, `tendon_color`, `active_color` | Tissue pigments and reflected-light modulation during measured activation; no emissive muscle light. |
| `ground_color`, `grid_color` | Arena floor and fixed-world contact-grid colors. |
| `contact_grid_spacing`, `contact_grid_radius` | Grid scale and extent for seeing slip/contact. |
| `inspector_shadows` | Body-view contact shadows; enable for contact inspection, disable if measuring their rendering cost. |

The editable genotype additionally contains per-bone dimensions/density/recursion,
per-joint limits/scale/reflection/attachments, per-muscle endpoints/area/material
scales, sensors, motor/ethology networks, mutation rates and life-history genes.
These are saved inherited values, not hidden world constants. Inspect them with
`bestiary genome show ... --json`, edit supported controls in the Designer, and
measure the resulting body before release.

## Training (`config/training.ron`)

Training settings are separate from world configuration: they never reach a world
file, trait cache or state hash. Pass a modified copy with
`bestiary train … --training PATH`. Compare settings with the same body, task and
`--seed`, and read the best and median curves together.

| Field | Default | Effect and symptom to investigate |
| --- | --- | --- |
| `population` | 50 | Individuals per generation (2–512). Too small: the best curve stalls for many generations and depends heavily on the seed. Larger costs proportionally more physics per generation. |
| `elite_fraction` | 0.2 | Fraction kept unchanged as parents. Too high: little exploration and the curve rises slowly. Too low: good controllers are lost to a few lucky mutants. |
| `generations` | 50 | Budget for headless `bestiary train`; the viewer runs until paused. |
| `slope_angle` | 0.2 | Incline in radians for the Slope task only. |
| `weight_rate` | 0.15 | Chance each motor connection is perturbed per child. Too high: the median collapses towards zero while the best stays flat (most children lose a working gait). Too low: a slow, step-like curve. |
| `weight_sigma` | 0.6 | Size of each weight perturbation. Raise when a zeroed or flat brain never starts moving; lower when the best curve is jagged and regresses through elitism. |
| `oscillator_sigma` | 0.03 | Size of per-bone oscillator frequency, phase and bias perturbations, applied to every bone of every child. Raising it quickly scrambles multi-limb coordination: a hexapod's best curve went flat at 0.15. Zero freezes gait timing. |
| `activation_rate` | 0.05 | Chance per bone that one motor neuron's activation function is redrawn. Large changes in behaviour; keep it low. |
| `body_mutation` | false | Also applies one morphology operator per child. Expect a noisier curve and bodies that leave the source's archive cell. |

## Experiment configurations and current results

- `config/demo.ron`: seed-42 diversification candidate with 24 original founders,
  mating epoch 5,000 and mutation probability 0.2. At 1M ticks it retained five
  species and a 15-part species founder, but reached only 5/128 archive cells
  (3.90625%). It does not meet the 15% archive requirement.
- `config/demo-baseline.ron`: preserved pre-resumption demo configuration, including
  the higher proposal rate; its fresh partial run reached four cells. Historical
  earlier-physics 10.2% coverage is not evidence for current acceptance.
- `config/exploration.ron`: broader compatibility and higher mutation experiment;
  its saved partial run also reached four cells. Do not present a stopped partial
  run as a completed million-tick study.
- `config/trophic.ron`: explicit trophic diagnostic candidate, not a proven balanced
  default. Four real tuning variants explored tick duration, refuge and attacks.
  The complete five-seed study had 0/5 persistent food chains, and the herbivore
  series did not establish an arms race.
- `config/validation.ron`: finite-life reproductive stability/accounting control
  with mutation probability zero and a 64-agent cap. It passes five seeds at 1M
  ticks with real births/deaths and repeated hashes; it does not test evolving
  diversity. Mutation-enabled stagnation runs use `config/demo.ron` instead.

Record seed, source revision, configuration, machine, command, initial population,
measurement windows, hashes and observed failures with each run. Use `make check`
after code changes and `make bench` after changing the world tick; preserve the
full numeric acceptance tests. See [RUNBOOK.md](RUNBOOK.md) for commands and
[evidence/v1](evidence/v1/README.md) for the recorded experiments.
