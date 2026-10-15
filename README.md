# bestiary

THE INSPO BTW:
https://keiwan.itch.io/evolution




A desktop artificial-life simulator where 3D creatures with evolvable bodies live,
hunt, reproduce, and speciate in a persistent world. Creatures are built from
bones, joints, and contractile muscles, arranged as a recursive graph, so mutation
can add a segment, a limb, or a symmetry, and new body plans appear over long runs
instead of better versions of the one you started with. Nothing is driven by joint
motors: leverage and timing are problems evolution has to solve. Every body is evaluated in real rigid-body physics; the world then runs on
the traits that physics measured, which is what makes thousands of creatures and
articulated 3D bodies affordable at once.

You can watch, inspect any creature's body and brain, browse the phylogeny and the
archive of every body plan discovered, design a creature by hand and release it,
and leave a world running for days. Linux, Windows, and macOS. No network, no
accounts, no telemetry.

Ancestry: Karl Sims' evolved virtual creatures (1994) for the encoding, Keiwan's
Evolution for the design-and-watch loop, The Bibites for the ecosystem.

This repo currently holds the specification only. The code is built by an agent
from the prompts below.

## Files

- `SPEC.md` — the specification: architecture, genome encoding, the combine
  harness, the world model, speciation and the archive, viewer, determinism rules,
  milestones, performance budget, tests, tuning, completion checklist, and the
  phase two and phase three work the v1 seams are placed for.
- `AGENTS.md` — operating rules for the agent (read automatically by Codex and
  Claude Code).
- `PROMPT.md` — the prompt to paste to start the build, and the one to paste if a
  fresh session has to resume it.

## Building it

1. Clone this repo and open it in Codex (or Claude Code) with the most autonomous
   approval mode available and network access for crate installs. Nothing in the
   spec needs an API key, an account, or a paid service.
2. Paste the Start prompt from `PROMPT.md`.
3. When the usage cap hits, press reset and let it continue. If the same session
   resumes, do nothing. If you land in a new session, paste the Resume prompt.
   `PLAN.md` and the per-milestone commits are what make that work.

The milestones are ordered so an interrupted run still leaves something usable:
physics core, genome, combine harness, and the headless world come before the
viewer and the designer. M1 is a hard performance gate, deliberately placed before
anything is built on top of it.

## After the build

The agent can prove the simulation is deterministic, conservative, stable, and
fast. It cannot tell whether the world is interesting, which is the part left to
you:

1. `make dist`, then create a world and run it headless for a few hours.
2. Attach the viewer, read the phylogeny and the archive, and see which niches
   filled and which stayed empty.
3. Tune. `TUNING.md` names the four failure modes (the sessile-sipper equilibrium,
   monoculture, predator-prey collapse, late-run stagnation) and which knob moves
   each one. Every constant lives in `config/world.ron`.
4. Design a creature in the editor and release it into a world that already has
   incumbents. This is the best test of whether the ecology has teeth.

`RUNBOOK.md` covers all of this in detail once the build has produced it.
