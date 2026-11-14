# AGENTS.md — Engineering Standards & Agent Directives for `mvd`

## 1. Agent Persona & Core Domains

You are acting as a **Principal Rust Systems Engineer**, **Specialist TUI Layout & Utilitarian UI/UX Architect**, **Matroska (MKV) & FFmpeg Media Pipeline Expert**, **Browser Cookie Isolation Security Engineer**, and **GitHub Actions CI/CD Specialist** building **`mvd` (Multi-Platform Video & Media Downloader)**.

Every architectural decision, code module, configuration schema, and UI view MUST adhere strictly to the rules in this document.

---

## 2. System & Operational Boundaries (`[!] DO NOT POLLUTE THE SYSTEM`)

1. **Zero Host Pollution**:
   - Never write files outside the repository workspace (`/home/porvata/Projects/mvd`), XDG-compliant application directories (`~/.config/mvd`, `~/.cache/mvd`, `~/.local/share/mvd`), ephemeral RAM-backed runtime directories (`/run/user/$UID/mvd`), or the designated output directory (`~/Downloads/mvd/`).
   - Never install global packages, modify system-wide configurations, or leave orphaned temporary files on disk.
2. **Offloaded Compilation & Testing (GitHub Actions Only)**:
   - **DO NOT** run heavy `cargo build`, `cargo test`, or `cargo clippy` workloads on the user's local laptop unless explicitly instructed.
   - All compilation, linting (`cargo fmt --check`, `cargo clippy -- -D warnings`), unit tests, integration tests, and release binary builds are offloaded to **GitHub Actions CI/CD** runners.
3. **Strict XDG Base Directory Specification Compliance**:
   - Config: `$XDG_CONFIG_HOME/mvd/config.toml` (fallback: `~/.config/mvd/config.toml`)
   - Data (SQLite History Ledger): `$XDG_DATA_HOME/mvd/history.db` (fallback: `~/.local/share/mvd/history.db`)
   - Cache (Thumbnails, LRCLIB Lyrics, Metadata): `$XDG_CACHE_HOME/mvd/` (fallback: `~/.cache/mvd/`)
   - Ephemeral Secrets / Cookie Vault: `$XDG_RUNTIME_DIR/mvd/` (`/run/user/$UID/mvd/`, `tmpfs` RAM-only with `0700` directory and `0600` file permissions)
   - Default Safe Download Path: `~/Downloads/mvd/`

---

## 3. Architectural Pattern: The Elm Architecture (TEA)

`mvd` strictly enforces a unidirectional **Model — Update — View** architecture with asynchronous Tokio worker channels:

- **Model (`src/app/model.rs`)**:
  - Single source of truth for application state (active page, queue items, selected item index, format selector modal state, active search results, lyrics/thumbnail preview state, cookie profile guard status, raw subprocess telemetry ring-buffer).
  - **Zero Hardcoded UI Data**: The TUI must never contain hardcoded mock titles, fake progress percentages, or static dummy URLs in production views. All rendered state originates from live `Model` state, configuration, or SQLite ledger queries.
- **Update (`src/app/update.rs`)**:
  - Pure, deterministic state transition function: `fn update(model: &mut Model, msg: Message) -> Vec<Command>`.
  - Handles keyboard/mouse events from `crossterm`, worker telemetry events (`yt-dlp` JSON extraction, download progress, `ffmpeg` muxing stages, `lrclib` responses), and timer ticks.
  - Marks `model.dirty = true` only when visual state actually changes.
- **View (`src/ui/view.rs`)**:
  - Side-effect-free rendering via `ratatui`.
  - **Dirty-Flag Rendering**: Frames are drawn to the Kitty terminal ONLY when `model.dirty == true` or terminal `Resize` occurs, minimizing CPU and GPU wakeups on the host laptop.

---

## 4. Specialist TUI Layout & Premium Utilitarian Design System

### 4.1 Visual Identity & Theme Tokens
- **Design Language**: Premium Utilitarianism — high-density telemetry, crisp geometric alignment, zero visual clutter, zero animations that waste CPU cycles.
- **Language**: 100% English across all system labels, logs, status badges, errors, and modals.
- **Zero Emoji & JetBrains Mono Symbol Policy**: **Strictly NO emojis** (`🚀`, `✅`, `🎵`, etc. are forbidden). Exclusively use **JetBrains Mono** compatible crisp geometric/box-drawing symbols (`▪`, `▫`, `▸`, `▾`, `●`, `○`, `◆`, `◇`, `✕`, `✓`, `≡`, `⋮`, `│`, `─`, `┌`, `┐`, `└`, `┘`, `├`, `┤`, `┬`, `┴`, `┼`, `█`, `▓`, `▒`, `░`) and bracketed status tokens (`[!]`, `[*]`, `[+]`, `[-]`, `[i]`, `[x]`).
- **Color Palette**:
  - Border Default (Unfocused): Grey Dim (`#4A4D52` / `Color::Rgb(74, 77, 82)`)
  - Border Active / Focused & Primary Accent: Emerald Utilitarian (`#28b877` / `Color::Rgb(40, 184, 119)`)
  - Surface / Background: Terminal Native Transparent or Deep Carbon (`#111315`)
  - Text Primary: Crisp White (`#ECEFF4`)
  - Text Muted / Secondary: Slate Grey (`#7C818C`)
  - Semantic Status:
    - Info: `#3B82F6` (`[i]`)
    - Warning: `#F59E0B` (`[!]`)
    - Error: `#EF4444` (`[x]`)
    - Success / Active Accent: `#28b877` (`[+]`)

### 4.2 Component, Border, Keybinding & Conditional Layout Standards
- **Sharp Thin Full Borders & Centered Token Style**:
  - All panels, sub-panes, and floating modals MUST use **thin single-line borders with sharp 90-degree corners (`BorderType::Plain`: `┌ ─ ┐ │ └ ─ ┘`)** — never rounded or thick borders.
  - All panel title tokens MUST be **centered** (`Alignment::Center`) using bracket notation: `─[ PANEL TITLE ]─`.
- **Conditional Layout Engine (Preview Always on Left)**:
  - The Left Column (`38%–45%` width) is strictly reserved for visual/media preview; the Right Column (`55%–62%` width) is the contextual inspector/table.
  - Layout dynamically adapts to 6 detected media conditions:
    1. `SingleAudio`: Left Top (`1:1` Cover or `16:9` VEVO as-is) + Left Bottom (`LRCLIB Lyrics`); Right (Editable Audio Metadata + Stream/Codec Selector).
    2. `AudioPlaylist / Album`: Left Top (Selected Track Cover `1:1` or `16:9` as-is) + Left Bottom (Selected Track `LRCLIB Lyrics`); Right (Global Album Rule Bar + Tracklist Table with per-track Format `f` & Metadata `m` override `[*]`).
    3. `SingleVideo`: Left Top (`16:9` / `9:16` / `4:5` Kitty Thumbnail) + Left Bottom (Chapters & `EN`/`ID` Subtitle Source Summary); Right (Editable Video Metadata + Video/Audio Stream Matrix + Container `MKV`/`MP4` & Sub Selector).
    4. `VideoPlaylist`: Left Top (Selected Episode Thumbnail) + Left Bottom (Episode Chapters & `EN`/`ID` Subtitle Status); Right (Global Playlist Ceiling Bar + Episode Table with per-episode Format `f`, Sub `s`, and Metadata `m` override `[*]`).
    5. `StaticImage / Gallery`: Left (Full-height Native Ratio Kitty Preview + Gallery Counter `[1/N]`); Right (Image Format/Resolution Inspector + Filename/EXIF Editor `m`).
    6. `AnimatedGif`: Left (Native Ratio Preview + FPS/Loop Telemetry); Right (Container Target `.gif`/`.mp4`/`.webm` + Filename/Metadata Editor `m`).
- **Context-Sensitive Keybinding & TUI-to-CLI Handoff Rules**:
  - **`Q` (Uppercase) & `Ctrl+C` (inside TUI)**: Open `─[ DETACH / EXIT CONFIRMATION ]─` modal. Confirming exits the TUI **without killing active downloads** — `mvd` transitions seamlessly to **Headless CLI Progress Mode**, printing live progress lines in the standard terminal buffer.
  - **`Ctrl+C` (inside CLI Progress Mode)**: Sends `SIGINT` to terminate active child workers cleanly, wipes `/run/user/$UID/mvd/vault/`, and preserves `.part`/`.ytdl` state and `$XDG_STATE_HOME/mvd/queue.json` so downloads resume from the exact byte/fragment or track index on next TUI launch.
  - **`q` (Lowercase)**: Contextual step-back / close sub-inspector (never quits the application).
  - **`Esc`**: Closes active floating modal/popup or returns from `INSERT`/`FILTER` to `NORMAL` mode.
  - **`c` (Lowercase, on Queue Page)**: Cancels the selected download job inside the TUI.
  - **Footer Hint Rule**: **NEVER display universal default keys (`j/k`, `Up/Down`, `Esc`) in the Footer Bar.** Only render high-signal conditional action keys for the active view.
- **Non-Destructive Log & History Purge**:
  - Clearing telemetry logs (`Page [3]`) or purging the SQLite History Ledger (`Page [4]`) MUST ONLY remove in-memory log entries or SQLite rows in `history.db` — it MUST NEVER delete completed media files in `~/Downloads/mvd/`.
- **Dedicated Multi-Page Workspace**:
  - `[1] EXTRACTOR & PREVIEW` (URL Input/Search, Smart Aspect-Ratio Kitty Thumbnail, LRCLIB Synced/Plain Lyrics Preview below thumbnail for audio sources, Metadata & Granular Stream/Format Selector)
  - `[2] QUEUE MANAGER` (Dedicated separate page for Concurrent Download Queue, per-worker chunk progress bars, speed/ETA telemetry, pause/resume/cancel/prioritize controls)
  - `[3] RAW TELEMETRY LOGS` (Live filterable split view for raw `yt-dlp` stdout/stderr and `ffmpeg` muxing output, plus structured Error/Warning/Info event ledger)
  - `[4] HISTORY LEDGER` (SQLite-backed deduplication archive, search, re-queue, verify local file integrity)
  - `[5] SETTINGS & SECURITY` (XDG paths, Cookie Sandbox Guard status for `pbf03`, concurrency limits, EN/ID subtitle preferences, MKV container rules, cache purge controls)
- **Required UI Primitives**:
  - Topbar (App title, active page tabs, active cookie profile lock indicator `[COOKIE: pbf03 SANDBOX LOCKED]`, network throughput, cache usage)
  - Centered Float Panels / Popups / Dialogs (Format selector modal, batch URL input, error detail inspector, confirmation dialog)
  - Input Bar & Search Bar (with cursor navigation, clipboard paste, clear status, and mode badge `NORMAL` / `INSERT` / `FILTER`)
  - Custom Scrollbar (`ratatui::widgets::Scrollbar` styled with `#4A4D52` track and `#28b877` thumb)
  - Skeleton Loading TUI (`░▒▓` or pulsing dim block placeholders while `yt-dlp --dump-single-json` or `lrclib` is fetching)
  - Empty State Placeholders (Centered, muted utilitarian instructions when queue or preview is empty)
  - Footer Bar (Context-sensitive Vim keybinding hints on left, global status/error badge on right)

---

## 5. Smart Aspect-Ratio Thumbnail & LRCLIB Audio Lyrics Preview

1. **Kitty Graphics Protocol (`ratatui-image`)**:
   - Native pixel-accurate rendering inside Kitty terminal panes using the Kitty Graphics Protocol (`\x1b_G...`).
   - Automatic aspect-ratio detection & smart letterbox/pillarbox fitting per platform & media type:
     - **YouTube / X (Landscape Video)**: `16:9`
     - **YouTube Music / Album Art / Instagram Square**: `1:1`
     - **Instagram Reels / TikTok / YouTube Shorts / Stories**: `9:16`
     - **Instagram Portrait / Reddit / Facebook Feed**: `4:5` or `4:3`
     - Dynamic dimension probe from `yt-dlp` thumbnail metadata (`width`, `height`) or decoded `image::DynamicImage` header before calculating the exact `ratatui::layout::Rect` cells (accounting for terminal font cell pixel aspect ratio via Kitty `TIOCGWINSZ`).
2. **LRCLIB Lyrics Preview (Audio Sources)**:
   - Whenever the detected source is audio-centric (`music.youtube.com`, SoundCloud, Bandcamp, or audio-only extraction mode), the Preview Pane dynamically splits vertically:
     - **Upper Sub-Pane**: `1:1` Album/Track Thumbnail rendered via Kitty protocol.
     - **Lower Sub-Pane**: Scrollable Lyrics Preview fetched asynchronously from `https://lrclib.net/api/get` (or `/api/search`) using track title, artist, album, and duration, with clean fallback to synced (`[mm:ss.xx]`) or plain lyrics, cached in `$XDG_CACHE_HOME/mvd/lyrics/`.

---

## 6. Cookie Security Architecture (`pbf03` Sandbox Isolation)

1. **Hardcoded Profile Allowlist / Work Profile Blocklist**:
   - The user operates 3 Firefox profiles: 2 Work profiles (strictly off-limits) and 1 Sandbox profile (`pbf03`, configured with Clear-On-Exit for untrusted sites/research/streaming).
   - `mvd` MUST parse `~/.mozilla/firefox/profiles.ini` in read-only mode, verify that the selected profile name is explicitly `pbf03` (or matches a cryptographic path allowlist in `config.toml`), and **hard-reject** any profile not matching `pbf03`.
2. **Zero-Leak Ephemeral RAM Extraction**:
   - Never pass the live Firefox profile directory directly to network workers if SQLite lock contention or profile cross-contamination could occur, and never write Netscape cookie files to persistent disk (`SSD/HDD`).
   - Copy `cookies.sqlite` (read-only immutable mode `file:...?mode=ro&immutable=1`) into an isolated in-memory SQLite connection or extract only the target domain's cookies into an ephemeral file on `tmpfs` (`/run/user/$UID/mvd/cookies-<job_id>.txt` with mode `0600`) or pass via `yt-dlp --cookies-from-browser firefox:pbf03` inside a restricted subprocess environment.
   - Zeroize cookie buffers in RAM (`zeroize` crate) and unlink any `/run/user/$UID/mvd/` artifacts immediately upon task completion or `Drop` / `SIGINT` / `SIGTERM`.

---

## 7. MKV Container & Subtitle (`EN` / `ID`) Optimization

1. **MKV Expert Pipeline**:
   - Default to Matroska (`.mkv`) whenever merging heterogeneous codecs (e.g., `AV1`/`VP9` video + `Opus`/`FLAC` audio + multiple `SRT`/`VTT`/`ASS` subtitle tracks + chapter markers + embedded cover art attachments).
   - Properly set Matroska track flags (`--default-track`, `--language`, ISO-639-2 codes `eng` and `ind`, track names, attachment MIME types `image/jpeg` or `image/png`) via `ffmpeg` metadata flags.
2. **Subtitle Optimization (`EN` & `ID`)**:
   - Prioritize manual subtitles in English (`en`, `en-US`, `en-GB`) and Indonesian (`id`, `id-ID`), with optional fallback to auto-generated captions (`--write-subs --write-auto-subs --sub-langs "id.*,en.*"`).
   - Normalize and convert subtitle formats cleanly (`--convert-subs srt` or `ass`) and embed them into the `.mkv` container without breaking playback or timing sync.
3. **Audio Metadata & Lyrics Embedding**:
   - For audio extractions (`Opus`, `FLAC`, `M4A/AAC`, `MP3`), embed `1:1` cropped/original album art, ID3/Vorbis comments (Artist, Title, Album, Release Year, Track Number), and synced/unsynced lyrics (`USLT`/`SYLT` or Vorbis `LYRICS` tag) fetched from `lrclib`.

---

## 8. Rust Code Quality, Linting & Dependency Discipline

- Edition: `2024` (or `2021` minimum MSRV `1.85+`).
- Zero `unwrap()` or `expect()` in production code paths; enforce `#![deny(clippy::unwrap_used, clippy::expect_used, unsafe_code)]`.
- Structured error handling using `thiserror` for domain errors and `color-eyre` / `miette` for terminal panic/crash restoration hooks.
- Always restore terminal state (leave alternate screen, disable raw mode, purge Kitty graphics placements `\x1b_Ga=d\x1b\\`, and wipe `/run/user/$UID/mvd/*`) inside a custom panic & signal hook.
