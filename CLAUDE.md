# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

flixparty is a Rust (edition 2024) tool that syncs play/pause across PCs, shipped as a GUI
(`flixparty`) and a CLI (`flixparty-cli`). Every client subscribes to a shared Redis pub-sub
channel. When the configured **toggle** key (default `P`) is pressed locally, a `toggle_play`
message is published. Every client, including the sender, reacts to that message by simulating
the **playback** key in the focused window. The toggle and playback keys must be different, and
`Config::validate()` checks this. rdev can't tell real key presses from simulated ones, so using
the same key for both would cause a feedback loop.

## Commands

```sh
cargo build                                # debug build of all crates
cargo run -p flixparty-gui                 # GUI; settings are stored in the OS config dir
cargo run -p flixparty-cli -- [config]     # CLI; config defaults to ./flixparty.config.toml
cargo test                                 # unit tests
cargo test -p flixparty-core -- --ignored  # session tests, need the docker compose Redis
cargo fmt                                  # uses .rustfmt.toml (some options are nightly-only: cargo +nightly fmt)
cargo clippy --workspace
docker compose up -d                       # local Redis on 127.0.0.1:16379, password "foobar" (matches example config)
```

On Windows, linking the GUI needs MSVC from VS 2022 or newer, because Skia's prebuilt libraries
use newer STL symbols than VS 2019 has.

Releases are built by `.github/workflows/relese.yaml` when a `vX.Y.Z` tag is pushed. It
cross-compiles the CLI for Linux (musl/gnu), Windows, and macOS targets, and the GUI for the
targets with prebuilt Skia binaries (`gui: true` in the matrix). Builds use `--locked`, so keep
`Cargo.lock` in sync. Linux builds need X11 dev libs (libx11, libxdo, libxtst, libxi, …) because
of rdev.

## Architecture

Cargo workspace with three crates:

- `crates/core` (`flixparty-core`): everything except the UI.
  - `session.rs`: `Session::start(cfg, on_event)` runs a session in a background thread and
    reports `Event`s (status, members, toggles) to the callback. `stop()` ends it; blocking loops
    poll the stop flag every `POLL_INTERVAL` (the pub-sub connection has a read timeout for this).
    Connecting runs inside a `Retry` loop; unrecoverable Redis error kinds (auth, parse, client
    config, …) end the session right away. A session opens two Redis connections (publish +
    subscribe), and a trigger thread publishes `TogglePlay` when the toggle key is pressed.
  - Presence: clients send `hello` on connect, answer `hello` with `here`, send `here` as a
    heartbeat every 10s and `bye` on disconnect. Members without a heartbeat are dropped after 35s.
    This runs on a separate `<channel>:presence` channel so older clients never see it.
  - `periphery.rs`: wraps rdev. rdev supports only one global hook per process, so `subscribe()`
    starts a single listener thread and fans key presses out to all subscribers. Callbacks run on
    the hook thread and must return fast (Windows removes slow hooks), so they only send on a
    channel. `simulate_press()` sends a press and release. `key_name()` gives display names.
  - `model.rs`: the JSON wire format. Main channel: `{sender, op, name?}`, where `op` is
    `toggle_play` or `introduce` (snake_case). Older clients fail on unknown `op`s, so never add
    variants to `Op`; new message kinds go on the presence channel (`PresenceMessage`).
    Unknown/invalid messages are logged and skipped.
  - `retry.rs`: an iterator that gives up after more than `max` retries arrive within `threshold`
    of each other. Spacing retries further apart than `threshold` earns back budget.
  - `config.rs`: figment + TOML, also `Serialize` so the GUI can save it. Defaults are set per field
    with `#[serde(default = ...)]`. `connection.address` is either `host:port` (with `tls`,
    `username`, `password`) or a legacy `redis://` URL. `Connection::connection_info()` builds the
    redis `ConnectionInfo`. TLS uses rustls with the ring provider (installed in `Session::start`).
    The default playback key depends on the platform: media play/pause (`Unknown(179)`) on Windows,
    `Unknown(172)` on Linux, and `Space` on macOS, since rdev can't simulate NX media keys there.
  - `condition/`: platform-specific `is_browser_in_focus(class, title_contains)`, selected with
    `cfg_if` in `mod.rs`. It returns true if the configured class or title matches, or if the
    foreground window looks like a known browser. `block_trigger` (default true) applies the check
    to local key presses; `block_receive` (default false) applies it to incoming messages.
    - Windows: Win32 `GetForegroundWindow` / `GetWindowTextW` / `GetClassNameW`.
    - macOS: the `frontmost` crate keeps a global with the frontmost app name. Its observer has to
      be registered on the **main thread**, which must then run an NSRunLoop. The CLI runs the
      session in a spawned thread and calls `condition::start_watcher()` on the main thread. The
      GUI calls `condition::init_watcher()` before launching, and the winit event loop runs the
      run loop.
    - Other platforms: always true, with a warning if conditions are configured.
- `crates/cli` (`flixparty-cli`): loads a config file and runs a `Session` until it ends.
- `crates/gui` (`flixparty-gui`, binary `flixparty`): [freya](https://docs.rs/freya) 0.4 UI.
  - `state.rs`: all UI state is global `State`s (`Globals`), shared by the main and log windows.
    Views are plain functions without hooks, so switching views can't break hook order. Background
    sources (session events, log lines, key presses) are sent over futures channels and applied
    to the state by `pump_*` futures registered with `LaunchConfig::with_future`.
  - `main.rs` passes a custom winit event loop with `DeviceEvents::Never`. winit's default raw
    keyboard input registration keeps Windows from calling the process's own low-level keyboard
    hook while a flixparty window is focused, so keys couldn't be recorded. Don't remove it.
  - `connection.rs`: owns the running `Session` (outside UI state so it can be shut down after the
    event loop exits). Events are tagged with a session generation, so stale events from a
    session that's still stopping are ignored.
  - `logging.rs`: a tracing `MakeWriter` that captures formatted lines for the log window.
  - `settings.rs`: loads and saves the config as TOML in the OS config dir (`directories` crate).

## Notes

- `flixparty.config.toml` is the documented example config that ships in the repo. Keep its
  comments up to date when you change config fields or defaults.
- `private.*.toml` files are gitignored and meant for local configs with real credentials.
