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
| `ffprobe/probe-av-json.golden` | input fixture first: `ffmpeg -y -hide_banner -loglevel error -f lavfi -i "sine=frequency=440:duration=0.2" -c:a pcm_s16le -ar 16000 austringer-test-support\golden\mocktool\_input\tiny.wav` then `ffprobe -v error -show_format -show_streams -of json austringer-test-support\golden\mocktool\_input\tiny.wav` | ffprobe 8.1.1 on the committed `tiny.wav` (16 kHz mono pcm_s16le, 0.2 s) |
| `ffmpeg/version.golden` | `ffmpeg -version` (first 6 lines) | ffmpeg 8.1.1 essentials build |
| `pandoc/version.golden` | `pandoc --version` (first 6 lines) | pandoc 3.9.0.2 |
| `ffprobe/probe-webm-json.golden` | input fixture first: `ffmpeg -y -hide_banner -loglevel error -f lavfi -i "testsrc=duration=0.2:size=64x48:rate=10" -f lavfi -i "sine=frequency=440:duration=0.2" -c:v libvpx-vp9 -c:a libopus austringer-test-support\golden\mocktool\_input\tiny-vp9.webm` then `ffprobe -v error -show_format -show_streams -of json austringer-test-support\golden\mocktool\_input\tiny-vp9.webm` | ffprobe 8.1.1 on the committed `tiny-vp9.webm` (vp9 64x48 @10 fps + libopus 48 kHz, 0.208 s) — M17 |
| `ffprobe/probe-h264-mp4-json.golden` | input fixture first: `ffmpeg -y -hide_banner -loglevel error -f lavfi -i "testsrc=duration=0.2:size=64x48:rate=10" -c:v libx264 -pix_fmt yuv420p austringer-test-support\golden\mocktool\_input\tiny-h264.mp4` then `ffprobe -v error -show_format -show_streams -of json austringer-test-support\golden\mocktool\_input\tiny-h264.mp4` | ffprobe 8.1.1 on the committed `tiny-h264.mp4` (h264 64x48, 0.2 s) — M17. The h264 fixture is dev-machine TEST-INPUT synthesis for the RED-input lanes (decode-away legal / stream-copy refused); the adapter itself never emits H.264/AAC (constraint 7) |
| `ffmpeg/thumbnail-png.golden` | `ffmpeg -y -hide_banner -loglevel error -ss 0 -i austringer-test-support\golden\mocktool\_input\tiny-vp9.webm -frames:v 1 <temp>.png` (then moved onto the `.golden` name — the image2 muxer needs the `.png` extension) | real keyframe PNG (1237 bytes) of `tiny-vp9.webm` — M17 |
| `ffmpeg/pcm-md5.golden` | `ffmpeg -hide_banner -loglevel error -i austringer-test-support\golden\mocktool\_input\tiny-vp9.webm -map 0:a -f md5 -` | real decode-hash of the opus track (`MD5=0e0344ab90750a8dbcb7b2274096a081`) — M17 |

## Pending (tool absent on this machine — declared-mock only)

| Tool | Golden | Status | First capture owner |
| --- | --- | --- | --- |
| blender | scene-dump JSON, builder artifacts | PENDING (blender not installed). M16 landed the PARAMETERIZED structural mock instead (`blendbuild`/`blenddump` modes deriving the census from the builder script's embedded spec - three-way triangulation with the python template and the independent spec->tree projection); the real-run waiver line is in `.omo/evidence/task-M16-media-formats.txt` | F3 / M25 scheduled tier (first real capture flips the manifest pin off `pending-first-real-run`) |
| whisper-cpp | transcript JSON | PENDING (whisper-cli not installed) | M14/M25 scheduled tier |
| libreoffice (soffice) | `--version` output | PENDING (soffice not installed) | M18/M19 round-trip hooks |
| freecadcmd | `--version` output, topology-dump census JSON | PENDING (FreeCAD not installed). M20 landed the PARAMETERIZED structural mock instead (`stepbuild`/`stepdump` modes deriving the topology census from the builder script's embedded spec - three-way triangulation with the python template and the independent spec->tree projection; the pinned per-primitive face/edge/vertex table is corrected by the first real capture); the real-run waiver line is in `.omo/evidence/task-M20-media-formats.txt` | F3 / M25 scheduled tier (first real capture flips the manifest pin off `pending-first-real-run`) |
| gdal | `gdalinfo --version` output | PENDING (gdal not installed). M21 landed the PARAMETERIZED structural mock instead (`gdaltranslate`/`gdalinfo` modes: hand-rolled TIFF writer/reader carrying the real GeoKey directory, pixel scale + tiepoint, and the COG ghost-area/tiled/overview layout - three-way triangulation with the adapter's tiff-crate extractor and the independent spec->tree projection); the committed fixture `austringer-media/fixtures/geotiff-geo.tif` is a capture from THAT mock writer (spec: 4x4 u8, EPSG:4326, origin 10/50, pixel 0.5/0.25), regenerable deterministically: stage `gdal-gen/spec.json` + a plain 4x4 baseline tiff, run `mocktool gdaltranslate`; the real gdal_translate + gdalinfo run waiver line is in `.omo/evidence/task-M21-media-formats.txt` | F3 / M25 scheduled tier (first real capture flips the manifest pin off `pending-first-real-run`) |

A pending row means exactly this: no golden file is committed, none is
faked, and no test asserts canned output content for that tool. When a
pending tool becomes available, capture with the documented command,
commit the golden, and move its row to the table above.
