# mocktool golden files — capture record + honesty table (plan M15)

Mock-drift law: every canned output mocktool serves is a GOLDEN FILE
captured from ONE REAL tool run on the dev machine, with the exact
capture command recorded below. Never hand-written. Where a tool is not
installed locally the golden is declared PENDING (no golden file exists;
tests for that tool use only structural mock modes — `env`, `exit`,
`hang`, `write`) until the capture happens on a machine that has the
tool (F3 / dev-machine re-check).

Capture environment (2026-09-24, Windows):

- ffmpeg  8.1.1-essentials_build-www.gyan.dev  (`C:\ProgramData\chocolatey\bin\ffmpeg.exe`)
- ffprobe 8.1.1 (same build)
- pandoc  3.9.0.2  (`C:\Users\vsbb1\AppData\Local\Pandoc\pandoc.exe`)

All commands run from the repository root.

## Captured (present locally)

| Golden | Capture command (verbatim) | Real tool run |
| --- | --- | --- |
| `ffprobe/probe-av-json.golden` | input fixture first: `ffmpeg -y -hide_banner -loglevel error -f lavfi -i "sine=frequency=440:duration=0.2" -c:a pcm_s16le -ar 16000 harness-test-support\golden\mocktool\_input\tiny.wav` then `ffprobe -v error -show_format -show_streams -of json harness-test-support\golden\mocktool\_input\tiny.wav` | ffprobe 8.1.1 on the committed `tiny.wav` (16 kHz mono pcm_s16le, 0.2 s) |
| `ffmpeg/version.golden` | `ffmpeg -version` (first 6 lines) | ffmpeg 8.1.1 essentials build |
| `pandoc/version.golden` | `pandoc --version` (first 6 lines) | pandoc 3.9.0.2 |

## Pending (tool absent on this machine — declared-mock only)

| Tool | Golden | Status | First capture owner |
| --- | --- | --- | --- |
| blender | scene-dump JSON, builder artifacts | PENDING (blender not installed) | M16 evidence (real-Blender run is an M16 requirement) |
| whisper-cpp | transcript JSON | PENDING (whisper-cli not installed) | M14/M25 scheduled tier |
| libreoffice (soffice) | `--version` output | PENDING (soffice not installed) | M18/M19 round-trip hooks |
| freecadcmd | `--version` output | PENDING (FreeCAD not installed) | M20 |
| gdal | `gdalinfo --version` output | PENDING (gdal not installed) | M21 |

A pending row means exactly this: no golden file is committed, none is
faked, and no test asserts canned output content for that tool. When a
pending tool becomes available, capture with the documented command,
commit the golden, and move its row to the table above.
