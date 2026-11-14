# Product Requirements Document (PRD): `mvd` — Multi-Platform Video & Media Downloader TUI

| Metadata | Value |
| :--- | :--- |
| **Document ID** | `PRD-MVD-2026-001` |
| **Product Name** | `mvd` (Multi-Platform Video & Media Downloader) |
| **Target Environment** | Linux (`x86_64-unknown-linux-gnu`), Wayland/X11, Kitty Terminal (`v0.50.0+`) |
| **Core Engines** | Rust (TEA Architecture), `yt-dlp` (`2026.10.26+`), `ffmpeg` / `ffprobe` (`n9.0.2+`), `SQLite3`, `LRCLIB API` |
| **Default Language** | 100% English (`en-US` system UI) |
| **Design Paradigm** | Premium Utilitarianism — Zero Emoji (Symbol-Only), Centered Tokens, Accent `#28b877`, Border `#4A4D52` |
| **Build & Test Policy** | 100% Offloaded to GitHub Actions CI/CD (Zero Local Host CPU/Disk Compilation Load) |
| **Status** | `DRAFT — READY FOR REVIEW` |

---

## 1. Executive Summary & Vision

`mvd` is a high-performance, security-hardened Terminal User Interface (TUI) media acquisition and post-processing suite written in Rust. It bridges the extraction breadth of `yt-dlp` (supporting 1,800+ extractors including YouTube, YouTube Music, Instagram, X/Twitter, Reddit, Facebook, TikTok, SoundCloud, Vimeo, and direct HLS/DASH/HTTP streams) with an ergonomic, keyboard-driven Vim-modal interface tailored specifically for the **Kitty terminal**.

Unlike conventional CLI wrappers or bloated Electron apps, `mvd` delivers:
1. **True Pixel-Accurate Visuals**: Native Kitty Graphics Protocol rendering with smart platform-aware aspect-ratio fitting (`16:9`, `1:1`, `9:16`, `4:5`, `4:3`) and live synchronized lyrics preview (`lrclib.net`) positioned directly beneath audio album art.
2. **Cryptographic-Grade Browser Profile Isolation**: Hardened cookie extraction locked exclusively to the designated Firefox sandbox profile (`pbf03`), physically blocking access to the user's two work profiles and operating strictly in RAM (`tmpfs` `/run/user/$UID/mvd`) with automatic memory zeroization on exit.
3. **Archival-Grade Matroska (`.mkv`) & Subtitle Pipeline**: Native `ffmpeg` stream remuxing, prioritized dual-language subtitle acquisition (`EN` & `ID`), chapter preservation, SponsorBlock segment trimming, and metadata/cover-art attachment without polluting the host system with extra external dependencies.

---

## 2. Cognitive & Architectural Bias Detection Matrix

Before defining requirements, we explicitly audit and eliminate common engineering biases that plague media downloader projects:

| # | Detected Bias Category | Flawed Assumption (The Trap) | Reality on Target System (`porvata` Host Recon) | Mitigation Strategy in `mvd` |
| :--- | :--- | :--- | :--- | :--- |
| **B1** | **Host Dependency Bias** | Assuming `aria2c`, `mkvmerge`, and `mkvpropedit` are pre-installed on every Linux system. | Host recon proved `yt-dlp` (`2026.10.26`), `ffmpeg` (`n9.0.2`), `ffprobe`, and `kitty` (`0.50.0`) are installed, while **`aria2c` and `mkvtoolnix` (`mkvmerge`/`mkvpropedit`) are absent**. Installing them violates `[!] tidak mengotori system`. | Perform 100% of Matroska `.mkv` muxing, track disposition, ISO-639-2 language tagging (`eng`/`ind`), and cover attachment using **`ffmpeg n9.0.2`** natively, and implement high-speed concurrent chunking via native Rust `reqwest` HTTP Range workers + `yt-dlp --concurrent-fragments`. Keep `aria2c`/`mkvmerge` strictly as optional runtime capability probes. |
| **B2** | **Aspect-Ratio Uniformity Bias** | Assuming all thumbnails are `16:9` YouTube landscapes, causing stretched or clipped images for YouTube Music (`1:1`), Instagram Reels/Shorts (`9:16`), or X/Reddit portrait posts (`4:5`). | Multi-platform extractors return radically different aspect ratios (`1:1` album art on `music.youtube.com`, `9:16` vertical video, `16:9` standard video). | Implement a **Smart Aspect-Ratio Layout Calculator** that inspects decoded image dimensions (`W x H`) + terminal cell pixel geometry (`TIOCGWINSZ`) and dynamically centers/scales the viewport within the preview pane without distortion. |
| **B3** | **Cookie Convenience vs. Security Bias** | Passing `--cookies-from-browser firefox` globally or copying `cookies.sqlite` to `/tmp` or `~/.cache`, risking work-account leakage or SSD forensic persistence. | User has **3 Firefox profiles**: 2 Work profiles (high-sensitivity identity) and 1 Sandbox profile (`pbf03`, Clear-On-Exit, anonymous browsing). | Enforce a **Strict Profile Allowlist (`pbf03` only)** verified against `~/.mozilla/firefox/profiles.ini`. Reject any other profile name/path at the Rust type-system level (`ValidatedSandboxProfile`). Stage any decrypted cookie jar exclusively in `/run/user/$UID/mvd/` (`tmpfs` RAM) with `0600` permissions and `zeroize` on drop. |
| **B4** | **TUI Redraw / Polling Bias** | Rendering the TUI at a fixed 60 FPS loop regardless of state changes, draining laptop battery and spiking CPU. | TUI views are predominantly static between keyboard presses and worker progress ticks. | Enforce **Dirty-Flag Rendering** inside The Elm Architecture (`model.dirty`). Render frames to Kitty only when state mutates or `SIGWINCH` (resize) fires. Throttle download progress UI updates to at most `10 Hz` (every `100ms`). |
| **B5** | **Wayland Clipboard Blindness** | Assuming X11 clipboard (`xclip`/`xsel`) works everywhere on Linux. | Host recon confirmed `XDG_SESSION_TYPE=wayland` (`KDE Plasma`). | Configure `arboard` with `wayland-data-control` feature enabled so clipboard URL parsing works natively under Wayland without XWayland leakage. |
| **B6** | **Hardcoded Mock UI Bias** | Leaving placeholder strings, fake queue rows, or emoji decorations inside TUI source code. | Degrades trust, obscures empty/loading states, and breaks utilitarian readability. | **Zero Hardcoded Data Rule** & **Zero Emoji Rule**: Empty states use dedicated utilitarian placeholders; active fetches render deterministic skeleton loaders; only geometric ASCII/Unicode symbols are permitted. |
| **B7** | **Local Compilation Resource Bias** | Running heavy Rust release builds and LTO link passes on the user's laptop. | Rust + `ratatui` + `image` + `sqlx`/`rusqlite` compilation consumes high CPU/RAM and gigabytes of `target/` artifacts. | **100% GitHub Actions CI/CD Offloading**: All `cargo fmt`, `cargo clippy`, `cargo nextest`, and release binary builds execute on GitHub-hosted Ubuntu runners with `sccache` / `rust-cache`. |
| **B8** | **YouTube Music Thumbnail Preference Bias** | Assuming `yt-dlp`'s last thumbnail (`thumbnails[-1]`) or every `music.youtube.com` link is `1:1`. | Live test on `T-03`, `T-04`, `T-05`, `T-06` proved two opposite traps: (1) For `1:1` Art Tracks (`5xx9gTEQaiQ`), `yt-dlp` gives `yt3.googleusercontent.com` (`544x544`) a low `preference: -39` and puts a pillarboxed `1280x720` (`hq720.jpg`, `preference: -3`) last; (2) Many music tracks on YT Music are **VEVO / Artist Music Videos** (`16:9` `1920x1080`) or community uploads (`WHVupRkTdqo`) with no `1:1` cover. | **Native As-Is Aspect Policy**: Inspect video stream dimensions (`1080x1080` vs `1920x1080`) and `thumbnails[]` URLs. If it is a VEVO/Artist Music Video (`16:9`), render the native `16:9` thumbnail **as-is** (never force-crop to `1:1`). If it is a `1:1` Art Track (`1080x1080`), prioritize the un-pillarboxed `yt3.googleusercontent.com` `1:1` artwork instead of `hq720.jpg`. |
| **B9** | **Spotify DRM Direct-Download Bias** | Passing `open.spotify.com/track/...` directly to `yt-dlp` and failing with `ERROR: [DRM]`. | Live test on `T-07` (`7MtVPRGtZl6rPjMfLoI3Lh`) confirmed `yt-dlp` aborts on Spotify DRM, AND a naive `ytsearch1` matched the `560s` age-gated double music video (`LrSX_OcpeJg`) instead of the `218s` single track! | **Spotify Metadata + Duration-Scored YT Music Bridge**: Extract official title, artist, release date, `duration_ms` (`218287ms`), and `640x640` `1:1` cover art from Spotify's public `oembed`/`embed` JSON (zero API keys needed), fetch synced lyrics from `lrclib.net`, and score `ytsearch5` audio candidates by `|yt_dur - sp_dur|` (`0.7s` delta match). |

---

## 3. Functional Requirements (FR)

### FR-01: Multi-Site URL Parser & Extractor
- **Single, Playlist & Batch Ingestion**:
  - Accept single video URLs, playlist URLs (`youtube.com/playlist`, `music.youtube.com/playlist`), artist albums (`OLAK5uy_...`), community playlists, clipboard paste (`Ctrl+V` or `p` in normal mode), and batch `.txt` files (ignoring `#` comment lines and blank lines).
- **URL Sanitization & Normalization**:
  - Automatically strip tracking parameters (`?si=...`, `&feature=share`, `utm_*`) before hashing URLs for deduplication and cache keys, while preserving functional query parameters (`v=`, `list=`, `drli=`, timestamp `t=` when requested).
- **Extraction Pipeline**:
  - Execute `yt-dlp --dump-single-json --flat-playlist` for instant playlist enumeration (<1.5s first paint), followed by background per-item metadata hydration.
  - Provide direct HTTP/HTTPS and HLS (`.m3u8`) / DASH (`.mpd`) fallback extraction when a direct media manifest URL is supplied.

### FR-02: Kitty Native Thumbnail Preview & Smart Aspect-Ratio Engine
- **Kitty Graphics Protocol (`\x1b_G...`)**:
  - Render crisp, full-color raster thumbnails directly inside the TUI Preview Pane using `ratatui-image` configured for the Kitty Graphics Protocol.
  - Never fall back to blurry half-block/ASCII art when running inside Kitty (`kitty 0.50.0`).
- **Smart Platform Aspect-Ratio Adaptation**:
  - Automatically detect platform & image dimensions and calculate exact cell bounding boxes using Kitty's pixel-per-cell ioctl (`TIOCGWINSZ`):
    - `16:9` Landscape: YouTube standard videos, X/Twitter landscape videos, Vimeo, Twitch clips.
    - `1:1` Square: `music.youtube.com` Album/Track Art, Instagram square posts, SoundCloud/Bandcamp artwork.
    - `9:16` Vertical: Instagram Reels, YouTube Shorts, TikTok, Facebook Stories.
    - `4:5` / `4:3` Portrait/Classic: Instagram portrait posts, Reddit media cards, archival broadcasts.
  - Center the image horizontally and vertically within the allocated thumbnail frame with clean border framing and zero aspect distortion.

### FR-03: LRCLIB Audio Lyrics Preview (Below Thumbnail)
- **Automatic Audio-Source Trigger**:
  - When the inspected media item originates from an audio platform (`music.youtube.com`, SoundCloud, Bandcamp) or when the user selects **Audio-Only Mode**, the Left Preview Column splits vertically into two sub-panels:
    1. **Top Sub-Panel (`55%` height)**: `1:1` Square Album/Track Thumbnail via Kitty Graphics Protocol.
    2. **Bottom Sub-Panel (`45%` height)**: **LRCLIB Lyrics Preview Pane**.
- **LRCLIB Integration & Query Normalization**:
  - Clean raw video/track titles (strip suffixes such as `(Official Music Video)`, `[Official Audio]`, `- Topic` from channel names) to extract clean `(artist_name, track_name, album_name, duration_secs)`.
  - Query `https://lrclib.net/api/get` with automatic fallback to `https://lrclib.net/api/search`.
  - Display status badge: `[SYNCED LRC]`, `[PLAIN TEXT]`, `[INSTRUMENTAL]`, or `[NO LYRICS FOUND]`.
  - Support keyboard scrolling (`J`/`K` or `Ctrl+d`/`Ctrl+u`) through the lyrics preview and optional automatic embedding of `.lrc` / metadata tags into the final audio file.

### FR-04: Granular Stream, Codec, Container & Subtitle (`EN`/`ID`) Selector
- **Interactive Stream Matrix**:
  - Video Resolutions: `4320p (8K)`, `2160p (4K)`, `1440p (2K)`, `1080p`, `720p`, `480p`, `360p`, `Worst/Data-Saver`.
  - Video Codecs: `AV1 (av01)`, `VP9 (vp9/vp9.2 HDR)`, `H.264 (avc1)`, `HEVC (hev1)`.
  - Audio Streams: `Opus (~160kbps WebM)`, `AAC (m4a)`, and post-extraction lossless/transcode targets (`FLAC`, `Opus`, `MP3 320k`, `WAV`).
  - Containers: **Expert MKV (`.mkv` Default for Video)**, `.mp4` (compatibility), `.webm`, `.flac` / `.opus` / `.m4a` (audio-only).
- **Subtitle Optimization (`EN` & `ID` First-Class Support)**:
  - Dedicated quick-toggle profiles:
    - `[SUB: EN+ID MANUAL+AUTO]` (Default recommended)
    - `[SUB: ID ONLY]`
    - `[SUB: EN ONLY]`
    - `[SUB: ALL AVAILABLE]`
    - `[SUB: OFF]`
  - Selection Priority Algorithm:
    1. Human-curated Indonesian (`id`, `id-ID`) & English (`en`, `en-US`, `en-GB`).
    2. Auto-translated / ASR captions (`id-orig`, `en-orig`, `id`, `en`) only if manual tracks are missing and user policy allows auto-subs.
  - Format Normalization: Convert WebVTT (`vtt`) / TTML (`srv3`) cleanly into standard `.srt` or `.ass` before embedding into `.mkv` with proper ISO-639-2 language codes (`ind`, `eng`) and track titles (`Indonesian`, `English`).

### FR-05: Dedicated Queue Manager Page & High-Speed Concurrent Engine
- **Dedicated Separate Queue Page (`Page [2] QUEUE MANAGER`)**:
  - Displays Active Downloads, Queued Pending Jobs, Paused Jobs, and Completed/Failed Jobs in a full-screen utilitarian table with real-time per-worker telemetry.
- **Concurrency & Chunking**:
  - Configurable parallel job workers (`1` to `8` concurrent media items, default `3`).
  - Per-job multi-fragment acceleration (`yt-dlp --concurrent-fragments 4` for DASH/HLS segments, and native Rust `reqwest` multi-range HTTP chunk downloader for direct streams).
  - Live controls: Pause (`p`), Resume (`p`), Cancel/Delete (`d`), Move Up/Down Priority (`K`/`J` or `+`/`-`), Retry Failed (`r`), Global Rate Limiter (`L` -> e.g. `5M`, `10M`, `UNLIMITED`).

### FR-06: Strict Firefox `pbf03` Sandbox Cookie Isolation
- **Hard Profile Lock**:
  - Parse `~/.mozilla/firefox/profiles.ini` in read-only mode.
  - Only permit profile `pbf03` (the user's anonymous Clear-On-Exit sandbox profile).
  - **Hard-Block Work Profiles**: Any attempt (via CLI flag, config edit, or environment variable) to load Profile 1 or Profile 2 (Work profiles) triggers a fatal `SecurityViolation::ForbiddenProfile` error before any file descriptor is opened.
- **Zero-Persistence Ephemeral Cookie Handling**:
  - Read `pbf03/cookies.sqlite` via immutable read-only SQLite URI (`?mode=ro&immutable=1`) so Firefox is never locked and `mvd` never writes to the browser profile.
  - Domain-Scoped Filtering: When downloading from a specific domain (e.g., `youtube.com`), extract **only** cookies matching that domain suffix into an ephemeral RAM-backed file in `/run/user/$UID/mvd/cookie-<uuid>.txt` (`0600` permissions) or memory buffer. Cross-site sandbox cookies (e.g. from other research tabs inside `pbf03`) are never exposed to unrelated extractors.
  - Automatic Cleanup: Overwrite with zeros (`zeroize`) and unlink immediately when the worker exits or when `mvd` terminates.

### FR-07: Expert MKV & Post-Processing Pipeline (`ffmpeg`)
- **Matroska (`.mkv`) Mastering**:
  - Remux video + audio + `EN`/`ID` subtitles + chapters + thumbnail attachment (`cover.jpg` / `cover.png`) using `ffmpeg n9.0.2`.
  - Set Matroska metadata tags (`TITLE`, `ARTIST`, `DATE_RELEASED`, `COMMENT`, `PURL` source URL) and stream disposition flags.
- **SponsorBlock Integration**:
  - Optional removal or chapter-marking of segments (`sponsor`, `selfpromo`, `intro`, `outro`, `interaction`, `music_offtopic` for music tracks).

### FR-08: Direct Search & SQLite History Ledger
- **Direct In-TUI Search**:
  - Search YouTube (`ytsearch15:...`), YouTube Music, or SoundCloud (`scsearch10:...`) directly from the TUI search bar without opening a web browser.
- **SQLite History & Deduplication Ledger (`$XDG_DATA_HOME/mvd/history.db`)**:
  - Automatically log every completed download (canonical extractor ID, normalized URL hash, title, duration, chosen format/codec, file path in `~/Downloads/mvd/`, SHA-256 of output header or file size, timestamp).
  - Warn immediately with `[!] DUPLICATE DETECTED IN LEDGER` if a URL/ID in a playlist or single input has already been downloaded and still exists in `~/Downloads/mvd/`.

### FR-09: Raw Subprocess Output & Structured Telemetry Page
- **Live Raw Output Inspector (`Page [3] RAW TELEMETRY LOGS`)**:
  - Stream real-time unbuffered `stdout` and `stderr` from `yt-dlp` and `ffmpeg` into a bounded ring-buffer (`10,000` lines max in RAM).
  - Split-pane view: Top pane shows structured `INFO` / `WARN` / `ERROR` application events; Bottom pane shows raw `yt-dlp` + `ffmpeg` console output with regex filtering (`/`).

---

## 4. Specialist TUI Layout & Premium Utilitarian Design System

### 4.1 Design Tokens & Visual Rules

| Token Name | Value / Specification | Usage |
| :--- | :--- | :--- |
| `color.border.dim` | `#4A4D52` (`Rgb(74, 77, 82)`) | Default unfocused panel borders, dividers, inactive tabs, scrollbar track |
| `color.accent.primary` | `#28b877` (`Rgb(40, 184, 119)`) | Active panel border, focused token badge, progress bar fill, active tab, selected row indicator |
| `color.text.primary` | `#ECEFF4` (`Rgb(236, 239, 244)`) | Primary titles, active input text, selected list items |
| `color.text.secondary` | `#888E99` (`Rgb(136, 142, 153)`) | Metadata labels, inactive tabs, secondary columns |
| `color.text.placeholder` | `#565B66` (`Rgb(86, 91, 102)`) | Empty state hints, input placeholders, skeleton shimmer base |
| `color.status.info` | `#3B82F6` (`[i]`) | Informational notifications, metadata hydration notices |
| `color.status.warn` | `#F59E0B` (`[!]`) | Rate limit backoff, missing manual sub fallback to auto-sub, duplicate warning |
| `color.status.error` | `#EF4444` (`[x]`) | Security violation, network failure, subprocess non-zero exit |
| `symbol.policy` | **ZERO EMOJI (JetBrains Mono Symbols Only)** | Allowed: `[+]`, `[-]`, `[*]`, `[!]`, `[x]`, `[i]`, `▸`, `▾`, `▪`, `▫`, `●`, `○`, `◆`, `◇`, `✕`, `✓`, `≡`, `⋮`, `│`, `─`, `┌`, `┐`, `└`, `┘`, `█`, `▓`, `▒`, `░` |
| `border.style` | **Thin Sharp 90° (`BorderType::Plain`)** | Strict `┌ ─ ┐ │ └ ─ ┘` sharp corners across all panels and float modals (never rounded) |
| `layout.gap` | `0` outer margin, `1` cell inter-pane horizontal spacing | Crisp utilitarian grid alignment with zero wasted vertical lines |
| `token.title.alignment` | **Strictly Centered (`Alignment::Center`)** with `─[ TOKEN ]─` | e.g., `─[ PREVIEW : 16:9 ]─`, `─[ LYRICS : LRCLIB ]─`, `─[ METADATA EDITOR ]─` |
| `footer.key.policy` | **Contextual Action Keys Only** | Omit universal navigation/dismiss keys (`j/k`, `Up/Down`, `Esc`) from footer; display only active context actions |

### 4.1.1 Conditional Layout Matrix (Preview Always on Left)

| Media Archetype | Left Column (`38%–45%` Width) | Right Column (`55%–62%` Width) | Metadata & Override Capabilities |
| :--- | :--- | :--- | :--- |
| **1. Single Audio** (`YT Music`, `Spotify`, `SoundCloud`) | **Top (`55%`)**: Kitty Thumbnail (`1:1` Cover or `16:9` VEVO as-is)<br>**Bottom (`45%`)**: Scrollable `LRCLIB` Synced/Plain Lyrics | Editable Audio Metadata Summary + Audio Codec/Bitrate Table (`Opus`, `FLAC`, `M4A`, `MP3`) + Post-Process Toggles | `m`: Edit `Title`, `Artist`, `Album`, `Track #`, `Year`, `Genre`, `LRC Embed` |
| **2. Audio Playlist / Album** (`T-03`, `T-06`, Spotify Album) | **Top (`55%`)**: Highlighted Track's `1:1` Cover (or `16:9` VEVO as-is)<br>**Bottom (`45%`)**: Highlighted Track's live `LRCLIB` Lyrics | **Top Bar**: Global Album Policy (`OPUS 160k / FLAC`, Subfolder)<br>**Main Body**: Tracklist Table with per-track `[GLOBAL]` vs `[* OVERRIDE]` badges | `f` on row: Override track codec/bitrate<br>`m` on row: Edit individual track `Title`, `Artist`, `Track #`<br>`M`: Edit Album subfolder name |
| **3. Single Video** (`T-02` YouTube, `X`, `Facebook`, `Reddit`) | **Top (`60%`)**: Native Ratio Kitty Thumbnail (`16:9`, `9:16`, `4:5`)<br>**Bottom (`40%`)**: Video Chapters & `EN`/`ID` Subtitle Source Inspector (`Manual` vs `Auto-ASR`) | Editable Video Metadata Summary + Granular Video/Audio Stream Table + Container (`MKV`/`MP4`) & Subtitle (`EN`/`ID`) Options | `m`: Edit `Title` (and safe filename), `Creator/Artist`, `Year`, `Comment/Description` |
| **4. Video Playlist** (`T-01` YouTube Series) | **Top (`60%`)**: Highlighted Episode's `16:9` Kitty Thumbnail<br>**Bottom (`40%`)**: Highlighted Episode's Chapters & `EN`/`ID` Subtitle Status (`en: Manual`, `id: Auto`) | **Top Bar**: Global Playlist Ceiling (`Max: 1080p`, `MKV`, `Sub: EN+ID`)<br>**Main Body**: Episode Checklist Table (`[x]`, `#`, `Title`, `MaxRes`, `Subs`, `Status`) | `f` on row: Override episode resolution/codec/subtitles (since episodes vary in max res & available subs)<br>`m` on row: Edit episode `Title` & metadata |
| **5. Static Image / Gallery** (`Instagram`, `Pinterest`, `X`, `Reddit`) | **Full Height (`100%`)**: Native Aspect-Ratio Kitty Preview (`1:1`, `4:5`, `2:3` Pinterest) + Carousel Counter `[SLIDE 2/5]` (`[`/`]` to cycle) | Image Stream Inspector (`Original PNG/JPG/WebP`, `W x H`, Size) + Slide Checklist (for Carousels) + EXIF/Filename Editor | `m`: Edit output filename, `Title`, `Author/Source` metadata |
| **6. Animated GIF / Loop** (`Reddit GIF`, `X Loop`, `Pinterest GIF`) | **Full Height (`100%`)**: Native Ratio Frame Preview + Loop Telemetry (`FPS`, `Duration`, `Silent Loop`) | Target Format Selector (`Lossless .mp4`, `.webm`, `Animated .gif`) + Filename Editor | `m`: Edit output filename & title tag |

### 4.1.2 Conditional Keybindings, TUI-to-CLI Handoff & Crash/Kill Resume
1. **`Q` (Uppercase) & `Ctrl+C` (Inside TUI) — Non-Destructive Detach to CLI**:
   - Opens centered modal `─[ EXIT TUI & HANDOFF TO CLI ]─`.
   - If downloads are running, confirming exits the TUI alternate screen **without interrupting any active download worker** and switches to **Live CLI Progress Output** in the standard terminal:
     ```text
     [MVD CLI HANDOFF] 2 active downloads running (Press Ctrl+C to abort & save resume state)
     [*] #01 [74.2%] 11.4 MiB/s ETA 00:11 | Linux_Kernel_Deep_Dive [SPwyp2NG-bE].mkv
     [*] #02 [48.0%]  6.1 MiB/s ETA 00:04 | Lana Del Rey - Love song [5xx9gTEQaiQ].opus
     ```
2. **`Ctrl+C` (Inside CLI Progress Mode) — Clean Process Kill + Persistent Resume State**:
   - Immediately terminates active `yt-dlp` / `ffmpeg` child processes (`SIGTERM` -> `SIGKILL`), wipes `/run/user/$UID/mvd/vault/` RAM cookies, and flushes `$XDG_STATE_HOME/mvd/queue.json` while keeping `~/Downloads/mvd/.staging/<job_id>/*.part` intact.
   - **Next Time User Opens TUI (`mvd`)**:
     - Reads `$XDG_STATE_HOME/mvd/queue.json`.
     - For **Single Videos/Tracks**: Resumes from the exact byte offset / DASH fragment via `yt-dlp --continue` (`-c`) or offers `R: Restart From Scratch`.
     - For **Playlists / Albums**: Completed tracks (verified in SQLite History Ledger) remain marked `[+] DONE` and are skipped; the interrupted track resumes at its `.part` offset; remaining `QUEUED` tracks continue in order.
3. **`q` (Lowercase) vs `Esc` vs `c`**:
   - `q`: Contextual step-back (returns from sub-inspector or resets active pane focus to default). Never exits the app.
   - `Esc`: Closes any floating modal/dialog or exits `INSERT`/`FILTER` mode back to `NORMAL` mode.
   - `c`: Cancels the highlighted download job (only inside `Page [2] QUEUE MANAGER`).
4. **New URL Ingestion UX (`p`, `n`, `D`)**:
   - Pressing `p` (Paste Clipboard URL) or `n` (New URL Input) in `NORMAL` mode automatically replaces the input bar and starts previewing the new link immediately.
   - **Conditional Unsaved-Edits Guard Modal**: Only appears if the user modified metadata (`m`) or per-item overrides (`*`) on the current preview without queueing (`Enter`) yet.
   - Pressing `D` (`Quick Enqueue Modal`) allows pasting/queueing a new link with a 1-click preset popup without replacing the currently inspected preview!
5. **Multi-Platform & Privacy Support (`X`, `Reddit`, `Facebook`, `Instagram`, `Pinterest`, `Adult/NSFW`)**:
   - **Pinterest / Instagram / X Static Images & GIFs**: Includes a native OpenGraph / CDN media extractor fallback (`pinimg.com/originals`, `pbs.twimg.com?name=orig`) when a link points to a static image or gallery rather than a video stream.
   - **Ephemeral Incognito Ledger Mode (`I` Key)**: Designed for adult sites / sensitive sandbox browsing. When `[PRIVACY: EPHEMERAL]` is active, `mvd` uses `pbf03` RAM cookies, writes **zero records** to `history.db`, and caches **zero thumbnails** on disk (renders strictly from RAM and zeroizes on exit).
6. **Non-Destructive History & Log Clearing (`x` / `X`)**:
   - Clearing logs (`Page [3]`) or purging history (`Page [4]`) strictly deletes in-memory log lines or SQLite rows in `history.db` — **completed files in `~/Downloads/mvd/` are never deleted**.

### 4.2 Wireframe — Page `[1] EXTRACTOR & PREVIEW` (Audio / YouTube Music Mode)

```text
┌──────────────[ MVD v0.1.0 | 1:EXTRACTOR*  2:QUEUE(3)  3:LOGS  4:HISTORY  5:SETTINGS | COOKIE: pbf03 ]──────────────┐
│ ┌─────────────────────────────────────────────[ URL / SEARCH INPUT ]─────────────────────────────────────────────┐ │
│ │ [NORMAL] ▸ https://music.youtube.com/watch?v=5xx9gTEQaiQ                              [p:Paste  n:New  D:Quick]│ │
│ └────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘ │
│ ┌────────[ THUMBNAIL : 1:1 ART ]───────┐ ┌──────────────────[ AUDIO METADATA & STREAM INSPECTOR ]────────────────┐ │
│ │                                      │ │ TITLE    : Love song                                  [m: Edit Meta]  │ │
│ │      ┌────────────────────────┐      │ │ ARTIST   : Lana Del Rey               ALBUM    : Norman F. Rockwell!  │ │
│ │      │                        │      │ │ SOURCE   : youtube:music [5xx9gTEQaiQ] DURATION : 03:49               │ │
│ │      │    [KITTY GRAPHICS]    │      │ │ LEDGER   : [+] NOT IN HISTORY         PRIVACY  : [STANDARD LEDGER]    │ │
│ │      │     1:1 PIXEL-EXACT    │      │ ├───────────────────[ GRANULAR AUDIO STREAM SELECTOR ]──────────────────┤ │
│ │      │      ALBUM ARTWORK     │      │ │   ID    EXT   BITRATE   CODEC        SAMPLE   SIZE      STATUS        │ │
│ │      │                        │      │ │ ▸ 251   opus  160kbps   opus         48kHz    4.21 MiB  [SELECTED] ▲  │ │
│ │      └────────────────────────┘      │ │   140   m4a   128kbps   mp4a.40.2    44.1kHz  3.40 MiB  [AVAILABLE]█  │ │
│ │                                      │ │   flac  flac  Lossless  flac (remux) 48kHz    ~24 MiB   [AVAILABLE]░  │ │
│ ├──────[ LYRICS : LRCLIB SYNCED ]──────┤ ├───────────────────[ POST-PROCESSING & TAG OPTIONS ]───────────────────┤ │
│ │ [00:29.26] In the car, in the car... ▲ │ [x] Embed 1:1 Cover Art      [x] Embed LRCLIB Synced Lyrics (lofty)   │ │
│ │ [00:35.70] We go fast, we go so fast █ │ [x] SponsorBlock: Trim Non-Music Segments                             │ │
│ │ [00:42.87] I believe in a place you  ░ │ Container: [ OPUS (Default) | FLAC | M4A | MKV ]                      │ │
│ │ [00:46.33] Make you feel like...     ▼ │                                                                       │ │
│ └──────────────────────────────────────┘ └───────────────────────────────────────────────────────────────────────┘ │
├─[ Enter:Queue  Space:Toggle  f:Formats  m:EditMeta  p:PasteURL  D:QuickDl  I:Privacy  Q:DetachCLI ]──[ [+] READY ]─┤
└────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### 4.2.1 Wireframe — Page `[1] EXTRACTOR & PREVIEW` (Playlist / Album Mode with Per-Item Override `[*]`)

```text
┌──────────────[ MVD v0.1.0 | 1:EXTRACTOR*  2:QUEUE(3)  3:LOGS  4:HISTORY  5:SETTINGS | COOKIE: pbf03 ]──────────────┐
│ ┌────────[ THUMBNAIL : 16:9 VEVO ]─────┐ ┌────────────[ PLAYLISTPOLICY : MAX 1080p | MKV | SUB: EN+ID ]──────────┐ │
│ │  ┌────────────────────────────────┐  │ │ FOLDER : ~/Downloads/mvd/Norman Fucking Rockwell!/     [M: Edit Dir]  │ │
│ │  │     [KITTY GRAPHICS 16:9]      │  │ ├────────────[ PLAYLIST ITEMS : 14 TRACKS (14 SELECTED) ]───────────────┤ │
│ │  │   Mariners Apartment Complex   │  │ │   SEL  #   TITLE                        DUR    RATIO  SUBS    POLICY  │ │
│ │  │      (Native VEVO Ratio)       │  │ │   [x]  01  Norman fucking Rockwell      04:09  1:1    ---     [GLOBAL]│ │
│ │  └────────────────────────────────┘  │ │ ▸ [x]  02  Mariners Apartment Complex   04:08  16:9   EN,ID   [* OVER]│ │
│ ├──────[ LYRICS : LRCLIB SYNCED ]──────┤ │   [x]  03  Venice Bitch                 09:38  1:1    ---     [GLOBAL]│ │
│ │ [00:13.20] You took my sadness...    ▲ │   [x]  04  Fuck it I love you           03:38  1:1    EN      [GLOBAL]│ │
│ │ [00:19.45] Out of context...         █ │   [x]  05  Doin' Time                   03:22  16:9   EN,ID   [GLOBAL]│ │
│ │ [00:25.10] At the Mariners Apt...    ▼ │   [x]  06  Love song                    03:49  1:1    ---     [* META]│ │
│ └──────────────────────────────────────┘ └───────────────────────────────────────────────────────────────────────┘ │
├─[ Enter:QueueAll  Space:Select  a:ToggleAll  f:ItemFormat  m:ItemMeta  s:ItemSub  G:GlobalRule  Q:DetachCLI ]──────┤
└────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### 4.3 Wireframe — Page `[2] QUEUE MANAGER` (Dedicated Separate Page)

```text
┌──────────────[ MVD v0.1.0 | 1:EXTRACTOR  2:QUEUE(3)*  3:LOGS  4:HISTORY  5:SETTINGS | SPEED: 18.4 MiB/s ]──────────┐
│ ┌─────────────────[ CONCURRENT DOWNLOAD QUEUE : 3/3 WORKERS | RATE: UNLIMITED | ~/Downloads/mvd/ ]───────────────┐ │
│ │   #   STATE      TITLE / SOURCE                         FORMAT / SUBS         PROGRESS           SPEED/ETA     │ │
│ │ ▸ 01  [ACTIVE]   you need to learn BASH Scripting EP 1  2160p AV1+Opus [MKV]  ██████████░░░░ 71% 11.2M 00:14   │ │
│ │                  └─[SUB: EN(Man), ID(Auto)] [FRAG: 4/4] [RESUME: .part READY]                                  │ │
│ │   02  [MUXING]   Mariners Apartment Complex (VEVO)      1080p VP9+Opus [MKV]  ██████████████ 99% ffmpeg remux  │ │
│ │   03  [ACTIVE]   Lana Del Rey - Love song               Opus 160k + LRC       ██████░░░░░░░░ 42%  7.2M 00:03   │ │
│ │   04  [PAUSED]   Fuck it I love you (Spotify Bridge)    Opus 160k + 1:1 Cover ██░░░░░░░░░░░░ 15%  ---  --:--   │ │
│ └────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘ │
│ ┌─────────────────────────────────────────[ SELECTED JOB TELEMETRY : #01 ]───────────────────────────────────────┐ │
│ │ FILE  : ~/Downloads/mvd/you need to learn BASH Scripting RIGHT NOW - EP 1 [SPwyp2NG-bE].mkv                    │ │
│ │ STAGE : Downloading Video Stream (f401: av01.0.12M.08) | Chunks: 4/4 | Retries: 0/5                            │ │
│ └────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘ │
├─[ p:Pause/Resume  c:CancelJob  r:Retry  +/-:Priority  L:RateLimit  x:ClearDone  Q:DetachToCLI ]───────[ 3 ACTIVE ]─┤
└────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### 4.4 Wireframe — Centered Floating Modals (`Metadata Editor` & `TUI-to-CLI Detach`)

```text
                ┌─────────────────────[ METADATA & TAG EDITOR ]──────────────────┐
                │                                                                │
                │  CONTAINER    : [ MKV (Matroska) / OPUS Audio ]                │
                │  TITLE        : [ Love song (Remastered)█                    ] │
                │  ARTIST       : [ Lana Del Rey                               ] │
                │  ALBUM        : [ Norman Fucking Rockwell!                   ] │
                │  TRACK / YEAR : [ 06 / 14 ]                  YEAR : [ 2019 ]   │
                │  SUBTITLES    : [ EN (Manual) + ID (Auto-ASR) ]                │
                │  ATTACHMENTS  : [x] 1:1 Cover Art   [x] Synced LRC (lofty)     │
                │                                                                │
                │                 [ ENTER : SAVE OVERRIDE [*] ]                  │
                └────────────────────────────────────────────────────────────────┘

                ┌──────────────[ DETACH TUI TO CLI PROGRESS MODE ]───────────────┐
                │                                                                │
                │  [!] 3 Downloads are currently active in the background.       │
                │                                                                │
                │  Exiting TUI will NOT cancel active downloads.                 │
                │  Progress will continue in standard CLI output mode.           │
                │  (Pressing Ctrl+C in CLI mode stops workers & saves .part      │
                │   state so you can resume next time you open MVD).             │
                │                                                                │
                │        [ ENTER / Y : DETACH TO CLI ]    [ N : STAY ]           │
                └────────────────────────────────────────────────────────────────┘
```

---

## 5. XDG Base Directory & Cache Management Specification

### 5.1 Does `mvd` Need Cache Management?
**Yes — strictly bounded, XDG-compliant cache management is essential** for three reasons:
1. **Playlist Re-Inspection Speed**: Inspecting a 50-item YouTube or YouTube Music playlist fetches dozens of thumbnails and `lrclib` lyrics. Caching decoded/resized thumbnails and JSON metadata avoids redundant network requests and prevents HTTP 429 rate-limiting when navigating up and down (`j`/`k`) in the TUI.
2. **Preventing Disk Bloat (`[!] tidak mengotori system`)**: Unbounded thumbnail and metadata caches silently grow over months. `mvd` enforces an **LRU + TTL Cache Eviction Policy** inside `$XDG_CACHE_HOME/mvd/`:
   - **Maximum Cache Quota**: `256 MiB` (configurable in `config.toml`).
   - **TTL (Time-To-Live)**:
     - Stream URLs / `yt-dlp` signed manifests: `3 hours` (since YouTube signed URLs expire after ~6 hours).
     - Thumbnail images (`$XDG_CACHE_HOME/mvd/thumbs/<sha256>.png`): `7 days`.
     - LRCLIB Lyrics (`$XDG_CACHE_HOME/mvd/lyrics/<sha256>.json`): `30 days`.
   - **One-Key Cache Purge**: Pressing `X` on Page `[5] SETTINGS & SECURITY` immediately purges `$XDG_CACHE_HOME/mvd/*` and displays reclaimed bytes.

### 5.2 Complete XDG Directory Mapping

| XDG Variable | Resolved Path on User Host | Purpose | Persistence & Permissions |
| :--- | :--- | :--- | :--- |
| `XDG_CONFIG_HOME` | `/home/porvata/.config/mvd/config.toml` | User settings, theme overrides, `pbf03` profile lock rule, default format profiles | Persistent (`0600`) |
| `XDG_DATA_HOME` | `/home/porvata/.local/share/mvd/history.db` | SQLite WAL download history ledger & duplicate detection index | Persistent (`0600`) |
| `XDG_STATE_HOME` | `/home/porvata/.local/state/mvd/queue.json` | Crash-resilient active/paused download queue state for session resume | Persistent (`0600`) |
| `XDG_CACHE_HOME` | `/home/porvata/.cache/mvd/{thumbs,lyrics,meta}/` | LRU-bounded (`256 MiB`) thumbnail, lyrics, and non-sensitive metadata cache | Ephemeral/Evictable (`0700`) |
| `XDG_RUNTIME_DIR` | `/run/user/1000/mvd/` | **RAM-only (`tmpfs`)** ephemeral cookie jar & IPC sockets | **WIPED ON EXIT (`0700` dir / `0600` files)** |
| **Safe Download Dir** | `/home/porvata/Downloads/mvd/` | Final muxed media files (`.mkv`, `.opus`, `.flac`, `.mp4`) | User Media Storage (`0755`) |

---

## 6. Structured Error, Warning & Info Handling

Every operational event in `mvd` is classified into a typed severity tier with deterministic UI presentation:

1. **`[x] ERROR` (`#EF4444`)**:
   - **Examples**: Attempted access to non-`pbf03` Firefox profile; `yt-dlp` extractor failure (private/deleted video, geo-block); `ffmpeg` non-zero exit code; disk full in `~/Downloads/mvd/`.
   - **Behavior**: Halts only the affected job (never crashes the TUI), increments error counter in Topbar (`[x] 1 ERR`), displays concise cause in Footer Bar, and logs the full command + stderr trace in `Page [3] RAW TELEMETRY LOGS`. Pressing `e` on a failed queue item opens a centered **Error Diagnostic Modal** with actionable remediation steps.
2. **`[!] WARNING` (`#F59E0B`)**:
   - **Examples**: Duplicate media item detected in SQLite history ledger; manual Indonesian (`id`) subtitle unavailable (falling back to auto-generated ASR `id-orig`); LRCLIB returned plain lyrics instead of synced `.lrc`; HTTP 429 retry backoff in progress.
   - **Behavior**: Non-blocking badge in the metadata/queue row and footer notification banner (`5s` auto-dismiss).
3. **`[i] INFO` (`#3B82F6`) / `[+]` SUCCESS (`#28b877`)**:
   - **Examples**: Playlist flat-extraction complete (`18 items loaded`); `pbf03` sandbox cookies verified (`0 work profiles touched`); `ffmpeg` MKV remuxing completed in `0.8s`.

---

## 7. Mandatory Test Suite Matrix & Empirical Live-Test Findings (`T-01` – `T-07`)

All 7 user-provided URLs were live-tested against `yt-dlp 2026.10.26`, `lrclib.net`, and Spotify's Embed API:

| Fixture ID | Category | Test URL | Empirical Live-Test Finding | Thumbnail Aspect & Preview Behavior | Container, Subtitles & Lyrics Behavior |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **T-01** | Video Playlist | `https://youtube.com/playlist?list=PLIhvC56v63IKioClkSNDjW7iz-6TFvLwS&si=Me_WJsw1pF2svME5` | *NetworkChuck — BASH Scripting* (5 videos). `--flat-playlist` resolves in `<1.2s`. | `16:9` (`336x188` flat / `1920x1080` hydrated); Lyrics pane hidden. | `.mkv` (default) or `.mp4` (with external `.srt` sidecar + `mov_text`). |
| **T-02** | Video Single | `https://youtu.be/SPwyp2NG-bE?si=0d_ISKZOBA72UU3C` | `115` formats (`AV1`, `VP9`, `H.264` up to `3840x2160`), `14` chapters. **Subtitles**: `en` is in `manual_subtitles`, while `id` is ONLY in `automatic_captions`! | `16:9` (`1920x1080` `maxresdefault.webp`). | **Smart Dual-Source Subtitles**: Fetches `en` from Manual + `id` from Auto-ASR (`--write-subs --write-auto-subs --sub-langs "en,id"`), deduplicating `en-auto`. |
| **T-03** | Music Album Artist | `https://music.youtube.com/playlist?list=OLAK5uy_kXEi_1ry11kHAe71Z36AShGLhVXejbK38&si=s72KLYDSaQ7vXc-d&drli=1` | *Lana Del Rey — Norman Fucking Rockwell!* (`14` tracks). Album header has `1200x1200` `1:1` cover, while Track 2 (`sgm3F5D1yz8` *Mariners Apartment Complex*) is a VEVO video (`1920x1080`)! | Renders `1:1` when album art is selected, or native `16:9` when inspecting VEVO video track, with **LRCLIB Lyrics Preview** below thumbnail. | `.opus` / `.flac` (tagged via pure-Rust `lofty` crate) or `.mkv`. |
| **T-04** | Music Single 1 (Age-Gated VEVO) | `https://music.youtube.com/watch?v=LrSX_OcpeJg&si=3_UGDgDjz88J6h6J&drli=1` | *Lana Del Rey — Fuck it I love you / The greatest* (`560s` double VEVO MV). Returns **`Sign in to confirm your age`** with `--no-cookies`! | Proves mandatory need for **`pbf03` Sandbox Cookie Vault** fallback on age-gated YT Music videos; renders native **`16:9` VEVO thumbnail as-is** + LRCLIB below. | Unlocks via `pbf03` domain-scoped `.youtube.com` cookie jar in `/run/user/1000/mvd/vault/`. |
| **T-05** | Music Single 2 (Art Track) | `https://music.youtube.com/watch?v=5xx9gTEQaiQ&si=k20r8T991nBzeW0t&drli=1` | *Lana Del Rey — Love song* (`229s`). Video stream is `1080x1080` (`1:1`), `yt3.googleusercontent.com` `544x544` has `preference: -39` while pillarboxed `hq720.jpg` (`1280x720`) has `preference: -3`. LRCLIB matched synced lyrics (`id: 6977868`). | Selects true `1:1` `yt3.googleusercontent.com` square cover (ignoring pillarboxed `hq720.jpg`) + **Synced LRCLIB Lyrics** (`[00:29.26] In the car...`) below thumbnail. | `.opus` / `.flac` with `1:1` cover & synced `.lrc` embedded via Rust `lofty`. |
| **T-06** | Music Community Playlist | `https://music.youtube.com/playlist?list=PLJNrcbVaS76JVPdSmjIs2Iep9C7XBUh74&si=YKqZuWdEJlT6XagI&drli=1` | *shiloh dynasty - all songs* (`43` tracks). Mixes `- Topic` Art Tracks (`RgjN09waabQ`, `KkAHECiG2Ro`) and non-Topic video uploads (`WHVupRkTdqo` by `CORBAL`). | Dynamically renders `1:1` for `- Topic` Art Tracks and **`16:9` as-is** for non-square video uploads (`CORBAL`), with LRCLIB preview underneath. | Batch queue with per-track SQLite deduplication. |
| **T-07** | Spotify Track (Bridge) | `https://open.spotify.com/track/7MtVPRGtZl6rPjMfLoI3Lh?si=c0ea33c09cfc4bd9` | Direct `yt-dlp` fails with `ERROR: [DRM]`. Public Spotify Embed JSON yields *Lana Del Rey — Fuck it I love you*, `218287ms`, `640x640` cover, and LRCLIB synced lyrics (`id: 30437529`). Duration-scored `ytsearch5` matches `wDWE_Qp3oKM` (`219s`, $\Delta t = 0.7\text{s}$), avoiding the `560s` age-gated MV! | `1:1` (`640x640` official Spotify CDN cover) + **Synced LRCLIB Lyrics** (`[00:00.87] I like to see everything in neon...`) below thumbnail. | Downloads high-bitrate audio via YT Music bridge + embeds official Spotify `1:1` cover, metadata & LRCLIB synced lyrics. |
