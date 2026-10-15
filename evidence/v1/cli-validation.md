# Fresh-world CLI acceptance

Command: `./scripts/check-cli` (release binaries; local IPC and GPU access).
Machine: Apple M4 Pro, arm64, macOS 15.3.1, Python 3.9.6.
Code base: `d4932b1` plus the documented proxy-only viewer LOD change; exact CLI and viewer SHA-256 hashes are in `cli-summary.json`.

All 30 commands/assertions passed. A new seed-42 world advanced from tick 0 to
2,020; live IPC status was observed during a paced run. File and share-string
combines returned identical traits and replay bytes. Genome mutation/crossover,
validation/show, archive sorting/export, and phylogeny JSON/DOT all worked.
Archive manifest floats were compared as exact stored f32 values because the two
JSON serialization paths use different decimal representations.

Recorded-ID and recorded-file exports each produced 61 animated 2,880×1,920 PNG
frames plus complete metadata and ffmpeg sidecars. Every PNG passed signature,
CRC, decompression, nonblank-frame and size checks. World database bytes and
canonical state hash were unchanged by inspection/export. CLI `bench --report
--json` passed at 0.788657666 seconds for the 12-part combine.

Detailed commands and outputs: `cli-commands.log`. Machine/binary provenance and
structured assertions: `cli-summary.json`. Fresh local outputs:
`var/v1/cli-2ygt02po/`. This short command audit is separate from million-tick
stability and evolutionary acceptance.
