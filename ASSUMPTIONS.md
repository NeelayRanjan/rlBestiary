# Assumptions and decisions

- The existing local repository had no commits or files; it now uses upstream main at e77d504 as its starting point.
- Rust is installed under ignored `.tools/` because this machine has no Rust installation. `scripts/cargo` selects that toolchain or falls back to the user's Cargo.
- Direct dependency versions are exact and the lockfile pins transitive dependencies; dependencies enter each crate when first needed.
- M0 binary entry points explicitly report that implementation is pending and exit unsuccessfully; they are scaffolding, not completed features.

## Deviations

None implemented.
