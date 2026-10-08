# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

flixparty is a single-binary Rust CLI (edition 2024) that syncs play/pause across PCs. Every client
subscribes to a shared Redis pub-sub channel. When the configured **toggle** key (default `P`) is
pressed locally, a `toggle_play` message is published. Every client, including the sender, reacts
to that message by simulating the **playback** key in the focused window. The toggle and playback
keys must be different, and `run()` checks this. rdev can't tell real key presses from simulated
ones, so using the same key for both would cause a feedback loop.

## Commands

```sh
cargo build                      # debug build
cargo run -- [path/to/config]    # config defaults to ./flixparty.config.toml
cargo test                       # unit tests (currently only src/retry.rs)
cargo test retry_with_reset      # run a single test by name
cargo fmt                        # uses .rustfmt.toml (some options are nightly-only: cargo +nightly fmt)
cargo clippy
docker compose up -d             # local Redis on 127.0.0.1:16379, password "foobar" (matches example config)
```

Releases are built by `.github/workflows/relese.yaml` when a `vX.Y.Z` tag is pushed. It
cross-compiles Linux (musl/gnu), Windows, and macOS targets with `--locked`, so keep `Cargo.lock`
in sync. Linux builds need X11 dev libs (libx11, libxdo, libxtst, libxi, …) because of rdev.

## Architecture

- `main.rs`: `run()` loads the config, sets up tracing, then calls `connect()` inside a `Retry`
  loop. Unrecoverable Redis error kinds (auth, parse, client config, …) end the program right away.
  `connect()` opens two Redis connections (publish + subscribe), sends an `Introduce` message, and
  starts a thread that listens for key presses and publishes `TogglePlay`. The main thread then
  loops over pub-sub messages and simulates the playback key. `last_local_trigger` is used only to
  log the round-trip time when a client receives its own message.
- `periphery.rs`: wraps rdev. `listen()` starts its own thread running rdev's global hook and
  forwards toggle-key presses over an mpsc channel. `simulate_playback_press()` sends a press and
  release.
- `model.rs`: the JSON wire format `{sender, op}`, where `op` is `toggle_play` or `introduce`
  (snake_case). Changing it breaks compatibility with clients running older versions.
- `retry.rs`: an iterator that gives up after more than `max` retries arrive within `threshold`
  of each other. Spacing retries further apart than `threshold` earns back budget.
- `config.rs`: figment + TOML. Defaults are set per field with `#[serde(default = ...)]`. The
  default playback key depends on the platform: media play/pause (`Unknown(179)`) on Windows,
  `Unknown(172)` on Linux, and `Space` on macOS, since rdev can't simulate NX media keys there.
- `condition/`: platform-specific `is_browser_in_focus(class, title_contains)`, selected with
  `cfg_if` in `mod.rs`. It returns true if the configured class or title matches, or if the
  foreground window looks like a known browser. `block_trigger` (default true) applies the check
  to local key presses; `block_receive` (default false) applies it to incoming messages.
  - Windows: Win32 `GetForegroundWindow` / `GetWindowTextW` / `GetClassNameW`.
  - macOS: the `frontmost` crate keeps a global with the frontmost app name. Its NSRunLoop has to
    run on the **main thread**, so on macOS `main()` starts `run()` in a spawned thread and calls
    `condition::start_watcher()` on the main thread.
  - Other platforms: always true, with a warning if conditions are configured.

## Notes

- `flixparty.config.toml` is the documented example config that ships in the repo. Keep its
  comments up to date when you change config fields or defaults.
- `private.*.toml` files are gitignored and meant for local configs with real credentials.
