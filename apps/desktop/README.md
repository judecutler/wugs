# wugs desktop shell

The desktop face of wugs, starting with Tongues (fieldwork): a Tauri 2 app
with a Vite and vanilla TypeScript frontend. Panels dock and rearrange
(dockview-core), and the arrangement is saved. You can open or create a
project, list its texts and sources, search, read and edit a text's segments,
and pair devices and sync between them.

Every write goes webview → Tauri command → `wugs-core` intent → op log. The
webview never touches the projection or the log directly.

## Run it

Prerequisites: Rust, Node 20 or later, and the Tauri system libraries for
your platform (on Linux, the WebKitGTK 4.1 and GTK 3 development packages).
See <https://tauri.app/start/prerequisites/>.

```sh
cd apps/desktop
npm install
npm run tauri dev
```

To check the parts without a display:

```sh
npm run build                       # tsc and vite
(cd src-tauri && cargo test)        # session-layer integration tests
npm test                            # the redraw rule, under node
```

## Layout

- `src/api.ts` holds the typed wrapper for each Tauri command. It is the only
  module that calls `invoke`.
- `src/store.ts` holds the app state the panels read and the actions that
  change it.
- `src/panels/` holds one module per panel; `registry.ts` names them all.
  A panel id is also its dockview component name.
- `src/dock.ts` mounts dockview, lays out the default and saves each change,
  flushing a pending save when the window closes.
  `src/main.ts` holds the top bar and the toast.
- `src/redraw.ts` decides when a panel redraws: never for unchanged content,
  and not while the person is typing in it (the timeline catches up when
  focus leaves).
- `src-tauri/src/layout.rs` keeps saved layouts at
  `<app data>/layouts/<kit>.json` (`tongues.json` today), as dockview's
  `toJSON()`. A layout is a kit preset. Delete the file to start from the
  default.
- `src-tauri/src/identity.rs` keeps the device key in the OS keychain, or in
  `<app data>/device-key` when no keychain is usable; `<app data>/key-store`
  records which, and the Devices panel shows it.
- `src-tauri/src/session.rs` holds what each command does, as plain functions
  over the open project. Opening a project takes an advisory lock on
  `<project>/.wugs-lock`, so a second process cannot open the same folder;
  `tauri-plugin-single-instance` sends a second launch to the running window.
  The tests in `src-tauri/tests/` call these functions directly, with no
  Tauri runtime.
- `src-tauri/src/listen.rs` is the sync listener's accept loop. A connection
  reaches the session thread only after its first bytes arrive, and
  `wugs_net::serve_stream` bounds it from there (`HANDSHAKE_DEADLINE`, then
  `SESSION_DEADLINE`). Dropping the listener closes the port.
- `src-tauri/src/lib.rs` holds the one-line command wrappers and the session
  thread. `Project` is not `Send`, so one thread owns it and runs each
  command on it in order.

## Unauthenticated sync, for development

`sync_now` runs the authenticated session. A debug build run with
`WUGS_DEV_OPEN_SYNC=1` uses the old plain-TCP session instead, which anyone
who can reach the port can use. Release builds ignore the variable.

## Outside the root workspace

`src-tauri/Cargo.toml` has its own empty `[workspace]` table. The root
`cargo build --workspace` and `cargo test --workspace` never compile Tauri,
so they keep working on machines without GTK or WebKitGTK. Build and test this
crate from `src-tauri/`.

## Not here yet

The waveform and timeline canvas (PRD gate 2, TONGUES-UI step 2): the
Timeline panel is a frame showing the segment table until then. When it lands,
waveform peaks are computed in Rust and never in the webview.
