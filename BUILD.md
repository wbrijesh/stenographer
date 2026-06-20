# BUILD.md — Build, Sign, Install & Acceptance Guide

**Stenographer** is a lightweight, **offline, macOS-only** speech-to-text app built with
**Tauri 2 + Rust** (React/Vite/Tailwind/Zustand frontend). Press a key, speak, and your words
are pasted into the focused app. This guide covers everything from a clean machine to a signed,
installed `.app` that passes the v1 acceptance suite.

For the deeper companion docs referenced throughout:

- **`PLAN.md`** — product requirements and the v1 definition of done.
- **`CONTRACTS.md`** — module interfaces and the settings schema.
- **`TESTING-AEROSPACE.md`** — the headline Aerospace overlay manual test.
- **`PERF.md`** — idle-footprint design and the exact measurement method.

---

## 1. Prerequisites

| Requirement | Notes |
|---|---|
| **macOS 13+** | `bundle.macOS.minimumSystemVersion` in `src-tauri/tauri.conf.json` is `"13.0"`. |
| **Xcode Command Line Tools** | `xcode-select --install`. Provides the Apple toolchain + `codesign`. (Full Xcode is only needed to create the free signing cert — see §4.2.) |
| **cmake** | **Required** — `transcribe-rs`/`whisper.cpp` build it natively. `brew install cmake`. Builds fail without it. |
| **Rust toolchain** | Install via [rustup](https://rustup.rs). Binaries land in `~/.cargo/bin`; make sure it's on `PATH` (`export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"`). |
| **Node + npm** | Used to install JS deps and run the Tauri CLI. The repo ships a `bun.lock` (so `tauri.conf.json` uses `bun run …` hooks), **but bun is optional** — `npm` works for everything below. |

> The dependency tree builds entirely on **upstream crates** — there are no Handy Tauri forks to
> vendor. A first build compiles `whisper.cpp`/Metal and the full crate graph, so expect it to be
> slow the first time; subsequent builds are incremental.

Quick sanity check:

```sh
xcode-select -p          # should print a developer dir
cmake --version          # must succeed
rustc --version          # via ~/.cargo/bin
node --version && npm --version
```

---

## 2. Development

```sh
# from the repo root
npm install                 # install JS deps (one time / on lockfile change)

npm run tauri dev           # full app, hot-reloading frontend + Rust  (LONG-RUNNING)
```

`npm run tauri dev` is **long-running** — it blocks the terminal and stays up until you quit it.
Do not use it inside automated/CI flows; build a real binary instead (§3).

Other useful targets:

| Command | What it does |
|---|---|
| `npm run build` | Frontend-only build (`tsc && vite build` → `dist/`). Fast; no Rust. |
| `cd src-tauri && cargo build` | Backend-only debug build → `src-tauri/target/debug/stenographer`. |
| `cd src-tauri && cargo build --release` | Backend release build (LTO; slow). |
| `cd src-tauri && cargo test export_bindings_test` | Regenerate the typed IPC bindings (`src/bindings.ts`) from the Rust command surface. Run after changing commands. |

> If `cargo build` can't find `cmake`, see §8.

---

## 3. Building a Release

```sh
npm run tauri build                      # full bundle: signed .app + DMG
# or, just the .app (skip the DMG):
npm run tauri build -- --bundles app
```

Output lands under:

```
src-tauri/target/release/bundle/
├── macos/Stenographer.app
└── dmg/Stenographer_0.1.0_<arch>.dmg     # only when DMG bundling is enabled
```

`bundle.targets` is `"all"` in `tauri.conf.json`, so the default build produces both the `.app`
and the DMG. The build runs `bun run build` as its `beforeBuildCommand` (the frontend step) before
compiling and signing the Rust binary.

---

## 4. Code Signing

This is the most important section for day-to-day use. macOS ties **TCC permission grants**
(Microphone, Accessibility) to a binary's **code identity**. If that identity changes between
builds, the OS treats the app as a new app and the grants do not carry over.

The signing identity is set in `src-tauri/tauri.conf.json` → `bundle.macOS.signingIdentity`.
Today it is `"-"` (ad-hoc). `bundle.macOS.hardenedRuntime` is `true`, and
`bundle.macOS.entitlements` points at `src-tauri/Entitlements.plist` — both are already wired
(see §5).

### 4.1 Ad-hoc signing (`"-"`) — default, $0, no Apple account

```jsonc
// src-tauri/tauri.conf.json  (current default)
"signingIdentity": "-"
```

- **Pros:** builds locally with **no Apple ID and no cost**.
- **Con (important):** an ad-hoc signature does **not** give a stable code identity. macOS TCC
  grants (Microphone / Accessibility) are tied to the binary's identity and **do not reliably
  persist across rebuilds**. After most rebuilds you'll have to **re-grant** Microphone and
  Accessibility (System Settings → Privacy & Security), and may need to remove the stale entry
  first. Fine for a one-off; painful for iterative development.

### 4.2 Free Apple Development certificate — recommended, $0, no paid program

A **free** "Apple Development" certificate (available with any Apple ID via a Personal Team — no
paid Apple Developer Program required) gives a **stable code identity**, so Microphone and
Accessibility grants **persist across rebuilds**. This is the recommended setup for anyone
building Stenographer more than once.

**Steps:**

1. Open **Xcode → Settings → Accounts**. Click **+** and add your Apple ID (sign in).
2. Select the account, click **Manage Certificates…**, then the **+** in the lower-left and
   choose **Apple Development**. Xcode creates and installs the cert into your login keychain.
3. Find the identity string:

   ```sh
   security find-identity -v -p codesigning
   ```

   Look for a line like:

   ```
   1) AB12CD34...40hex...  "Apple Development: you@example.com (TEAMID)"
   ```

4. Set `signingIdentity` in `src-tauri/tauri.conf.json` to **either** the full identity name
   **or** the 40-character SHA-1 hash:

   ```jsonc
   "macOS": {
     "signingIdentity": "Apple Development: you@example.com (TEAMID)"
     // or: "signingIdentity": "AB12CD34...40hex..."
   }
   ```

5. Rebuild (`npm run tauri build`). The `.app` is now signed with your stable identity. The first
   launch still prompts for Microphone + Accessibility, but those grants **survive subsequent
   rebuilds** because the identity no longer changes.

> `hardenedRuntime: true` and the microphone / audio-input entitlements (§5) are already set, so
> the only change needed to move from ad-hoc to a stable identity is `signingIdentity`.

---

## 5. Entitlements & Info.plist

Two files carry the macOS metadata the app needs at runtime; both are already configured and
referenced by `tauri.conf.json`:

- **`src-tauri/Entitlements.plist`** — grants the hardened runtime the microphone capability:
  - `com.apple.security.device.microphone`
  - `com.apple.security.device.audio-input`
- **`src-tauri/Info.plist`** — carries `NSMicrophoneUsageDescription` (the string shown in the
  microphone permission prompt: *"Request microphone access to transcribe audio locally"*).

No edits are needed here for a normal build; they're documented so you know where mic access is
declared if you need to adjust the prompt text or entitlements.

---

## 6. Installing

1. Build the `.app` (§3).
2. Copy it to Applications:

   ```sh
   cp -R src-tauri/target/release/bundle/macos/Stenographer.app /Applications/
   ```

   (Or open the DMG and drag it across.)
3. Launch **Stenographer** from `/Applications` (or Spotlight).

Stenographer is an **Accessory** (menu-bar) app — there is **no Dock icon**. Confirm it started
by looking for its **menu-bar icon**. The Settings window opens on demand from the tray menu.

---

## 7. First-Run Onboarding

On first launch the onboarding flow walks you through getting to a working setup:

1. **Microphone permission** — prompted on first record need; macOS shows the
   `NSMicrophoneUsageDescription` text. Grant it.
2. **Accessibility permission** — required for the Fn-key event tap and for pasting (Cmd+V) into
   other apps. Grant Stenographer under **System Settings → Privacy & Security → Accessibility**.
3. **Model download** — pick and download a transcription model. The default/recommended model is
   **`parakeet-tdt-0.6b-v3`**. Models are stored under **`<app_data>/models/`** (the app's data
   directory) and verified by SHA-256 after download.
4. **Globe (🌐 / Fn) key setting** — macOS must have **System Settings → Keyboard → "Press 🌐
   to…" set to "Do Nothing"**, otherwise macOS intercepts the Fn key before Stenographer's event
   tap. The app **detects** the current setting (via the `check_fn_key_behavior` command) and
   **nudges** you to change it if it isn't "Do Nothing".

**Triggers (once set up):**

| Gesture | Action |
|---|---|
| **Hold Fn (≥ 250 ms)** | Push-to-talk — records while held; transcribes + pastes on release. |
| **Fn + Space** | Start a hands-free **toggle** session. |
| **Tap Fn (< 250 ms)** | Stop the **active** session (toggle or PTT). Tap with nothing active = no-op. |

The hold/tap threshold is configurable (`hold_tap_threshold_ms`, default 250).

---

## 8. Acceptance Tests (v1 Definition of Done)

The v1 acceptance suite (from `PLAN.md` → "Final acceptance"). All must pass:

1. **Free-account build + persistent permissions.** Builds and runs signed with a **free Apple
   Development cert** (§4.2); Microphone + Accessibility grants **persist across a rebuild**
   (the proof that the code identity is stable).
2. **Triggers paste into the focused app.** **Hold-Fn push-to-talk**, **Fn+Space toggle**, and
   **tap-Fn stop** each transcribe and paste text into the currently focused application.
3. **Aerospace-correct overlay.** The recording overlay appears on the user's **current Aerospace
   workspace** (bottom-center of the monitor under the cursor), with **no Space switching** and
   **no focus steal**, on any workspace. ⇒ Run the full headline procedure and repeat matrix in
   **`TESTING-AEROSPACE.md`**. A single Space-switch or focus-steal is a FAIL.
4. **Minimal idle footprint.** Accessory mode (no Dock icon), overlay hidden when idle, and the
   model **loaded on demand / unloaded on idle** (default `model_unload_timeout` = 5 min). Idle
   RAM should be in the low tens of MB with the model unloaded and ~0% CPU. ⇒ Measure using the
   exact commands and method in **`PERF.md`** (§2 build + idle-sample snippet; sum the main
   process + WKWebView helper RSS).
5. **Model & preferences management.** From the **Settings** window you can **download / select /
   delete** models and change preferences, with download progress shown.

> Tests 3 and 4 cannot be reproduced in CI — they require real hardware (Aerospace with multiple
> workspaces / a GUI session). Use the companion docs for the step-by-step procedures.

---

## 9. Troubleshooting

**`cmake` not found / `transcribe-rs` or `whisper.cpp` fails to build.**
Install it: `brew install cmake`. Ensure `/opt/homebrew/bin` is on `PATH` for the build shell
(`export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"`), then rebuild.

**Microphone / Accessibility permissions don't persist after a rebuild.**
You're almost certainly using **ad-hoc** signing (`signingIdentity: "-"`), whose identity changes
each build. Switch to the **free Apple Development cert** (§4.2) for a stable identity. If a stale
permission entry is blocking the re-grant, remove Stenographer from the relevant list in
**System Settings → Privacy & Security** and grant it again.

**Fn push-to-talk does nothing / triggers the emoji picker or dictation.**
The Globe key is being intercepted by macOS. Set **System Settings → Keyboard → "Press 🌐 to…"**
to **"Do Nothing"**. You can cross-check the underlying value with
`defaults read com.apple.HIToolbox AppleFnUsageType` (`0` = "Do Nothing"). Also confirm
Stenographer has **Accessibility** permission — the Fn event tap requires it. See
`TESTING-AEROSPACE.md` §3 for the full check.

**No menu-bar icon after launch.**
Stenographer is an Accessory app with **no Dock icon** — look in the menu bar, not the Dock. If
it isn't there, check `/tmp` logs from a manual run (`src-tauri/target/release/stenographer`),
or relaunch.

**Where are downloaded models?**
Under **`<app_data>/models/`** in the app's data directory. Re-downloads resume and are SHA-256
verified; deleting a model from Settings removes it from this directory.

**The overlay yanks me to another Aerospace workspace or steals focus.**
That's the exact bug the Aerospace work guards against — capture the failing row from the
`TESTING-AEROSPACE.md` matrix (launch WS → record WS) when reporting it.
