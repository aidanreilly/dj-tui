# Working on dj-tui

Spec and milestone plan: `docs/spec.md`. M0 and M1 are done; M2 (mixer EQ, filter, trim,
meters, split-mono routing) is next.

## Rules

- Test first, always. Write the failing test, run it and see it fail, then write the code.
  Commit per feature with red and green in the same commit or as consecutive commits.
- Behaviour tests go in each crate's `tests/` and use only the public API.
- `engine` does no I/O and never allocates in `EngineProcessor::process`.
  `crates/engine/tests/no_alloc.rs` enforces this; extend it when adding commands or DSP.
- Old tracks leave the audio thread through the garbage ring, never by drop.
- The UI owns absolute values of continuous controls (`ControlState` in `src/apply.rs`) and
  sends absolute `Set*` commands.
- User-facing text follows the style of the existing messages: plain, specific, no jargon.

## Commands

```sh
cargo test --workspace
cargo run -- --demo
cargo run -- --no-audio some.flac
# JACK integration test:
jackd -n test -d dummy -r 48000 -p 256 -P 4 -C 0 &
JACK_DEFAULT_SERVER=test DJ_TUI_JACK_TESTS=1 cargo test -p backend --test jack_dummy
```

## Toolchain

MSRV is 1.82. `Cargo.lock` pins encoding_rs, indexmap, instability, unicode-segmentation
and tempfile to releases that build on 1.82. On a newer toolchain `cargo update` is fine, but
keep the MSRV CI job green or raise `rust-version` deliberately.
