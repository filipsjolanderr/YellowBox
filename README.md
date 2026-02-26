# YellowBox

A desktop application for recovering and exporting Snapchat Memories from a data export archive. It downloads the original media files if not already present, composites any overlays, restores GPS metadata, and sets file timestamps to match the original capture date.

## What it does

Snapchat's data export provides a JSON manifest (`memories_history.json`) with download URLs and metadata for each memory. YellowBox reads that manifest, downloads each file if not already present, and processes it through a pipeline:

1. **Download** — Fetches the raw file from Snapchat's CDN. Files arrive as either a ZIP archive (containing a media file and an optional overlay PNG) or a bare media file. Downloads are retried up to three times on failure.
2. **Extract** — Unpacks ZIP archives, separating the main media file from the overlay.
3. Combine — Composites the overlay onto the media. Images are processed natively using the `image` crate. Videos are processed with a bundled FFmpeg binary using a `scale2ref` filter to scale the overlay to the video's resolution before compositing.
4. **Metadata** — Writes GPS coordinates to the output file (EXIF for images via `little_exif`, ISO 6709 location atom for videos via FFmpeg), then sets the file's modification and access timestamps to the original capture date.

Each memory progresses through discrete states (`Pending`, `Downloaded`, `Extracted`, `Combined`, `Completed`, `Failed`, `Paused`) tracked in a per-session SQLite database. If processing is interrupted, the pipeline resumes from the last completed state rather than starting over. Items that already exist on disk are detected on startup and marked completed automatically.

Processing runs concurrently, using one worker per available logical CPU.

## Technology

| Layer | Technology |
|---|---|
| UI | SvelteKit 5, TypeScript, Tailwind CSS v4 |
| Desktop shell | Tauri 2 |
| Backend | Rust (Tokio async runtime) |
| Database | SQLite via `tokio-rusqlite` |
| Image compositing | `image` crate |
| Video compositing / metadata | FFmpeg (bundled sidecar binary) |
| Image EXIF | `little_exif` |
| HTTP | `reqwest` |
