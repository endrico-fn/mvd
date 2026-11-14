# `mvd` — Master Implementation Plan & Phased TODO Roadmap

> **Operational Policy**:
> 1. `[!] ZERO SYSTEM POLLUTION`: All config/cache/data strictly follow XDG Base Directory rules; output defaults to `~/Downloads/mvd/`; ephemeral cookies exist only in RAM (`/run/user/$UID/mvd/`).
> 2. `[!] OFFLOADED CI/CD COMPILATION`: Do NOT run `cargo build` or `cargo test` on the local laptop. All compilation, `clippy`, `rustfmt`, and test execution run on **GitHub Actions CI/CD**.

---

## Phase 0: Architecture, RFC, PRD & Security Recon (COMPLETED)
- [x] Author `AGENTS.md` with Rust Specialist, Utilitarian TUI, MKV Expert, `pbf03` Security, and CI/CD directives
- [x] Perform non-invasive host recon (`kitty 0.50.0`, `yt-dlp 2026.10.26`, `ffmpeg n9.0.2`, `sqlite3`, `Wayland/KDE`, `XDG_RUNTIME_DIR=/run/user/1000`)
- [x] Author `plan/PRD.md` with Cognitive/Architectural Bias Detection Matrix (`B1`–`B7`), Wireframes, Token System, and Test Fixture Matrix (`T-01`–`T-06`)
- [x] Author `plan/RFC.md` detailing Rust crate stack for Kitty, 4-Layer `pbf03` Cookie Isolation Vault, Smart Aspect-Ratio math, LRCLIB pipeline, Expert MKV + `EN`/`ID` Subtitles, and GitHub Actions CI/CD

---

## Phase 1: Project Scaffolding & Offloaded GitHub Actions CI/CD Pipeline
- [ ] **1.1 Workspace & Manifest Setup**:
  - Create `Cargo.toml` with strict lint rules (`#![deny(clippy::unwrap_used, clippy::expect_used, unsafe_code)]`)
  - Create `rustfmt.toml` and `clippy.toml`
  - Configure dependencies: `ratatui`, `crossterm`, `ratatui-image`, `image`, `tokio`, `reqwest`, `serde`, `serde_json`, `toml`, `url`, `rusqlite`, `zeroize`, `directories`, `arboard` (`wayland-data-control`), `thiserror`, `color-eyre`
- [ ] **1.2 GitHub Actions Workflow (`.github/workflows/ci.yml` & `release.yml`)**:
  - Job 1: `fmt-and-clippy` (`cargo fmt --check`, `cargo clippy -- -D warnings`)
  - Job 2: `unit-and-security-suite` (`cargo test --all-targets`)
  - Job 3: `release-artifact` (Compile stripped LTO Linux x86_64 binary on GitHub runners and publish downloadable artifact)

---

## Phase 2: XDG Base Directory, Config & Bounded Cache Engine (`src/config/` & `src/storage/`)
- [ ] **2.1 XDG Resolver (`src/config/xdg.rs`)**:
  - Resolve `$XDG_CONFIG_HOME/mvd/config.toml`
  - Resolve `$XDG_DATA_HOME/mvd/history.db`
  - Resolve `$XDG_STATE_HOME/mvd/queue.json`
  - Resolve `$XDG_CACHE_HOME/mvd/{thumbs,lyrics,meta}/`
  - Resolve `$XDG_RUNTIME_DIR/mvd/vault/` (verify `tmpfs` mount and enforce `0700` permissions)
  - Resolve safe output directory `~/Downloads/mvd/`
- [ ] **2.2 Bounded LRU + TTL Cache Manager (`src/storage/cache.rs`)**:
  - Implement 256 MiB quota enforcement + TTL eviction (`3h` manifests, `7d` thumbnails, `30d` LRCLIB JSON)
  - Implement manual cache purge command for Settings page
- [ ] **2.3 SQLite WAL History & Deduplication Ledger (`src/storage/ledger.rs`)**:
  - Create `downloads` schema (`id`, `extractor`, `media_id`, `normalized_url`, `title`, `container`, `file_path`, `file_size`, `downloaded_at`)
  - Implement instant `is_duplicate(extractor, media_id, normalized_url)` check

---

## Phase 3: Strict Firefox `pbf03` Sandbox Cookie Isolation (`src/security/`)
- [ ] **3.1 Firefox Profile Guard (`src/security/profile_guard.rs`)**:
  - Parse `~/.mozilla/firefox/profiles.ini` in read-only mode
  - Enforce hard allowlist: ONLY `Name=pbf03` is permitted
  - Enforce hard blocklist: Work Profile 1, Work Profile 2, and any default profile != `pbf03` return `SecurityError::ForbiddenProfile`
  - Verify canonical path resolution (`std::fs::canonicalize`) to prevent symlink escapes
- [ ] **3.2 Ephemeral RAM Cookie Vault (`src/security/cookie_vault.rs`)**:
  - Copy `pbf03/cookies.sqlite` + `.sqlite-wal` into `/run/user/$UID/mvd/vault/snap-<uuid>.db` (`0600`)
  - Query only cookies matching the target URL's registrable domain (`eTLD+1`) so cross-site sandbox cookies never leak
  - Emit scoped Netscape cookie jar into `/run/user/$UID/mvd/vault/jar-<uuid>.txt` (`0600`)
  - Implement `ZeroizeOnDrop` + automatic unlink on job completion, panic, or `SIGINT`/`SIGTERM`

---

## Phase 4: Multi-Site Extractor, LRCLIB Lyrics & Expert MKV Pipeline (`src/engine/`)
- [ ] **4.1 URL Parser & Tracking Stripper (`src/engine/parser.rs`)**:
  - Strip `?si=...`, `utm_*`, `feature=share` while preserving `v=`, `list=`, `drli=`
  - Detect platform category (`YouTubeVideo`, `YouTubePlaylist`, `YouTubeMusicTrack`, `YouTubeMusicAlbum`, `Instagram`, `XTwitter`, `Reddit`, `Facebook`, `DirectStream`)
  - Support batch `.txt` file ingestion and Wayland clipboard paste
- [ ] **4.2 Async `yt-dlp` Extractor & Raw Log Streamer (`src/engine/ytdlp.rs`)**:
  - Implement `--dump-single-json --flat-playlist` fast playlist loader
  - Implement per-item full stream/codec/subtitle metadata hydration
  - Stream unbuffered `stdout` + `stderr` lines to the TUI Raw Telemetry ring-buffer
- [ ] **4.3 LRCLIB Synced/Plain Lyrics Engine (`src/engine/lrclib.rs`)**:
  - Normalize audio track metadata (strip `- Topic`, `(Official Audio)`, etc.)
  - Query `https://lrclib.net/api/get` with fallback to `/api/search`
  - Parse `[mm:ss.xx]` synced LRC lines and cache in `$XDG_CACHE_HOME/mvd/lyrics/`
- [ ] **4.4 Subtitle (`EN`/`ID`) Optimizer & Expert `ffmpeg` MKV Muxer (`src/engine/subtitle.rs`, `src/engine/ffmpeg.rs`)**:
  - Prioritize manual `id` (`ind`) and `en` (`eng`) subtitles over auto-generated ASR captions
  - Build lossless `ffmpeg n9.0.2` Matroska (`.mkv`) muxing pipeline with ISO-639-2 language tags, default track disposition, chapter markers, cover art attachment, and SponsorBlock segment trimming
- [ ] **4.5 Concurrent Multi-Worker Queue (`src/engine/queue_worker.rs`)**:
  - Tokio semaphore pool (`1..8` parallel downloads), `--concurrent-fragments 4` / native HTTP Range chunks, pause/resume (`SIGSTOP`/`SIGCONT` or range resume), priority reordering, and bandwidth rate limiter

---

## Phase 5: Specialist Utilitarian TUI, Smart Aspect-Ratio Kitty Preview & TEA (`src/app/`, `src/ui/`)
- [ ] **5.1 The Elm Architecture Core (`src/app/model.rs`, `update.rs`, `message.rs`)**:
  - Pure `Model -> Message -> (Model, Command)` loop with **Dirty-Flag Rendering** (`model.dirty`)
  - Zero hardcoded mock data anywhere in UI components
- [ ] **5.2 Utilitarian Theme & Zero-Emoji Component Library (`src/ui/theme.rs`, `components/`)**:
  - Apply `#4A4D52` dim grey border and `#28b877` primary emerald accent
  - Enforce 100% English copy and strict symbol-only badges (`[+]`, `[-]`, `[*]`, `[!]`, `[x]`, `[i]`)
  - Build centered tokens, Topbar, Footer, Input/Search bar, Styled Scrollbar, Skeleton Loading widget, and Centered Floating Modals
- [ ] **5.3 Smart Aspect-Ratio Kitty Thumbnail & Bottom LRCLIB Lyrics Pane (`src/ui/aspect.rs`, `thumbnail.rs`, `lyrics.rs`)**:
  - Query Kitty `TIOCGWINSZ` pixel geometry and fit `16:9`, `1:1`, `9:16`, and `4:5` thumbnails without distortion via `ratatui-image` Kitty protocol
  - Render scrollable LRCLIB Lyrics sub-panel directly beneath the `1:1` thumbnail whenever source is audio/YouTube Music
- [ ] **5.4 Five Dedicated TUI Pages (`src/ui/pages/`)**:
  - Page `[1] EXTRACTOR & PREVIEW`
  - Page `[2] QUEUE MANAGER` (Dedicated separate queue page)
  - Page `[3] RAW TELEMETRY LOGS` (Live `yt-dlp` + `ffmpeg` raw output & structured Error/Warn/Info log)
  - Page `[4] HISTORY LEDGER` (SQLite deduplication archive)
  - Page `[5] SETTINGS & SECURITY` (XDG paths, `pbf03` lock status, EN/ID subtitle policy, cache purge)

---

## Phase 6: Verification Against Mandatory Test Links (Live-Tested & CI Automated)
- [x] **T-01 (Video Playlist)**: `https://youtube.com/playlist?list=PLIhvC56v63IKioClkSNDjW7iz-6TFvLwS&si=Me_WJsw1pF2svME5` (Verified 5 items, `16:9` thumbs)
- [x] **T-02 (Video Single)**: `https://youtu.be/SPwyp2NG-bE?si=0d_ISKZOBA72UU3C` (Verified 115 formats up to 4K AV1, 14 chapters, Manual `en` + Auto `id` subtitles)
- [x] **T-03 (Music Album Artist)**: `https://music.youtube.com/playlist?list=OLAK5uy_kXEi_1ry11kHAe71Z36AShGLhVXejbK38&si=s72KLYDSaQ7vXc-d&drli=1` (Verified 14 album tracks, `1200x1200` `1:1` album art + mixed VEVO `16:9` track detection)
- [x] **T-04 (Music Single 1 - Age-Gated VEVO)**: `https://music.youtube.com/watch?v=LrSX_OcpeJg&si=3_UGDgDjz88J6h6J&drli=1` (Verified age-gate triggers `pbf03` cookie vault requirement; native `16:9` VEVO video)
- [x] **T-05 (Music Single 2 - Art Track)**: `https://music.youtube.com/watch?v=5xx9gTEQaiQ&si=k20r8T991nBzeW0t&drli=1` (Verified `1:1` `544x544` `yt3.googleusercontent.com` cover selection over `hq720.jpg` + LRCLIB synced lyrics `#6977868`)
- [x] **T-06 (Music Community Playlist)**: `https://music.youtube.com/playlist?list=PLJNrcbVaS76JVPdSmjIs2Iep9C7XBUh74&si=YKqZuWdEJlT6XagI&drli=1` (Verified 43 tracks mixing `- Topic` `1:1` Art Tracks and `16:9` artist uploads)
- [x] **T-07 (Spotify Track Bridge)**: `https://open.spotify.com/track/7MtVPRGtZl6rPjMfLoI3Lh?si=c0ea33c09cfc4bd9` (Verified Spotify Embed JSON `640x640` `1:1` cover + LRCLIB synced lyrics `#30437529` + duration-scored YT Music audio match `wDWE_Qp3oKM` with `0.7s` delta)
