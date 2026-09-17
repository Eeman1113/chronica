# Chronica Engine (Rust rewrite)

The emergent-world engine rebuilt from the `index.html` prototype. Docs in `docs/`;
stage-by-stage history in `docs/STAGE_REPORTS.md`.

## Run

```sh
# (use rustup's toolchain: rustc 1.88+)
cargo run --release -p chronica-renderer          # the world, live (optional seed arg)
cargo run --release -p sim_runner -- --seed 7 --days 3600 --stats-every 360 --save world.crn
cargo run --release -p world_inspector -- world.crn                # chronicle
cargo run --release -p world_inspector -- world.crn why 12345      # causal chain
cargo run --release -p world_inspector -- world.crn person 3       # derived biography
cargo run --release -p replay -- verify world.crn                  # bit-identical replay check
cargo test --release --workspace                                   # determinism + emergence suite
```
