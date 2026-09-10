# nexus

A small video editor built on top of `ffmpeg`. One video track: import clips,
set an in/out point on each, put them in order, export the result to an mp4.

It is **not** a replacement for DaVinci Resolve or Premiere — no effects, no
transitions, no multi-track, no audio mixing. It does the boring 80% (trim +
arrange + concat) reliably, by shelling out to `ffmpeg` instead of linking it.

## Requirements

- Rust 1.98+
- `ffmpeg` and `ffprobe` on your `PATH` (override with `NEXUS_FFMPEG` /
  `NEXUS_FFPROBE`)

## Build

```sh
cargo build --release
# CLI-only build (no GUI dependencies):
cargo build --release --no-default-features
```

## GUI

```sh
nexus            # or: nexus gui
```

- **File → Import clips** — add one or more videos; each lands on the timeline
  as a full-length clip.
- Select a clip on the timeline strip; set **In** / **Out** and reorder it in
  the inspector on the right.
- Scrub the **playhead** slider to preview a frame; **Play** scrubs in real time
  (video only, no audio).
- **File → Export** — renders the timeline to an mp4 in a background thread.
- **File → Save / Open** — projects are plain JSON (see below).

## CLI

```sh
nexus probe clip.mp4

# trim one file
nexus trim raw.mp4 --start 00:00:05 --end 00:00:20 -o cut.mp4

# join files (scaled/padded to a common format)
nexus concat a.mp4 b.mp4 c.mp4 -o joined.mp4 --width 1920 --height 1080 --fps 30

# project workflow
nexus new my.json --width 1920 --height 1080 --fps 30
nexus add my.json intro.mp4 --in 0 --out 4
nexus add my.json body.mp4
nexus ls  my.json
nexus export my.json -o final.mp4
```

## Project file

```json
{
  "name": "My Project",
  "width": 1920,
  "height": 1080,
  "fps": 30.0,
  "timeline": {
    "clips": [
      { "id": 1, "source": "/abs/path/intro.mp4", "src_in": 0.0, "src_out": 4.0 }
    ]
  }
}
```

## How export works

Each clip is re-encoded to identical parameters (project `width`/`height`/`fps`,
H.264 + AAC, silent audio synthesised for clips that have none), then the parts
are joined with ffmpeg's stream-copy `concat` demuxer. Two encodes, but it joins
arbitrary sources without artefacts.

## Layout

| File | Role |
|------|------|
| `src/lib.rs` | crate root |
| `src/media.rs` | `ffprobe` wrapper |
| `src/project.rs` | project / timeline / clip model + JSON I/O |
| `src/render.rs` | ffmpeg orchestration (trim, concat, export) |
| `src/main.rs` | CLI (clap) + GUI dispatch |
| `src/gui.rs` | egui front-end (behind the `gui` feature, on by default) |

## License

MIT — see [LICENSE](LICENSE).
