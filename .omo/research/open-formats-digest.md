# Open Formats Research Digest: harness media-formats wave (researched 2026-09)

## 1. Master verdict table

*Every format from the four research lanes. Verdicts carry over verbatim from the cluster sections (3.1-3.6), never softened. Gen tier: P = pure Rust, S = subprocess, W = WASM-viable, C = C bindings. Priority is the consolidated slate call, rationale in section 6.*

| Format | Domain | Spec body / version (year) | Verdict | One-line law | Tier | Priority |
|---|---|---|---|---|---|---|
| PDF | Documents | ISO 32000-2:2020 (PDF 2.0) | GREEN | Adobe Public Patent License: RF for Essential Claims, defensive suspension only | P + S | CORE |
| PDF/A | Documents | ISO 19005-4:2020 (PDF/A-4) | GREEN | Archival profile, same Adobe grant; archive/e-invoice mandates | P + S | CORE |
| ODF 1.4 (ODT/ODS/ODP) | Documents | OASIS 2025-12-03; ISO/IEC 26300:2025 | GREEN | Royalty-free IP mode, no caveats; German gov mandate Mar 2026 | P (ODS) / S | DEFERRED |
| OOXML (DOCX/XLSX/PPTX) | Documents | ECMA-376 5th ed 2021; ISO/IEC 29500 | YELLOW, treat GREEN | MS Open Specification Promise: irrevocable covenant; ship Transitional | P (xlsx/docx) / S (pptx) | CORE |
| Legacy DOC/XLS/PPT | Documents | OLE2/CFB | RED | Reverse-engineered, superseded | S (LO read) | EXCLUDE (read-only via LO) |
| Markdown | Documents | CommonMark 0.31.2 + GFM | GREEN | Community spec, zero patents | P | CORE |
| AsciiDoc | Documents | Eclipse spec pre-1.0; Asciidoctor 2.0.x | GREEN (spec YELLOW) | Tooling open, weak Rust ecosystem | S | DEFERRED |
| LaTeX/TeX | Documents | LaTeX2e on TeX Live 2025 | GREEN | LPPL 1.3c free; GPL engines fine as subprocess | P emit / S render | DEFERRED |
| EPUB 3.3 | Documents | W3C Rec May 2023 (3.4 rev underway) | GREEN | W3C RF policy | P + S | DEFERRED |
| HTML5 | Documents | WHATWG Living Standard | GREEN | RF; not archival on its own | P | CORE |
| DjVu | Documents | AT&T lineage, dormant | YELLOW, effectively exclude | Rights shuffled to Cuminas/ePapyrus; zero activity | S read-only | EXCLUDE |
| UOF | Documents | Chinese GB standard | SKIPPED | Regional-only, out of brief | n/a | EXCLUDE |
| PNG + APNG | Images | W3C Rec 3rd ed 2025-06-24 | GREEN | Built around the LZW patents; W3C RF policy | P | CORE |
| SVG 1.1 (+SVG 2 subset) | Images | W3C SVG 1.1 2nd ed 2011; SVG 2 still CR | GREEN | W3C RF; target 1.1 semantics | P | CORE |
| JPEG baseline | Images | ITU-T T.81 / ISO 10918-1 (1992) | GREEN | Forgent assert died 2006 | P | CORE |
| WebP | Images | Google VP8-in-RIFF spec | GREEN | Irrevocable VP8 grant + 2013 MPEG-LA deal + expiry | P lossless / S lossy | CORE |
| AVIF | Images | AV1 on HEIF/BMFF (ISO 23008-12 / 14496-12) | GREEN w/ footnote | Sisvel pool claims disputed, never collected; container clean (BMFF patents expired) | P encode (slow) / S decode | CORE |
| JPEG XL | Images | ISO/IEC 18181:2022 | GREEN | JPEG Committee RF regime; adoption reversing (Firefox 157 default Sept 2026) | P decode / S encode | DEFERRED (promote on Chrome flip) |
| TIFF (plain) | Images | TIFF 6.0 (Aldus/Adobe 1992) | GREEN | Free spec; LZW expired 2004 | P | DEFERRED |
| GeoTIFF | Geo | OGC GeoTIFF 1.1 (OGC 19-075, 2019) | GREEN | OGC keys on the free TIFF spec | S | CORE |
| OpenEXR | Images | OpenEXR 3.x (ASWF); ACES ST 2065-4 | GREEN | BSD, ASWF | P | DEFERRED |
| KTX2 | Images | Khronos KTX v2 | GREEN | Khronos RF + Basis Universal RF contribution | P parse / S encode | DEFERRED |
| DDS | Images | MS spec (BC6H/BC7 documented) | GREEN | S3TC patents expired ~2017 | P decode / S encode | DEFERRED |
| QOI | Images | Szablewski 2022, frozen, MIT | GREEN | Public-domain-ish, niche gamedev | P | EXCLUDE |
| DNG | Images | Adobe DNG 1.7 -> ISO 12234-4:2026 (Mar 2026) | YELLOW | RF license with revocation clause; Adobe trademark | S | EXCLUDE (skip) |
| Proprietary RAW (CR3/NEF/...) | Images | vendor-proprietary | RED | Undocumented, vendor-controlled | n/a | EXCLUDE |
| FLIF | Images | dead | n/a | Confirmed dead, superseded by JXL | n/a | EXCLUDE |
| JPEG LS / XR / 2000 | Images | ISO 14495 / 29199 / 15444 | RED (2000) | JP2 patent-thicketed dead; LS/XR legally clean but irrelevant | n/a | EXCLUDE |
| EXIF / XMP sidecars | Images | JEITA Exif 3.0 (2023); ISO 16684-1 | GREEN | Universal metadata layer | P | CORE (sidecar layer) |
| glTF 2.0 / GLB | 3D | Khronos 2.0; ISO/IEC 12141:2022 | GREEN | Khronos RF IP framework; cleanest in 3D | P | CORE |
| USD / USDZ | 3D | OpenUSD; AOUSD 2023; TomorrowOpen license | GREEN | Apache-2.0 minus trademark section; LF/JDF governance | P (pre-1.0) / S | DEFERRED |
| 3MF | 3D | 3MF Consortium core 1.x | GREEN | Specs free, member claims RF | P | DEFERRED (printing consumer) |
| STL | 3D | de-facto 3D Systems 1987 | GREEN | No owner; 80s patents expired | P | CORE (compat floor) |
| OBJ + MTL | 3D | Wavefront vendor docs ~1989 | GREEN | De-facto public domain | P | CORE |
| STEP AP242 | 3D/CAD | ISO 10303-242 Ed3 2022 | YELLOW | ISO-paywalled text; usage unrestricted, free schema mirrors | S (freecadcmd) | CORE |
| IGES | 3D/CAD | US PRO 5.3 (1996, last) | RED | Deprecated, superseded by STEP | n/a | EXCLUDE |
| BLEND | 3D | source-as-spec (DNA/SDNA) | YELLOW | No cross-version stability contract; never hand-write, bpy headless is THE pattern | S (bpy) | CORE |
| COLLADA | 3D | Khronos 1.5.1 (2008) | YELLOW | Open but dormant (Blender legacy I/O 2024) | P read | EXCLUDE |
| FBX | 3D | Autodesk proprietary | RED | Closed spec/SDK, no open write path | P read (ufbx) | EXCLUDE (read-only) |
| Alembic .abc | 3D | SPI/ILM 2011, mod-Apache | GREEN/YELLOW | Permissive license; VFX cache niche | S | DEFERRED |
| OpenVDB / nanoVDB | 3D | ASWF v13 (2026, Apache-2.0) | GREEN | Formerly MPL-2.0; volumetrics | S | DEFERRED |
| Draco | 3D transform | Google, Apache-2.0 | GREEN | Superseded by meshopt for delivery | P (alpha) | DEFERRED |
| meshopt | 3D transform | zeux, MIT | GREEN | EXT_meshopt_compression ratified | P (FFI) | DEFERRED |
| MaterialX | 3D materials | ASWF, Apache-2.0, v1.39.x | GREEN | Rising via USD; no Rust story | S | DEFERRED |
| Opus | A/V | IETF RFC 6716 (2012) | GREEN | Standards-Track RF declarations, no pool targets it | S (libopus) | CORE |
| FLAC | A/V | Xiph BSD-3 + RFC 9639 (Dec 2024) | GREEN | Dual-anchored, patent-free, lossless, bit-exact | P | CORE |
| Vorbis | A/V | Xiph Vorbis I (2004) | GREEN | Open; superseded by Opus | P dec / S enc | DEFERRED |
| WAV (PCM) | A/V | RIFF de-facto 1991 (RF64 >4 GiB) | GREEN | Zero licensing | P | CORE |
| MP3 | A/V | ISO/IEC 11172-3:1993 | GREEN | All patents expired 2017 (verified) | P dec / S enc | DEFERRED |
| AAC (LC/HE) | A/V | ISO/IEC 13818-7, 14496-3:2019 | RED | Via Licensing pool alive; libfdk_aac = nonfree binary | S (avoid) | EXCLUDE (no generation) |
| ALAC | A/V | Apple, Apache-2.0 since 2011 | GREEN | Clean, Apple-ecosystem niche | S | DEFERRED |
| AV1 | A/V | AOMedia v1.0.0 (2018) | GREEN | AOM cross-license + defensive termination; zero sustained rulings | S (SVT-AV1; rav1e P option) | CORE |
| VP9 | A/V | Google WebM spec | GREEN | Google RF grant; MPEG LA dropped 2013 | S | CORE (fallback) |
| Theora | A/V | Xiph VP3-derived (2004) | GREEN but DEAD | libtheora dead since ~2010-2013 | n/a | EXCLUDE |
| Matroska (MKV) | A/V | IETF RFC 9559 (Oct 2024); EBML RFC 8794 | GREEN | Formal IETF standard; archival pairing with FFV1 | S | CORE |
| WebM | A/V | Matroska profile | GREEN | BSD + Google grants | S | CORE |
| MP4 / BMFF / M4A | A/V | ISO/IEC 14496-12/-14 | YELLOW container, RED payload | Container open ISO; M4A bites only via AAC payload | S | DEFERRED |
| Ogg | A/V | RFC 3533 (2003); MIME RFC 5334 | GREEN | Xiph open, IETF-documented | P | CORE (Opus/FLAC carrier) |
| Y4M | A/V | de-facto | GREEN | Raw encoder pipe | P | DEFERRED (internal) |
| FFV1 | A/V | IETF RFC 9043 (2021) | GREEN | Archival lossless; MKV+FFV1 LoC pairing | S | DEFERRED (archival) |
| "Filmux" | A/V | none found | NOT VERIFIABLE | No such notable format exists | n/a | EXCLUDE |
| SRT | A/V subs | de-facto | GREEN | Nothing to license | P | CORE (trivial) |
| WebVTT | A/V subs | W3C | GREEN | W3C RF; web standard | P | CORE (trivial) |
| ASS/SSA | A/V subs | de-facto (libass) | GREEN | Free; render engine is C | P auth / S render | DEFERRED |
| Parquet | Data | Apache parquet-format (ASF) | GREEN | Apache-2.0; THE columnar standard | P | CORE |
| Arrow IPC / Feather v2 | Data | Apache Arrow format spec | GREEN | Apache-2.0 in-memory standard | P | CORE |
| HDF5 | Data | The HDF Group 1.14.x | YELLOW | BSD-like but single-vendor stewardship | C bindings | DEFERRED |
| NetCDF-4 | Data | Unidata (v4 = HDF5) | GREEN/YELLOW | Permissive; inherits HDF5 single-vendor nature | C bindings | DEFERRED |
| FITS | Data | IAU FITS 4.0 (2016) | GREEN | NASA/IAU open since 1981; astronomy standard | C bindings | DEFERRED |
| NPY/NPZ | Data | NumPy npy-format | GREEN | Trivial documented format | P | DEFERRED |
| JSON / JSONL | Data | RFC 8259; jsonlines de-facto | GREEN | IETF, patent-free | P | CORE |
| CSV | Data | RFC 4180 (2005) | GREEN | IETF informational; dialect chaos only hazard | P | CORE |
| YAML | Data | yaml.org 1.2.2; RFC 9512 | YELLOW | Spec open; serde_yaml archived 2024 -> serde_yaml_ng / saphyr | P | DEFERRED (prefer TOML emit) |
| TOML | Data | TOML 1.0 (MIT spec) | GREEN | MIT spec, zero drama | P | CORE |
| SQLite | Data | sqlite.org file format (frozen) | GREEN | Public-domain code, contractually frozen format | P (rusqlite) | CORE |
| Protobuf | Data | Google proto3 (BSD spec) | GREEN | BSD + patent non-assert norms | P (prost) | DEFERRED |
| FlatBuffers | Data | Google, Apache-2.0 | GREEN | Zero-copy niche | P | DEFERRED |
| Cap'n Proto | Data | MIT | GREEN | Single maintainer, healthy | P | DEFERRED |
| ZSTD | Data | Meta, BSD-3 + patent grant | GREEN | Compression layer under Parquet/Arrow | P | CORE (layer) |
| gzip / lz4 | Data | RFC 1952/1951 | GREEN | Patent-clear by design | P | CORE (layer) |
| xz | Data | liblzma | YELLOW (operational) | CVE-2024-3094: pin/verify versions | P/S | DEFERRED |
| Zarr v3 | Data | zarr-specs CC-BY-4.0, 3.x finalized | GREEN | Cloud-native HDF5 challenger | P (zarrs) | DEFERRED (big-array trigger) |
| COG | Geo | Radiant Earth COG spec (community) | GREEN | Disciplined GeoTIFF; cloud-raster delivery standard | S | CORE |
| GeoPackage | Geo | OGC GeoPackage 1.4 (Feb 2024) | GREEN | OGC standard on public-domain SQLite; Shapefile successor | P / S | CORE |
| Shapefile | Geo | ESRI white paper (1998) | YELLOW (dated) | Free but format liabilities; read-only policy | P read | EXCLUDE (generation) |
| KML | Geo | OGC KML 2.3 | YELLOW | OGC-open, adoption flat; user-approved | P read / S emit | CORE (trivial emit) |
| GPX | Geo | TopoGrafix GPX 1.1 | GREEN | Free schema; GPS-track standard | P | DEFERRED |
| LAS / LAZ / COPC | Geo | ASPRS LAS 1.4 R15; OGC LAZ 1.4 CS (2026) | GREEN | LASzip Apache-2.0 since late 2021 (signed grant) | P | DEFERRED |
| GML | Geo | OGC GML 3.2.x / ISO 19136 | YELLOW (dated) | Verbose XML; INSPIRE/cadastral mandates only | S | EXCLUDE (unless mandated) |
| FlatGeobuf | Geo | Harrtell spec (MIT) | GREEN | Community-governed; GDAL/QGIS support | P | DEFERRED |
| MVT | Geo | Mapbox vector-tile-spec 2.1 (2016) | YELLOW-GREEN | Explicit free-use grant; Mapbox-governed, OGC-referenced only | P | DEFERRED |
| WKT / WKB / EWKB | Geo | OGC Simple Features; WKT2 = ISO 19162:2019 | GREEN | Encoding substrate under GPKG/PostGIS | P | CORE (substrate) |
| CRS sidecars (.prj) | Geo | OGC WKT2:2019; EPSG registry free | GREEN | Open registry | S (proj) | CORE (with geo lanes) |
| GeoJSON | Geo | IETF RFC 7946 (2016) | GREEN | IETF lightweight vector standard | P | CORE |

## 2. Licensing landmine summary

Anchor applied throughout: H.264/H.265 proprietary -> AV1/VP9; M4A problematic -> Opus (MP3 as the verified patent-expired fallback).

- Verified now-safe (expired patents, dead asserts: the lingering fear is folklore): MP3 (last US patent April 2017; Fraunhofer closed its licensing program), JPEG baseline (Forgent died 2006), TIFF LZW (2004), S3TC/BC1-3 (~2017), STL and OBJ (80s patents), WebP's VP8-era patents (filed 2008 or earlier, now expiring).
- Hard RED, active pools and real invoices: H.264/H.265 in any container (MPEG LA / Access Advance / Velos; x264/x265 GPL-or-commercial), AAC generation (Via Licensing pool alive; libfdk_aac makes the ffmpeg binary --enable-nonfree and unredistributable), HEIC/HEIF-with-HEVC, JPEG 2000 (patent-thicketed corpse), proprietary camera RAW, FBX generation (closed Autodesk SDK), IGES and legacy DOC/XLS/PPT (obsolescence REDs, not patent REDs).
- GREEN-with-covenant structures (all shippable, none FRAND): Adobe's PDF grant (defensive suspension), MS Open Specification Promise (irrevocable covenant, target OOXML Transitional), AOMedia cross-license (defensive termination; Sisvel's voluntary pool claims ~1,900 disputed patents; Elemedia/InterDigital/reported-2026-Dolby suits in flight with zero sustained rulings; EU probe closed with no action 2023), Khronos RF framework (glTF/KTX2), TomorrowOpen (Apache-2.0 minus trademark section, OpenUSD).
- Container is not codec: MP4/BMFF is an open ISO container (structural patents expired 2021+); M4A is only problematic via its AAC payload; MP4+AV1/Opus/ALAC is clean.
- Governance nuances that are not patent issues: STEP's ISO-paywalled text (usage unrestricted, free schema mirrors), MVT's Mapbox CC-BY text with an explicit free-use grant (OGC referenced, never adopted it), HDF5 BSD-like but single-vendor (The HDF Group; Zarr v3 is the open challenger), .blend source-as-spec with zero cross-version stability (GPL tool, emitted files are user data), DjVu commercial rights shuffled to Cuminas/ePapyrus (dormant).
- Recently resolved: LASzip LGPL -> Apache-2.0 (late 2021, signed Isenburg grant), sealed by the OGC LAZ 1.4 Community Standard (2026); pre-2021 LGPL binaries are the only residual surface.
- Tool licensing is not content licensing: distro ffmpeg (LGPL-2.1+) as subprocess; GPL tools (Blender, FreeCAD, pandoc, LibreOffice) fine as unmodified subprocesses; only output-content licensing matters, controlled by the ffmpeg flag table (section 3.4).
- Operational (not legal) cautions: xz CVE-2024-3094 (pin/verify), libwebp CVE history (prefer Rust decoders), never assert byte equality on lossy codecs.

Quotable one-paragraph form: build on PDF/PDF/A, OOXML-Transitional, PNG/SVG/JPEG/WebP/AVIF (+JXL when Chrome flips), glTF/STEP/STL/OBJ + Blender-bpy .blend, Opus/FLAC/WAV + AV1/VP9 in Matroska/WebM, Parquet/Arrow/SQLite/JSON/TOML, and GeoPackage/GeoTIFF-COG/GeoJSON; keep H.264/H.265, HEVC-anything, AAC generation, JPEG 2000, proprietary RAW, FBX-write, IGES, and legacy OLE2 binaries out. DNG stays YELLOW (revocation clause + trademark); AVIF is GREEN-with-footnote, not RED.

## 3. Per-cluster detail sections

### 3.1 Documents cluster (lane 1)

*Snapshot 2025-2026. Sources: ISO / PDF Association, OASIS, W3C, Ecma, LoC sustainability-of-formats, crates.io / lib.rs (release data pulled Sep 2026).*

#### Documents verdict recap

| Format | Verdict | One-line law |
|---|---|---|
| PDF (ISO 32000-2) | GREEN | ISO standard since 2008; Adobe Public Patent License grants royalty-free rights to all Essential Claims for Compliant Implementations (defensive-suspension only); spec free via PDF Association. |
| PDF/A (ISO 19005) | GREEN | ISO archival profile on top of PDF; same Adobe grant applies; mandatory in EU gov/archives. |
| ODF 1.4 (ODT/ODS/ODP) | GREEN | OASIS + ISO/IEC 26300, explicit royalty-free IP mode, no caveats; mandated by German gov (Mar 2026). |
| OOXML (DOCX/XLSX/PPTX) | YELLOW, treat as GREEN with footnotes | MS Open Specification Promise irrevocably covers current+future ECMA-376/ISO 29500, no royalty, no field-of-use limit, but it is a covenant-not-to-sue and Strict-vs-Transitional plus MS extension namespaces require care. Shippable without legal review. |
| DOC/XLS/PPT legacy binary | RED | Undocumented-in-ISO reverse-engineered formats, superseded by OOXML; exclude. |
| Markdown (CommonMark 0.31.2 + GFM) | GREEN | Pure community spec, CC-BY-SA docs, zero patents; the lingua franca of technical docs. |
| AsciiDoc | GREEN (spec: YELLOW) | Language/tooling open (EPL-2.0/MIT), but the Eclipse spec is still pre-1.0; Asciidoctor 2.0.x is the de facto norm; weak Rust ecosystem. |
| LaTeX/TeX | GREEN | LPPL 1.3c is a free license; GPL-incompatibility only affects distributing modified macro files, never documents you produce; engines GPL. |
| EPUB 3.3 | GREEN | W3C Recommendation (May 2023), W3C RF policy; EPUB 3.4 revision underway (Feb 2025 to 2027 charter). |
| HTML5 | GREEN | WHATWG Living Standard, royalty-free; not archival on its own (see fidelity notes). |
| DjVu | YELLOW, effectively exclude | Confirmed dormant: GPL reference impl (DjVuLibre) + patent grant exists, but commercial rights shuffled AT&T -> LizardTech -> Celartem -> Cuminas/ePapyrus, zero spec activity, no browser support, no maintained Rust crates. Read-only-if-needed via subprocess; RED for generation. |
| UOF | SKIPPED (EXCLUDE) | Chinese GB national standard with Chinese-labeled XML vocabulary; regional-only adoption, out of scope per brief. |

#### PDF / PDF/A detail

- Spec/body + version: ISO TC 171 SC 2, ISO 32000-2:2020 (PDF 2.0) plus industry Errata Collection 3; further revision in ISO ballot as of 2025-26. PDF/A: ISO 19005 family, current part 19005-4:2020 (PDF/A-4, based on PDF 2.0); PDF/UA-2 = ISO 14289-2:2024.
- Spec URL: https://pdfa.org/sponsored-standards/ (free ISO 32000-2 bundle, sponsored by Adobe/Apryse/Foxit); pdfa.org for PDF/A family.
- MIME: application/pdf (RFC 3778/8118). PDF/A uses the same MIME, no separate type.
- Magic bytes: 25 50 44 46 2D ("%PDF-"), e.g. %PDF-1.7 or %PDF-2.0.
- Licensing: Adobe Public Patent License, royalty-free for Essential Claims for Compliant Implementations, with defensive-suspension clause. GREEN.
- Adoption: universal (Acrobat, browsers, Word/LO export, print pipelines). PDF/A: archives (NARA/LoC), EU e-invoicing, German gov alongside ODF.
- Rust crates (verified Sep 2026): krilla 0.8.2 (June 2026, MIT/Apache-2.0, 1.28M downloads, active, high-level modern writer, typst-community adjacent, rustybuzz embedding); printpdf 0.12.5 (July 2026, mature writer, now also reads); genpdf (same author, layout layer on printpdf, slower release cadence); lopdf 0.40.0 (Mar 2026, 5.3M downloads, low-level parse/merge/manipulate, active); pdf-extract (release days ago, text extraction); PDFOxide (newer high-speed text/markdown extraction claimant); pdf-writer (typst team, low-level writer used by typst). Subprocess tier: pdfium-render / mupdf (FFI) for high-fidelity render+extract; qpdf/mutool for validation; veraPDF for PDF/A conformance.
- Generation tier: pure-Rust writer YES, solid (krilla/printpdf/genpdf); subprocess YES (wkhtmltopdf-era tools deprecated, prefer pandoc -> typst/LaTeX, LibreOffice -> pdf, or Chromium print-to-pdf); WASM YES (krilla/lopdf/pdf-extract are pure Rust; typst itself compiles to wasm).
- Fidelity risks: unembedded fonts / missing ToUnicode CMaps (breaks copy-paste and text extraction; the #1 agent-generated-PDF defect); wrong encoding (never emit WinAnsi for non-Latin1); broken xref/objects (the "Word repairs..." class of failures); ICC/transparency handling.
- Round-trip hooks: `qpdf --check`; `pdftotext` and compare content; fonts-embedded flag; veraPDF if PDF/A claimed.

#### ODF 1.4 (ODT/ODS/ODP) detail

- Spec/body + version: OASIS ODF 1.4, OASIS Standard 2025-12-03; ODF 1.3 = ISO/IEC 26300:2025; ISO 26300-1/-2/-3:2015 covers ODF 1.2.
- Spec URL: https://docs.oasis-open.org/office/OpenDocument/v1.4/os/
- MIME: application/vnd.oasis.opendocument.text / .spreadsheet / .presentation (IANA registry, RFC 6720 family).
- Magic bytes: 50 4B 03 04 (PK zip) with the FIRST zip entry stored uncompressed, named `mimetype`, containing the ODF MIME string.
- Licensing: OASIS royalty-free IP mode. GREEN, no caveats.
- Adoption: LibreOffice native; MS 365 reads/writes ODF 1.4 since v2404 (mid-2024; Mac 16.84); Google Workspace; German government mandates ODF + PDF/UA from March 2026 (Deutschland-Stack).
- Rust crates: spreadsheet-ods (Mar 2026 release, 49 versions, active; ODS read+write); ODT: thin `odt` crate or DIY zip+XML with quick-xml (structure is dramatically simpler than OOXML; this is ODF's superpower for generators); standard unwrap stack: zip + quick-xml + serde (all pure Rust, WASM-fine). Subprocess tier: LibreOffice / pandoc.
- Generation tier: pure-Rust: ODS good, ODT DIY-manageable; subprocess YES; WASM YES for zip+XML crates.
- Fidelity risks: automatic-vs-manual style families.
- Round-trip hooks: reload in LibreOffice; LO headless `--convert-to` no-op as smoke test; unzip + XML sanity.

#### OOXML (DOCX/XLSX/PPTX) detail, plus legacy binary

- Spec/body + version: Ecma TC46, ECMA-376 5th edition (Dec 2021); ISO line lags: ISO/IEC 29500:2016 + Amd 1:2017 (ECMA line newer than ISO line).
- Spec URL: https://ecma-international.org/publications-and-standards/standards/ecma-376/
- MIME: application/vnd.openxmlformats-officedocument.wordprocessingml.document (DOCX) / .spreadsheetml.sheet (XLSX) / .presentationml.presentation (PPTX); IANA registered. Legacy binary: application/msword etc.
- Magic bytes: 50 4B 03 04 (OPC zip package). Legacy binary: OLE2 CFB, D0 CF 11 E0.
- Licensing: YELLOW leaning GREEN. Microsoft Open Specification Promise (OSP) covers "current and future versions" of ISO/IEC 29500 and ECMA-376 (LoC-verified: promised "irrevocably"). See legal box below. Verdict as researched: shippable without legal review. Legacy DOC/XLS/PPT binaries: RED, undocumented-in-ISO, reverse-engineered, superseded; exclude.
- Adoption: industry default interchange: Word/Excel/PowerPoint, Google (round-trips), LibreOffice, Apple iWork export.
- Rust crates: XLSX read: calamine 0.36.1 (pure Rust; xlsx/xlsm/xlsb/xls and ODS; very active). XLSX write: rust_xlsxwriter 0.99.1 (by jmcnamara, author of Python XlsxWriter; extremely active, wasm-compatible; the strongest office-format crate in Rust). Modify: umya-spreadsheet (active; read+write+edit). DOCX write: docx-rs 0.4.19 (Feb 2026, wasm feature flag, 1.5M downloads). DOCX read: thin; use docx-rs / pandoc, or zip + quick-xml over word/document.xml. PPTX: no mature pure-Rust writer; use pandoc (`-o out.pptx --reference-doc`) or LibreOffice.
- Generation tier: pure-Rust: XLSX excellent, DOCX good, ODT/ODS good (see ODF), PPTX weak (DIY possible but heavy); subprocess YES (LibreOffice, pandoc); WASM YES for zip+XML crates and docx-rs.
- Fidelity risks: DOCX: direct formatting vs styles (breaks the user's style machinery), numbering definitions, relationship IDs, section properties, table widths; "Word found unreadable content" repairs on malformed XML; [Content_Types].xml zip structure must be right. XLSX: shared strings vs inline; formulas written without cached `<v>` values read as empty by calamine/pandas until opened in Excel (rust_xlsxwriter lets you write the cached value via the value param); calc chains. PPTX: slide-master/layout inheritance is where naive generators fail; generate from templates or pandoc reference doc.
- Round-trip hooks: reopen with calamine / docx reader; unzip + XML-schema sanity; LO headless `--convert-to` no-op as smoke test plus repair-prompt check.

##### OOXML legal box: is it shippable without legal review? Yes, treat as GREEN-with-footnotes

1. The Open Specification Promise is an irrevocable covenant not to sue covering ANY implementation of ECMA-376/ISO 29500: no royalty, no field-of-use restriction (explicitly NOT the H.264/FRAND situation). Applies to current and future versions.
2. Footnote A: it is a covenant, terminated only if you sue someone over essential claims (standard defensive clause; same shape as Adobe's PDF grant).
3. Footnote B: Strict vs Transitional. Target Transitional (what Word/LO/Google actually emit); ISO-Strict output has worse real-world consumption.
4. Footnote C: Microsoft extension namespaces (`w14:`, `w15:`, `mc:AlternateContent`, etc.) live in OSP-listed [MS-*] open specs; implement what you need, don't cargo-cult copy Word's output.
5. Trademark note: the "docx" file format is open; "Microsoft Word" the product is trademarked. No format-level trademark issue.

#### Markdown (CommonMark + GFM) detail

- Spec/body + version: CommonMark 0.31.2 spec (community; cmark/cmark-gfm reference impls); GFM = informal GitHub spec, strict superset (adds tables, strikethrough, tasklists, autolinks).
- Spec URL: https://spec.commonmark.org/ and https://github.github.com/gfm/
- MIME: text/markdown; charset=utf-8 (RFC 7763).
- Magic bytes: none (UTF-8 text; optional BOM).
- Licensing: CC-BY-SA spec docs, zero patents. GREEN.
- Adoption: dominant for technical docs: GitHub/GitLab render, rustdoc/mdBook, docs.rs, every static-site generator, LLM-adjacent tooling, pandoc.
- Rust crates: comrak (CommonMark 0.31.2 + GFM compliant, actively maintained, wasm-capable; the safe default parse+render); pulldown-cmark (pull parser, powers mdBook/rustdoc, active, 0.13.x); markdown crate (wooorm, CommonMark compliant); mdbook for doc sites.
- Generation tier: pure-Rust YES, subprocess YES, WASM YES, trivially at every tier.
- Fidelity risks: dialect bleed (GFM tables in a CommonMark-strict consumer), reference-link normalization, hard line breaks, tab/indent handling in lists.
- Round-trip hooks: parse with comrak in strict-CommonMark mode and GFM mode, diff ASTs; canonicalize with the GFM formatter.

#### AsciiDoc detail

- Spec/body + version: Eclipse AsciiDoc Language project; spec+TCK still pre-1.0 (spec in progress); de facto norm = Asciidoctor 2.0.x. Antora is the big consumer (docs-as-code).
- Spec URL: https://gitlab.eclipse.org/eclipse/asciidoc-lang/asciidoc-lang plus https://docs.asciidoctor.org/
- MIME: text/asciidoc / text/x-asciidoc (never IANA-registered; a draft exists).
- Magic bytes: none (UTF-8).
- Licensing: language open (EPL-2.0 docs / MIT Asciidoctor, MPL-2.0 Antora). GREEN language, YELLOW spec status.
- Adoption: strong niche (Red Hat, Eclipse Jetty, Spring docs via Antora); pandoc reads it (input only; no AsciiDoc writer in pandoc 3).
- Rust crates: no production-grade Rust parser (asciidocr experimental, dormant/partial). Generation-only (emit syntax, never parse) is feasible in pure Rust.
- Generation tier: pure-Rust: generate-only; subprocess: asciidoctor CLI (Ruby) required for full fidelity; WASM: no.
- Fidelity risks: include-macros and attribute resolution make naive generation leaky.
- Round-trip hooks: render through asciidoctor in CI as validation.

#### LaTeX/TeX detail

- Spec/body + version: no ISO standard; LaTeX2e (kernel continuously updated since 2015) on TeX engines. Distribution: TeX Live 2025 (annual; verified: all engines now emit PDF 1.7 by default as of TL 2025; DVI still available but legacy). Engines: pdfTeX, XeTeX, LuaTeX.
- Spec URL / license text: https://latex-project.org/lppl.txt ; tug.org for TeX.
- MIME: application/x-tex (.tex); DVI application/x-dvi. Magic: DVI is F7 02.
- Licensing: LPPL v1.3c, free software; GPL-incompatible rename-condition applies ONLY to distributing modified macro files, never to output documents; engines (pdfTeX/XeTeX/LuaTeX) are GPL (arm's-length subprocess use unproblematic). Fonts: Computer Modern (Knuth), Latin Modern (GFDL+FE), TeX Gyre free; embedding in PDFs fine.
- Adoption: academic/STM gold standard: arXiv, journals, Overleaf, book publishing. Pandoc emits LaTeX; typst is the rising Rust-native challenger (MIT/Apache-2.0).
- Rust crates: no Rust TeX engine; tectonic (Rust-driven XeTeX engine, self-contained, bundles TeX Live, semi-active, last release ~0.15.0 2024) is the closest. Generating .tex source is trivial string templating at any tier.
- Generation tier: pure-Rust: emit .tex trivially, rendering NO; subprocess: tectonic or stock latexmk/pdflatex/xelatex; WASM: no (practically; tectonic needs network/filesystem).
- Fidelity risks: package availability on minimal installs (tectonic solves by fetching), floats migrating, missing glyph warnings, references needing 2 passes.
- Round-trip hooks: compile with -halt-on-error, grep log for `Missing character` / `Overfull`, assert PDF page count > 0.

#### EPUB 3 detail

- Spec/body + version: W3C Recommendation EPUB 3.3 (May 2023) + OCF/RS specs; EPUB 3.4 revision started 2025-02-11 (chartered to 2027); 3.3 remains the shipping target.
- Spec URL: https://www.w3.org/TR/epub-33/ (+ /epub-33/ocf); EPUB 3.4 drafts at w3c.github.io/epub-specs.
- MIME: application/epub+zip (RFC 4839).
- Magic bytes: 50 4B 03 04 (PK zip) + first entry uncompressed `mimetype` = application/epub+zip (OCF).
- Licensing: W3C Royalty-Free policy. GREEN.
- Adoption: every ebook retailer/reader (Kindle KPF accepts EPUB ingestion, Apple Books, Kobo, Thorium/Readium), textbooks, technical publishing. Niche but real for harness use.
- Rust crates: epub-builder 0.8.2 (Nov 2025, MPL-2.0, active; targets EPUB 2.0.1 default / 3.0.1; acceptable since 3.3 was mostly conformance tightening); `epub` crate for reading (aging); official validator epubcheck is Java -> subprocess verification.
- Generation tier: pure-Rust YES (epub-builder; zip+XML DIY); subprocess YES (pandoc, LibreOffice, Calibre ebook-convert); WASM YES for building (zip+XML).
- Fidelity risks: remote resources forbidden by OCF (assets must be in-package); nav-doc + NCX completeness; media-type manifest correctness; malformed XHTML content docs (require well-formedness, not tag-soup).
- Round-trip hooks: epubcheck zero-error gate (Java, subprocess).

#### HTML5 detail

- Spec/body + version: WHATWG HTML Living Standard (no version numbers; W3C HTML5 spec retired 2019).
- Spec URL: https://html.spec.whatwg.org/multipage/
- MIME: text/html (RFC 2854). Magic bytes: none.
- Licensing: WHATWG royalty-free; zero-patent-drama history. GREEN.
- Adoption: the universal document interchange substrate; every renderer, editor, and converter speaks it.
- Rust crates: parse: html5ever (servo, maintained), html5gum (active), scraper; streaming rewrite: lol_html (Cloudflare, active); generate: maud / askama / horrorshow templates. All pure Rust.
- Generation tier: pure-Rust YES, subprocess YES, WASM YES for all three.
- Fidelity risks: as interchange it is lossy (no pagination, JS-dependence, external assets); not archival. Generate self-contained single-file HTML (inlined images/CSS) for the interchange lane; watch encoding declaration and relative-asset resolution.
- Round-trip hooks: reparse with html5ever/html5gum; check encoding declaration; keep the file self-contained.

#### DjVu detail

- Spec/body: no standards body; AT&T-origin spec, dormant (verified). Historic spec at djvulibre.djvuzone.org.
- MIME: image/vnd.djvu, image/vnd.djvu+multipage. Magic bytes: 41 54 26 54 46 4F 52 4D ("AT&TFORM").
- Licensing: DjVuLibre GPL reference implementation + patent grant; commercial rights shuffled AT&T -> LizardTech -> Celartem -> Cuminas/ePapyrus. YELLOW at best.
- Adoption: scanning/archival niche only; no browser support; no spec activity.
- Rust crates: essentially nothing maintained.
- Generation tier: pure-Rust NO; subprocess: DjVuLibre CLI read-only if an archival corpus forces it; WASM NO.
- Verdict for the plan: RED for generation, read-only optional. Effectively exclude.

#### UOF

Chinese GB national standard, Chinese-labeled XML vocabulary, regional-only adoption. SKIPPED per brief; EXCLUDE.

#### Toolchain hooks captured from this lane

- LibreOffice headless (`soffice --headless --convert-to X`), verified clean pairs: docx <-> odt <-> rtf <-> doc (legacy-read); xlsx <-> ods <-> csv (with `--infilter` dialect control, e.g. `CSV:59,34,76,1`); pptx <-> odp; any-of-them -> pdf; Writer docs -> epub (EPUB filter since LO 6.0); -> html / txt / png; flat-XML fodt. Ops musts: one `-env:UserInstallation=file:///tmp/lo_N` profile per concurrent process (second instance on a shared profile silently exits 1); batch many files per invocation (measured 5.96s vs 13.52s for 4 files); hard timeout (corrupt input hangs rather than errors); fonts must be OS-installed or layout silently shifts; complex Word layouts drift, diff-test. EPUB export understands character/paragraph styles only (frames/tables/page styles degrade).
- Pandoc role: universal tier-b pivot. Reads: CommonMark/GFM variants, AsciiDoc (input only), HTML, LaTeX (subset), DocBook, EPUB, DOCX, ODT, RST, Org, djot. Writes: DOCX, ODT, EPUB2/3, PPTX, PDF (via typst/LaTeX/weasyprint engines), HTML5, LaTeX/beamer, man. `--reference-doc` gives style control for docx/odt/pptx. Cannot: XLSX/spreadsheets (use rust_xlsxwriter/LO), DjVu, AsciiDoc output. License GPL: fine as sandboxed subprocess, not linkable.
- Verification sidecar tools: qpdf, mutool, pdftotext, veraPDF (PDF); epubcheck (EPUB, Java); Calibre ebook-convert (ebook conversions); asciidoctor (AsciiDoc).

### 3.2 Images / 2D-vector cluster (lane 2)

*Deep-research snapshot, Sept 2026. Headline: the safe core is PNG/APNG, SVG, JPEG-baseline, WebP, AVIF, with JPEG XL a certified momentum-reversal story (Chrome re-added it Feb 2026, Firefox 157 turns it on by default this month). Two corrections to the task brief: (1) JPEG XL is ISO/IEC 18181, not 18477 (18477 = JPEG XT); (2) the Google/Chrome drop was Feb 2023 (Chrome 110), not 2021, and it was reversed Nov 2025 to Feb 2026.*

#### Images verdict table

| Format | Spec body / version (year) | MIME | Magic bytes | Licensing verdict | Adoption | Rust feasibility | Gen tier |
|---|---|---|---|---|---|---|---|
| PNG + APNG | W3C Rec 3rd ed, 2025-06-24 (ex-ISO 15948:2004) | image/png, image/apng (new in ed-3) | 89 50 4E 47 0D 0A 1A 0A | GREEN | Universal | full pure-Rust R/W | Pure Rust |
| SVG 1.1 (+2 subset) | W3C SVG 1.1 2nd ed (2011); SVG 2 still CR, not Rec | image/svg+xml | none; sniff `<svg` / `<?xml` | GREEN (W3C RF) | Universal | usvg/resvg + templating | Pure Rust |
| JPEG baseline | ITU-T T.81 / ISO/IEC 10918-1 (1992) | image/jpeg | FF D8 FF | GREEN (Forgent patent dead 2006) | Universal | zune-jpeg + jpeg-encoder | Pure Rust |
| WebP | Google WebP spec (VP8 intra + RIFF; 1.x) | image/webp | 52 49 46 46 ?? ?? ?? ?? 57 45 42 50 (RIFF...WEBP) | GREEN (irrevocable VP8 grant + MPEG-LA 2013 deal + patents now ageing out) | Universal since 2020 | lossless R/W pure Rust; lossy encode = C or ffmpeg | Pure Rust (lossless) / subprocess (lossy) |
| AVIF | AOMedia AV1 + av1-avif profile on ISO/IEC 23008-12 (HEIF) over ISO/IEC 14496-12 (BMFF) | image/avif, image/avif-sequence | bytes 4-7 = ftyp, brand avif/avis | GREEN w/ YELLOW footnote (Sisvel pool claims; AOM defensive-termination clause) | ~94% browsers (all since Safari 16.4 / Mar 2023) | encode pure-Rust (ravif -> rav1e, slow); decode = C (dav1d) or ffmpeg | Pure Rust encode (slow) / subprocess decode-back |
| JPEG XL | ISO/IEC 18181-1..3:2022 (+libjxl ref software) | image/jxl | FF 0A (bare) or 00 00 00 0C 4A 58 4C 20 0D 0A 87 0A (container) | GREEN (JPEG Committee royalty-free commitments); adoption was the issue, now reversing | ~22% default today, rising fast | decode pure-Rust (jxl-oxide); encode = libjxl C++ or cjxl subprocess | Subprocess (encode) |
| TIFF / GeoTIFF | TIFF 6.0 (Aldus/Adobe 1993); GeoTIFF 1.1 (OGC 2019) | image/tiff | 49 49 2A 00 / 4D 4D 00 2A (BigTIFF 2B) | GREEN (LZW patent expired 2004) | Non-web universal (print/GIS/science) | tiff crate R/W (strips, deflate/LZW); GeoTIFF write -> GDAL | Pure Rust (plain) / subprocess (geo) |
| OpenEXR (ACES) | OpenEXR 3.x (Academy Software Foundation; ACES = SMPTE ST 2065-4) | image/x-exr (de facto) | 76 2F 31 01 ("v/1" + 01) | GREEN (BSD, ASWF) | VFX/HDR standard; zero browsers | exr crate pure-Rust R/W, good not perfect | Pure Rust |
| KTX2 | Khronos KTX v2 spec (registry.khronos.org; KTX-Software Apache-2.0; Basis Universal contributed RF by Binomial) | image/ktx2 | AB 4B 54 58 20 32 30 BB 0D 0A 1A 0A | GREEN | glTF / KHR_texture_basisu standard; zero browsers | ktx2 parse R/W; basisu crate transcode; ETC1S/UASTC encode -> toktx subprocess | Subprocess (encode) |
| DDS | Microsoft spec (MS Learn, free; BC modes fully documented) | image/vnd-ms.dds | 44 44 53 20 ("DDS ") | GREEN (S3TC patents expired ~2017; BC6H/BC7 docs open, MS never asserts; twilight note) | Game-tool standard; zero browsers | decode partial pure Rust; ddsfile + bcdec; BC7 encode = C | Subprocess (encode) |
| QOI | Szablewski 2022, frozen spec, MIT | image/qoi (de facto, unregistered) | 71 6F 69 66 ("qoif") | GREEN (public-domain-ish, MIT) | Niche (gamedev); zero browsers | trivial (image crate R/W) | Pure Rust |
| DNG | Adobe DNG spec 1.7 (2023) -> ISO 12234-4:2026 (published 2026-03-24) | image/x-adobe-dng | TIFF magic + DNGVersion tag | YELLOW (RF patent license with revocation clause; Adobe trademark on name) | Camera/archive niche; zero browsers | raw-mosaic DNGs are a color-science problem, not a writer problem | Skip (subprocess Adobe DNG Converter if ever) |
| Proprietary RAW (CR3/NEF/ARW/RAF/R3D...) | vendor-proprietary, undocumented | image/x-* | varies | RED. Out of scope, exactly like the H.264/HEVC example | (varies) | (n/a) | OUT |
| FLIF | dead | (n/a) | 46 4C 49 46 | (n/a) | Confirmed dead: official site/repo state dev stopped; superseded by FUIF -> JPEG XL | (n/a) | OUT |
| JPEG LS / XR / 2000 | ISO/IEC 14495 / 29199 / 15444 | (n/a) | (n/a) | LS/XR legally clean but irrelevant; JPEG 2000 patent-thicketed, dead | Effectively zero (medical LS niche) | (n/a) | OUT |
| XMP / EXIF sidecars | ISO 16684-1 / JEITA CP-3451 (Exif 3.0, 2023) | application/rdf+xml / APP1 | text `<?xpacket` / APP1 `Exif\0\0` | GREEN | Universal metadata layer | kamadak-exif (R/W); XMP = emit XML; ExifTool subprocess for exotic fields | Pure Rust + ExifTool escape hatch |

#### Patent rigor: the four load-bearing verdicts (images)

- PNG, GREEN, verified. The format exists precisely because GIF's LZW was patented; PNG mandates zlib/DEFLATE (RFC 1950/1951), LZW never applied. Third Edition went through the W3C Patent Policy (RF licensing commitments) on its way to Recommendation, 24 June 2025. No credible assertion ever made against PNG.
- WebP, GREEN, verified. WebP = VP8 intra-frame coding in a RIFF box. Chain of safety: (a) Google released the VP8 bitstream spec under an irrevocable royalty-free patent license (May 2010); (b) March 2013 MPEG-LA agreement: MPEG-LA dropped its VP8 pool formation and granted Google sublicensing rights over 11 patentees' "may be essential" patents for VP8/VP9 users; (c) Nokia's assertion lost in Mannheim (2013) and its 64 patents are long expired anyway; (d) by 2026 essentially all VP8-era patents (filed 2008 or earlier) have expired. libwebp itself is BSD; note its 2023 security history (CVE-2023-4863) as an operational, not licensing, reason to prefer Rust decoders.
- AVIF, GREEN with a named footnote. Two layers. Codec (AV1): AOMedia Media Patent License 1.0 is royalty-free but with defensive-termination (Mode 2-style) reciprocity, and Sisvel runs a separate AV1 pool claiming ~1,900 patents / 14 owners (since 2019). No implementer has ever been successfully forced to pay; essentiality is heavily disputed (independent assessments plus Unified Patents IPRs have knocked out chunks). A paper cloud, not an invoice; same class of risk Google already neutralized for VP8, but here unresolved by settlement. Container (the nuance): AVIF rides HEIF (ISO/IEC 23008-12) on ISOBMFF (ISO/IEC 14496-12). The ISO spec text is copyrighted/paywalled; that is copyright, not patents, and doesn't restrict implementations. The HEVC patent pools (Access Advance / MPEG-LA -> Via) license HEVC-in-HEIF files, which AVIF doesn't use; generic BMFF structural patents date from the early-2000s 14496-12 era and have expired (2001 + 20 => 2021+). Net: container does not bite.
- JPEG XL, legally GREEN; adoption story corrected. ISO/IEC 18181 (2022), open reference implementation (libjxl, BSD-style), JPEG Committee's standing royalty-free licensing policy, Google/Cloudinary/etc. contributor commitments; no pool has ever formed against it. The real question was adoption after the Chrome 110 removal (Feb 2023). Jpegli (Google 2023) is a JPEG codec that outputs baseline JPEG, correctly out of the format discussion.

#### JPEG XL adoption reality-check (timeline)

| Date | Event |
|---|---|
| 2021-2023 | Chrome 91-110 shipped JXL behind flag; removed entirely in Chrome 110 (Feb 2023) citing "ecosystem lacking interest" |
| Sep 2023 | Safari 17 ships JXL enabled by default (Apple breaks the deadlock); macOS/iOS 17+ |
| Jun 2025 | Photoshop adds AVIF + JXL (v26.x; JXL export confirmed in 26.8) |
| Nov 2025 | Google publicly reverses position, welcomes JXL back into Chromium |
| Feb 2026 | Chrome 145 ships JXL decoder behind `chrome://flags/#enable-jxl-image-format` (default OFF) |
| Jun 2026 | Firefox 152 ships JXL behind flag |
| End Sep 2026 | Firefox 157 ships JXL enabled by default on all platforms (Mozilla Intent to Ship, Aug 2026), i.e. this month |
| H2 2026 (expected) | Chrome default-on flip anticipated; Edge follows Chromium |

Today (Sep 21, 2026): default-on = Safari (~18-20%) + Firefox-imminent (~3%), about ~22% and climbing; Chrome's flag is shipped and the flip is a policy event, not an engineering event. ISO standard stable; Photoshop/Firefox/Safari/Krita/GIMP/ffmpeg/ImageMagick/libvips all read and write it. Verdict: no longer "fractured consortium", a recovering standard. Strategy: implement JXL decode now (pure Rust), route encode through libjxl subprocess, promote to first-class the day Chrome flips default.

#### Browser support matrix (Sept 2026)

| Format | Chrome/Edge | Firefox | Safari | Coverage |
|---|---|---|---|---|
| SVG | yes, all | yes | yes | ~100% |
| PNG / APNG | yes / yes | yes / yes | yes / yes (14+, 2020) | ~100% |
| JPEG | yes | yes | yes | ~100% |
| WebP | yes (32+, 2014 era onward) | yes (65+) | yes (14+, 2020) | ~98% |
| AVIF | yes (85+, 2020) | yes (93+, 2021) | yes (16.4, Mar 2023) | ~94% |
| JXL | flag (145+), default pending | 152 flag / 157 default (Sept 2026) | yes, default (17+, 2023) | ~22% default, rising |
| TIFF/EXR/KTX2/DDS/QOI/DNG | no, all | no | no | 0% (OS-level: Windows/macOS preview cover AVIF+HEIC, not the rest) |

#### Tooling support matrix, read+write unless noted (Sept 2026)

| Tool | PNG/APNG | JPEG | WebP | AVIF | JXL | TIFF | EXR | QOI | DDS | KTX2 | SVG |
|---|---|---|---|---|---|---|---|---|---|---|---|
| ffmpeg (7.x/8.x) | yes/yes | yes/yes | yes/yes (anim) | yes/yes (libaom/svt/rav1e) | yes/yes (libjxl; native decoder since 7.1) | yes/yes | yes/yes (OpenEXR) | yes/yes (native since 5.1) | decode only | no | no (no rasterizer) |
| ImageMagick 7 | yes | yes | yes | yes (libheif) | yes (libjxl delegate; common in 2025+ distro builds) | yes | yes | yes (since 7.1.0-22, 2022) | yes | no | rasterize only (rsvg/delegate-dependent) |
| libvips (8.1x) | yes | yes | yes | yes (heifload) | yes (jxlload since 8.15, libjxl) | yes | yes | not core (via magick fallback) | no | no | no |
| Blender (4.x) | yes | yes | yes (3.0+) | no (open feature request) | no | yes | yes (native, incl. multilayer) | no | no | no | import/rasterize only |
| Photoshop (26.x) | yes | yes | yes (23.2+, 2022) | yes (2025) | yes (2025, v26.8 export) | yes | decode only (old filter; weak) | no | no | no | decode; no real SVG export |
| GIMP 3 | yes | yes (12/16-bit via libjpeg-turbo 3+) | yes | yes (libheif) | yes (libjxl) | yes | yes | no (core) | decode (addon) | no | decode; export limited |
| Krita 5.x | yes | yes | yes | yes | yes (incl. animation export) | yes | yes (HDR-strong) | no | no | no | decode |
| Inkscape 1.4 | yes | yes (1.2+) | yes (1.2+) | no | no | yes (1.2+) | no | no | no | no | native format (best-in-class editor) |
| Figma | yes | yes | no native (plugins only, confirmed 2026) | no | no | no | no | no | no | no | yes export (SVG 1.1-level) |

Notable: Figma's export menu remains PNG/JPG/SVG/PDF only; for a design-agent harness, SVG+PNG remain the only Figma-native deliverables.

#### Rust crate layer (MSVC build context)

| Need | Crate | Notes |
|---|---|---|
| PNG/APNG | `png` (image-rs), `image` | Pure Rust. APNG encode+decode supported by the png crate (`set_animated`, frame iteration). 16-bit, iCCP, eXIf chunk (PNG-3) writable. `fdeflate` makes it fast. |
| SVG parse/validate | `usvg` | Strict parse -> structural validation hook. Text/layout subset caveats below. |
| SVG render | `resvg` (+ `tiny-skia`) | Pure Rust render-to-PNG for visual regression. resvg 0.44+ (2025); SVG 2 support partial (most of SVG 1.1 + some SVG 2 features). |
| SVG generate | templating: `maud` / `askama` / string building + quick-xml | Generation-by-templating is the right model. |
| JPEG | `zune-jpeg` (decode, used by `image`), `jpeg-encoder` (encode: quality, progressive, 12-bit, app markers) | Pure Rust, complete. Jpegli is C++ and unnecessary here. |
| WebP | `image` / `image-webp` (pure Rust: decode lossy+lossless; encode lossless only); `webp` crate = C libwebp binding for lossy encode | Lossy encode -> ffmpeg subprocess is the low-friction MSVC path. |
| AVIF encode | `ravif` (-> `rav1e`) | Pure Rust encoder; slow (tens of seconds per image at speed defaults). rav1e's default `asm` feature wants nasm on x86; install nasm or `default-features = false` (2-4x slower). Builds clean on MSVC. |
| AVIF decode | `image` + `avif-native` (dav1d C + mp4parse) | C toolchain (meson+nasm) is MSVC-hostile; prefer ffmpeg subprocess for decode-back verification. No mature pure-Rust AV1 decoder exists. |
| JXL decode | `jxl-oxide` (pure Rust, spec-conforming, has image-crate integration; optional lcms2/moxcms for ICC) | Also `zenjxl-decoder` (hardened jxl-rs fork, 100% on codec corpus Dec 2025). Healthy decode ecosystem. |
| JXL encode | `libjxl` FFI (C++) or `cjxl` subprocess | No mature pure-Rust encoder. Subprocess tier. |
| TIFF | `tiff` (image-rs) | Pure Rust R/W; write = strips + deflate/LZW; BigTIFF read only; tiled TIFF not written; float32 write OK. |
| GeoTIFF | `geotiff` (read-only), `gdal` crate (C bindings) | GeoTIFF write -> GDAL subprocess (gdal_translate / gdal_edit for GeoKeys). |
| EXR | `exr` | Pure Rust R/W; strong on common lossless compressions/multilayer (RLE/ZIP/PIZ; some lossy modes may be unsupported); used by image crate behind `exr` feature. ACES container = EXR + specific attributes/primaries; constructible. |
| KTX2 | `ktx2` (container parse; 385k downloads/mo, wgpu/bevy ecosystem), `ktx2-rw` (R/W + Basis), `basisu` (pure-Rust transcoder, no_std-capable) | ETC1S/UASTC encode -> `toktx` subprocess (KTX-Software, Apache-2.0) is the battle-tested path. |
| DDS | `image` (decode: BC1-3 pure Rust; newer BC modes via `bcdec` C), `ddsfile` (headers) | BC7 encode: `bc7enc` (C single-file, Rich Geldreich) or texconv/toktx subprocess. |
| QOI | `image`, `qoi`, `fast_qoi` | Trivially implementable; ~300 LOC if ever needed inline. |
| EXIF/XMP | `kamadak-exif` (R/W); XMP = hand-emitted XML or `xmp-toolkit` (Adobe C++ SDK FFI); `exiftool` subprocess for exotic round-trips | ExifTool is the universal oracle for verification. |

MSVC summary: full pure-Rust pipeline achievable for PNG/APNG/JPEG/lossless-WebP/TIFF/EXR/QOI/SVG(+render). C/C++ enters only for lossy WebP (libwebp), AVIF decode (dav1d; avoid, use ffmpeg), JXL encode (libjxl; prefer cjxl subprocess), KTX2 Basis encode (toktx), BC7 (bc7enc/texconv). vcpkg packages exist for libwebp/libjxl/openexr when FFI is unavoidable.

#### Generation tiers + round-trip hooks (images)

| Format | Tier | Round-trip hook |
|---|---|---|
| PNG/APNG | pure Rust | decode-back via `png`; assert dimensions, bit depth, frame count; byte/pixel hash on lossless. |
| JPEG | pure Rust | decode via `zune-jpeg`; assert dims + PSNR/SSIM floor (lossy) rather than hash. |
| SVG | pure Rust (templating) | two-stage: (1) structural: parse with `usvg` (must accept), then canonicalized XML tree diff via quick-xml events (attribute-order-insensitive); (2) visual: `resvg` render -> PNG -> dimension + perceptual hash vs golden. |
| WebP | pure Rust (lossless) / ffmpeg subprocess (lossy+animated) | decode-back via `image-webp` (handles both); dims + hash (lossless) or SSIM (lossy); for animation assert frame count + durations. |
| AVIF | pure Rust encode (ravif), decode-back via ffmpeg | ffmpeg `-i out.avif -f null -` (validity) + decode -> dims/SSIM; keep an ffmpeg conformance smoke in CI since dav1d-on-MSVC is the pain point. |
| JXL | subprocess (cjxl/djxl) + `jxl-oxide` decode in-process | decode-back with `jxl-oxide`; dims + SSIM; also `djxl` cross-check vs reference. |
| TIFF | pure Rust; GeoTIFF via gdal_translate subprocess | `tiff` decode-back; dims/tag assertions (for Geo: `gdalinfo` assert on CRS GeoKeys). |
| EXR | pure Rust (`exr`) | decode-back via `exr`; dims, channel set, compression header assertions. |
| KTX2 | toktx subprocess | `ktx2` crate parse; assert VkFormat, mip count, level sizes; transcode smoke via `basisu` crate. |
| DDS | subprocess (texconv/toktx) or bc7enc | `ddsfile` header assertions + `bcdec` decode -> dims/hash. |
| QOI | pure Rust | trivial decode-back + hash. |

Fidelity risks to engineer around (images):

- Color: embed iCCP (PNG) / correct nclx or ICC (AVIF/JXL) or assume sRGB explicitly; rav1e/ravif does RGB<->YUV, pin full/limited range and matrix; 16-bit PNG pipelines must not silently quantize; EXR is linear scRGB by convention (don't gamma-encode).
- EXIF/orientation: lossless transcodes must carry the orientation flag through (kamadak-exif R/W); PNG-3's eXIf chunk vs JPEG APP1 asymmetry; XMP sidecar emission for RAW-adjacent flows.
- Animation: APNG (acTL/fcTL/fdAT ordering, default-image fallback frame), animated WebP (ANMF disposal/blend), AVIF sequences (avis brand + item tracks), JXL animations. Each has its own timing/disposal model; verify frame count + durations, not just first frame.
- SVG text: font availability is the #1 render divergence. For generated SVG deliverables, either restrict to common font stacks or convert text to paths; filter/blur fidelity differs across renderers (usvg supports the common subset); test CSS-in-SVG since SVG2-style `style` attributes are where generators most often emit invalid markup.

#### Images TOP-5 (adoption x licensing-safety x Rust feasibility)

1. PNG / APNG. ~100% adoption; W3C Recommendation (2025) with RF patent policy; flawless pure-Rust R/W incl. animation, 16-bit, ICC, EXIF. The harness's lossless workhorse and default round-trip target.
2. SVG (1.1 baseline + SVG 2 features where usvg supports). ~100% adoption; the only editable/vector-native format with universal tooling (Figma exports it, Inkscape native); W3C RF; pure-Rust generate+validate+render loop (templating -> usvg -> resvg). SVG 2 is still a Candidate Recommendation: target 1.1 semantics, cherry-pick SVG 2.
3. JPEG (baseline + progressive). ~100% adoption; patent era definitively over; complete pure-Rust R/W with fast decode; the lossy interchange format everything reads.
4. WebP. ~98% adoption; cleanest licensing paper trail of any modern codec (irrevocable grant + MPEG-LA sublicense + expiry); pure-Rust for decode + lossless encode, ffmpeg only for lossy/animated encode.
5. AVIF. ~94% adoption; AOMedia royalty-free with the Sisvel footnote and ISO-container nuance documented above; pure-Rust encode path exists (ravif, accept the speed), decode-back via ffmpeg.

Number 6, JPEG XL (promote-now trigger): pure-Rust decode ready (jxl-oxide), cleanest licensing of the new generation, and adoption has visibly reversed (Safari default since 2023, Firefox 157 default Sept 2026, Chrome 145 shipped-behind-flag with default flip expected H2 2026, Photoshop 26.8 export). Implement decode + cjxl-subprocess encode today; move to top-tier when Chrome flips default.

#### Images landmine paragraph (quotable)

The encumbered minefield lives almost entirely in video-codec-adjacent formats: HEIC/HEIF-with-HEVC and anything H.264/H.265-based (Access Advance/MPEG-LA/Via pools, real invoices, hard RED), JPEG 2000 (patent-thicketed corpse), and proprietary camera RAWs (undocumented, vendor-controlled). Even DNG, though now ISO 12234-4:2026 and royalty-free on paper, carries Adobe's defensive-termination revocation clause and trademark control; treat as YELLOW. The open web stack is clean by construction: PNG was designed around the LZW patents and is a 2025 W3C Recommendation under RF patent policy; baseline JPEG's one assert (Forgent) died in 2006; WebP rests on Google's irrevocable VP8 grant plus the 2013 MPEG-LA sublicensing settlement, and the underlying patents are now largely expired; JPEG XL is an open ISO standard (ISO/IEC 18181, not 18477 which is JPEG XT) under the JPEG Committee's royalty-free regime whose only wound, Chrome's 2023 removal, was self-inflicted and has since healed (reinstated 2026). The one genuinely gray zone is AVIF: the AV1 codec is covered by AOMedia's royalty-free license, but that license terminates defensively if you assert patents, and Sisvel's separate AV1 pool claims ~1,900 patents it says are essential; nobody has ever been made to pay and essentiality is widely disputed, so AVIF stays GREEN-with-footnote, not RED. The HEIF/BMFF container itself is a non-issue: ISO copyright on spec text is not a patent on implementations, and the early-2000s BMFF structural patents have expired. Net: build on PNG/SVG/JPEG/WebP/AVIF + JXL-as-fast-follow; keep HEVC-flavored anything, JPEG 2000, and proprietary RAW out.

### 3.3 3D / CAD cluster (lane 3)

*Deep-research snapshot, Sept 2026. Scope: open, industry-standard 3D/geometry/CAD/animation formats the harness must parse + generate; flagship = LLM-authored .blend / .glb / .step. Verified against Sept 2026 sources (specs, registries, crates.io/GitHub metadata, vendor docs).*

#### 3D/CAD verdict table

| Format | Body / spec | Licensing verdict | Adoption today | Rust read | Rust write | Generation tier |
|---|---|---|---|---|---|---|
| glTF 2.0 / GLB | Khronos 2.0 (2017; 2.0.1 rev. 2021); ISO/IEC 12141:2022 | GREEN (royalty-free, Khronos IP framework) | THE web/AR/realtime interchange | YES: `gltf` 1.4.1 | YES: `gltf-json` + `Glb::to_writer` (low-level) | PURE RUST |
| USD / USDZ | Pixar OpenUSD (2016 onward); AOUSD since 2023; ISO path | GREEN (TomorrowOpen = Apache-2.0 mod. Section 6 only; neutral LF/JDF governance) | Film/VFX backbone, Apple AR/Vision Pro, Omniverse | YES: `openusd` 0.7.0 (pure Rust, new) | YES, same (usda/usdc/usdz; pre-1.0 caveat) | PURE RUST (new) / subprocess fallback |
| 3MF | 3MF Consortium (2015 onward), core 1.x | GREEN (specs free; member patent claims RF) | THE 3D-printing container (Prusa/Bambu/MS/SolidWorks MBD) | YES: `lib3mf`, `threemf` | YES: `lib3mf` (pure Rust, conformance-tested) | PURE RUST |
| STL | none (de-facto, 1987 3D Systems) | GREEN (no owner; 80s patents expired) | Universal lowest common denominator | YES: `stl_io` 0.11.0 (2026-03) | YES, trivial | PURE RUST (trivial) |
| OBJ + MTL | none (Wavefront vendor docs, ~1989) | GREEN (de-facto public domain; spec vacuum) | Universal static-mesh exchange | YES: `tobj` 4.0.5 (active) | YES: `wavefront-obj-io`, `obj` (stale) | PURE RUST |
| STEP AP203/214/242 | ISO 10303; AP242 Ed3 2022, Ed4 in development | YELLOW (spec ISO-paywalled; usage unrestricted, free schema mirrors) | THE CAD interchange (SolidWorks/Fusion/Onshape/CATIA/NX; aerospace-mandated) | YES: `step-p21` / `stepperoni` / `ruststep` (deserialize) | CAUTION: kernel-scoped only (`truck-stepio`, `brepkit-io` AGPL) | SUBPROCESS (FreeCAD/OCCT) |
| IGES | US PRO, 5.3 (1996) last | RED (deprecated, superseded by STEP) | Legacy importers everywhere; no new content | niche | none | DROP |
| BLEND | none formal; source-as-spec (DNA/SDNA) | YELLOW (GPL tool; format readable but no stability contract) | Blender-native; huge community | niche parsers | NO: never hand-write | SUBPROCESS flagship (bpy headless) |
| COLLADA | Khronos 1.5.1 (2008, last) | YELLOW (open but dormant; Blender marked I/O legacy 2024) | Legacy | YES: `collada`, `dae-parser` | NO maintained writer | DROP for generation |
| FBX | Autodesk proprietary (free closed SDK) | RED (proprietary; no spec, no open write path) | Ubiquitous legacy DCC/game interchange | YES: `ufbx` (official bindings, MIT/Unlicense), `fbxcel` | NO | READ-ONLY |
| Alembic .abc | Open (SPI/ILM 2011), modified-Apache | GREEN (license) / YELLOW overall | VFX geometry-cache standard | NO (nothing viable) | NO | SUBPROCESS |
| OpenVDB / nanoVDB | ASWF; v13 (2026) now Apache-2.0 (ex-MPL-2.0) | GREEN | Volumetrics: Houdini/Arnold/Blender/USD | partial, read-only (`vdb-rs`, `nanovdb-rs`) | NO | SUBPROCESS |
| Draco (transform) | Google; Apache-2.0; KHR_draco_mesh_compression (ratified) | GREEN | glTF delivery compression | YES: `draco-oxide` (pure Rust enc+dec, alpha) | YES, same | PURE RUST (alpha) |
| meshopt (transform) | zeux; MIT; EXT_meshopt_compression (ratified) | GREEN | The modern glTF codec (gltfpack) | YES: `meshopt` crate (FFI, maintained) | YES (encode) | BINDINGS |
| MaterialX | ASWF; Apache-2.0; v1.39.x | GREEN | Rising via USD/UsdMtlx, Omniverse, renderers; Blender GSoC-2026 work | NO | NO | SUBPROCESS / via USD |
| KTX2/BasisU (texture sidecar) | Khronos; KHR_texture_basisu (ratified) | GREEN | Standard glTF texture path | crates exist (out of lane scope) | (n/a) | PURE RUST (texture lane) |

#### glTF 2.0 + .glb detail (GREEN, top target)

- Spec: Khronos glTF 2.0 (2017; current 2.0.1-era registry text, 2021), https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html. Transposed to ISO/IEC 12141:2022 (Aug 2022) via JTC1 PAS; Khronos retains control.
- MIME: model/gltf+json (.gltf JSON scene + external .bin buffers + textures), model/gltf-binary (.glb).
- Magic/layout: GLB 12-byte header = magic "glTF" (0x676C5466) + version (2) + total length; then JSON chunk type 0x4E4F534A ("JSON"), padded to 4 bytes with 0x20; optional BIN chunk type 0x004E4942 ("BIN\\0"). Buffers little-endian; SHOULD stay <= 2^53 bytes.
- Extensions (extension-first evolution; no ratified 3.0 as of 2026): compression = KHR_draco_mesh_compression (ratified; Draco codec Apache-2.0), EXT_meshopt_compression (ratified; MIT codec, the modern default, GPU-friendly, gltfpack toolchain), KHR_texture_basisu (KTX2/Basis supercompressed textures). 2024-2026 wave: KHR_materials_dispersion (ratified Apr 2024), physics/interactivity/audio/spatial extensions at review-draft stage, 3D Gaussian-splat extensions added Sept 2025 (Khronos + OGC/Niantic/Cesium/Esri), MaterialX-gltf PBR node mapping maintained by the WG.
- Licensing/patent: GREEN. Khronos royalty-free spec + IP framework (contributors must license essential claims RF). The historical "3D compression patent thicket" (Deering/Sun geometry-compression era) is expired; active codecs carry clean licenses (meshopt MIT, Draco Apache-2.0, KTX2 Khronos RF). Cleanest licensing story in 3D.
- Adoption 2026 (verified): three.js GLTFLoader + model-viewer (GLB) = default web path; Unity 6 ships an official glTF package (glTFast-derived, plus the UniGLTF ecosystem); Unreal imports/exports glTF via Interchange (UE 5.4+ unifies FBX/glTF/OBJ); Godot 4 native glTF 2.0 import/export, used as the editor's own interchange; Blender official glTF I/O; Onshape exports GLTF/GLB natively; Windows/macOS Quick Look, AR on Android. The single most-adopted open 3D delivery format ("JPEG of 3D").
- Rust: read: `gltf` v1.4.1 (MIT/Apache-2.0, active 2026), standard loader; `easy-gltf`, `bevy_gltf` (load-only) on top. Write: the `gltf` crate is import-only by design (features import/utils/names/extensions/extras; no export feature). Generation is still production-viable pure Rust: `gltf-json` types derive Serialize and `glb::Glb` exposes to_writer/to_vec (verified in source, binary.rs L177-231); author `gltf_json::Root` + pack buffer -> GLB. No maintained high-level exporter crate (`gltf-writer` does not exist; 2026 newcomers `draco-gltf` "load and save full glTF 2.0 scenes", `awsm-renderer-glb-export`, `vcad-kernel-export` fill the gap). Verification ally: the official Khronos glTF Validator is written in Rust (KhronosGroup/glTF-Validator); directly usable in a verify hook.
- Generation tier: PURE RUST (low-level but spec-stable and small; JSON+bytes is exactly what an LLM emits happily).
- Round-trip hook: re-parse with `gltf` crate (node/mesh/accessor counts, min/max) + run glTF-Validator; structural diff, not byte diff.

#### USD + USDZ detail (GREEN, governance normalized)

- Spec: Pixar's Universal Scene Description, open-sourced 2016. Physical forms: .usda (text, header line "#usda 1.0"), .usdc (binary crate, magic "PXR-USDC"), .usdz (Apple, 2018: unencrypted, uncompressed ZIP with 64-byte file alignment; MIME model/usdz, alongside model/usda; IANA/Apple registered). Composition engine (LIVERPS, variants, payloads, instancing) is the core value.
- Governance (verified): Alliance for OpenUSD (AOUSD) announced Aug 1, 2023 by Pixar, Adobe, Apple, Autodesk, NVIDIA under the Linux Foundation's Joint Development Foundation (chair Steve May, Pixar); purpose = written OpenUSD specs + an explicit path to ISO recognition; liaisons with Khronos (glTF alignment), ASWF (MaterialX), buildingSMART (IFC), AOMedia, AMT. License: the old "Modified Apache 2.0" was renamed TomorrowOpen Source Technology License 1.0 (May 2024); verified from the repo LICENSE: it differs from Apache-2.0 only in Section 6 (Trademarks); full copyright+patent grants retained. Licensing verdict GREEN (permissive, patent-granted; footnote: not OSI-vanilla Apache).
- Adoption 2026: film/VFX pipeline backbone (Maya, Houdini, Katana, Nuke, RenderMan/Arnold/Karma via Hydra); Apple's entire AR stack (AR Quick Look on iOS/macOS/tvOS, RealityKit, Vision Pro; USDZ is the only AR delivery format there); NVIDIA Omniverse; Autodesk+Adobe product lines. Blender: USD import/export in core since 2.82; 4.x bundles USD + Hydra delegates. Game engines: Unreal strong (USD importer + Omniverse live link); Unity's USD package remains experimental/stale; Godot limited import. Traditional mechanical CAD: essentially absent. Toolchain: usdcat / usdview / usdchecker / usdzip from the OpenUSD distro; Python pxr.Usd authoring.
- Rust (reality check flipped in 2025/2026): `openusd` v0.7.0 (2026-09-05, MIT, mxpv/openusd): pure Rust, no C++ dependencies, read AND write .usda/.usdc/.usdz, full composition (LIVERPS, variants, payloads, instancing), Stage authoring API, typed schemas via `openusd-schemas` (UsdGeom/UsdShade/UsdSkel/UsdPhysics), tested against AOUSD Core Spec baselines, CI on Linux/macOS/Windows, actively pushed (2026-09-20). Caveat: pre-1.0, APIs may change. Fallbacks: `usd`/usd-rs v0.0.9 (2020, dead placeholder); `pxr_sys`/`pxr_build` v0.2.0 (2024, C++ OpenUSD bindings, heavy build, semi-dormant); `openusd-rs` v0.1.0 (2025-07, stalled WIP).
- Generation tier: PURE RUST (new, pre-1.0 caveat) with subprocess fallback (usdcat / Python pxr, or Blender's USD exporter).
- Round-trip hook: `usdcat --flatten` diff / `usdchecker`, or structural re-read via `openusd`.

#### 3MF detail (GREEN, easy win)

- Spec: 3MF Consortium (founded 2015 by Microsoft, HP, Autodesk, Dassault et al.); core spec + extensions (materials, production, beam lattices). https://3mf.io : "All specifications are freely available. 3MF members have agreed to make necessary patent claims available on a royalty-free basis." Container: OPC/ZIP ([Content_Types].xml, _rels/.rels, 3D/3dmodel.model XML); MIME model/3mf (IANA-registered). Reference C++ lib3mf is open source. LoC sustainability entry (2025): "No known license or patent concerns."
- Adoption: THE printing interchange; Prusa/Bambu/UltiMaker ecosystems, Windows 3D Builder, Fusion (import), SolidWorks MBD 3MF; carries units, transforms, multiple objects, materials, everything STL lacks. Verdict vs STL: 3MF is what STL should have been; STL persists as the compatibility floor.
- Rust: `lib3mf` v0.1.6 (2026-03, MIT, telecos/lib3mf_rust): pure Rust, `#![forbid(unsafe_code)]`, read AND write, round-trip tested, conformance-tested against the official 3MF test suites (~2,200 cases, ~97.6%); caveat: self-described AI-assisted codebase; fine for a harness, pin the version. `threemf` v0.8.0 (2026-03, 0BSD, Fornjot lineage) = solid read-only alternative. No C++ lib3mf bindings on crates.io (and none needed).
- Generation tier: PURE RUST (also trivially hand-rollable: zip + XML).
- Round-trip hook: re-parse + unzip XML diff.

#### STL detail (GREEN, trivial)

- Spec: none; de-facto from 3D Systems (1987, Albert Consulting). Binary: 80-byte header (may legally start with "solid"; the classic parser trap: sniff structure, not the first word) + uint32 triangle count + 50 bytes per triangle (normal + 3 vertices as f32 LE + uint16 attribute byte count). ASCII: "solid ... / facet normal / outer loop / vertex ...". MIME: model/stl (IANA-listed; historically application/sla). Unitless (convention mm); no normals guarantees, no colors, no scene, no instancing.
- Licensing: GREEN. No owner, no live patents (everything dates to the 80s). Verdict: dated but universal; keep as the lowest-common-denominator emit target and verify fixture, not as a primary authoring format.
- Rust: `stl_io` v0.11.0 (2026-03, MIT): pure Rust read+write, ASCII+binary, active. (`stl` v0.2.1 2021 read+write stale; `nom-stl` parser stale; there is no `mesh-io` crate on crates.io.)
- Generation tier: PURE RUST, trivial.
- Round-trip hook: re-parse -> triangle count + vertex-normal hash.

#### OBJ + MTL detail (GREEN)

- Spec: none formal; Wavefront vendor manuals (late 1980s). .obj: v/vt/vn/f/o/g/s/usemtl/mtllib; 1-based indices with negative (relative) indexing; polygon faces (triangulate on export). .mtl: newmtl, Ka/Kd/Ks/Ns/d/illum + map_Kd etc. PBR sidecars are de-facto extensions, not standard: Pm/Pr (metallic/roughness, written by Blender's exporter and read by Sketchfab-era tooling), map_Pm/map_Pr, norm (normal map), Ke/map_Ke. An LLM should emit them as optional extras, never as ground truth.
- Licensing: GREEN (de-facto public domain; spec vacuum is the only wart). Adoption: universal static-mesh exchange in every DCC/engine/scanner toolchain; no units, no scene graph, no animation; geometry-only lane.
- Rust: read via `tobj` v4.0.5 (2026-08, MIT, very active; loader only); write via `wavefront-obj-io` v0.3.1 (2026-09, streaming reader+writer); `obj` v0.10.2 (kvark, "importer and exporter" but 2020-stale); `obj-exporter` (2018, stale). MTL writing is the weak spot: no maintained dedicated MTL writer; hand-roll (~50 lines of serde/text) or use the stale crates.
- Generation tier: PURE RUST.
- Round-trip hook: re-parse (vertex/face counts, mtl reference set).

#### STEP (ISO 10303) detail (YELLOW on access, subprocess for generation)

- Spec: ISO 10303; geometry lives in B-rep (exact NURBS surfaces + topology), PMI/GD&T in AP242. Lineage: AP203 (1994, config-controlled design) -> AP214 (automotive, colors) -> AP242 "Managed Model Based 3D Engineering": Ed1 2014 (supersedes 203/214), Ed2 = ISO 10303-242:2020, Ed3 = ISO 10303-242:2022 (with STEP Module/Resource Library SMRL v9), Ed4 in development (prostep ivip/VDA roadmap: assembly PMI, PMI visual issue management, further MBD extensions).
- Physical file: ISO 10303-21 clear text; starts "ISO-10303-21;"; FILE_SCHEMA names the AP (e.g. 'AUTOMOTIVE_DESIGN { 1 0 103 214 1 1 1 1 }' or 'AP242_MANAGED_MODEL_BASED_3D_ENGINEERING_MIM_LF { 1 2 103 242 2 1 }'). Schemas defined in EXPRESS (ISO 10303-11).
- Licensing: YELLOW, the "open format purist" caveat. The normative spec text is ISO-copyrighted and paywalled (ISO store, per-part CHF; the full AP242 suite across SMRL is real money). In practice: implementations are unrestricted, no patent encumbrance is asserted, and the EXPRESS schemas circulate in free mirrors (STEPcode repo, CAx Implementor Forum). Everything needed to interoperate is obtainable free; the standard document itself is not. That asymmetry = YELLOW, not RED.
- Adoption 2026 (verified per-vendor): SolidWorks (AP203/214/242 export; dedicated PublishSTEP242File API for MBD/PMI), Fusion (AP203/214/242, semantic PMI post-2019), Onshape (import and export of AP203/AP214/AP242 + IGES 5.3, Parasolid, JT; glTF/GLB/OBJ/3MF/STL exports in the web UI), CATIA/NX/Creo universal; aerospace/automotive supply chains mandate AP242 (Airbus/Safran-class field reports, >95% partner acceptance). FreeCAD/OpenCASCADE reality: full-fidelity STEP is an OCCT affair; FreeCAD reads/writes STEP natively (write defaults AP214-flavored; AP203/AP214 schema selection; AP242 read solid, PMI handling partial). Blender has no STEP support at all (no B-rep kernel).
- Rust (verified; answers "is pure-Rust STEP viable?"): parsers, all fresh 2026: `step-p21` (virtualritz, ISO-10303-21 parser + serde entity tables), `stepperoni` (vcad-io, fast parser), `step21` (streaming scanner), `openbim-step` (AGPL). `ruststep` v0.4.0 (ricosjp, 2024-09): EXPRESS compiler + deserialize only; serialization still a TODO (issue #13); README banner "still in experimental stage. DO NOT USE FOR PRODUCT"; dormant since 2025-03. Write exists only inside Rust B-rep kernels: `truck-stepio` v0.3.0 (ricosjp/truck, 1.5k stars, active) reads/writes STEP from/to truck's own geometry subset; `monstertruck` (active hard fork) adds STEP assembly output; `brepkit-io` (2026, very active) has stable analytic-preserving STEP import/export but is AGPL-3.0-or-commercial since v3 (was MIT/Apache <= v2.129). OCCT bindings: `opencascade-rs` (bschwind, active 2026-08, LGPL-2.1 via OCCT, git-only, heavy C++ build); `cadmium` (WASM OCCT): repo deleted, project dead.
- Verdict: general-purpose, PMI-complete STEP generation is subprocess/WASM-only. `freecadcmd` (FreeCAD headless, OCCT-backed) is the pragmatic engine; pure-Rust write is viable solely for kernel-scoped B-rep via truck/monstertruck (permissive) or brepkit (license-gated).
- Generation tier: SUBPROCESS.
- Round-trip hook: re-read with FreeCAD headless; compare topology counts (solids/faces/edges/vertices, volume/bbox to tolerance); strip/ignore the FILE_NAME header timestamp (OCCT stamps wall-clock time, so byte-identical diffs are impossible by design).
- FreeCAD headless (verified): two executables per install: GUI `FreeCAD` and headless `FreeCADCmd`/`freecadcmd`; run `freecadcmd script.py` (or `-c "import Part; ..."`), same Python modules minus GUI. Export: `import Part; Part.makeBox(...).exportStep(path)` or `Import.export([obj], path)`. The standard CI/automation pattern (official wiki "Headless FreeCAD"); FreeCAD 1.0.x current.

#### IGES detail (RED, deprecated, confirmed)

US PRO / ANSI Y14.26M lineage; 5.3 (1996) was the final revision; universally deprecated in favor of STEP. Every CAD tool still imports it (Onshape lists "IGES 5.3"; colors are lost per vendor docs, the tell of its age). No patent issues; RED purely on obsolescence. Harness: read-only tolerance at most; never generate. No Rust crate of consequence.

#### BLEND detail (YELLOW; subprocess generation is THE pattern)

- Spec/format: no formal specification; source-as-spec. Self-describing DNA/SDNA block structure (each file embeds its own struct catalog), documented only via Blender's wiki/dev docs and community reverse-engineering ("The mystery of the blend"). Header (12 bytes): magic "BLENDER" + pointer-size char ('_' = 32-bit, '-' = 64-bit) + endianness char ('v' = little, 'V' = big) + 3-char version = major*100+minor (e.g. 405 = 4.5, 500 = 5.0). Optional Zstd compression of blocks.
- Stability contract: none. Blender reads older files (DNA forward-compat), but newer files are unreadable by older Blender and the internal layout shifts every release. Combined with the GPL tool (the format itself is not GPL-encumbered; file formats aren't copyrightable, and independent readers exist) -> YELLOW: documented-enough to read, absolutely wrong to hand-write.
- The rule: never hand-write binary .blend. DNA serialization, version-dependent struct catalogs, block ordering, and pointer fixups make LLM-authored bytes undiagnosable garbage. Generate via Blender's own serializer. (Rust readers exist only as niche experiments; irrelevant to this lane.)
- Generation path (flagship, verified): `blender --background --factory-startup --python gen.py -- <argv>` (full binary, any platform); or `pip install bpy` (official wheel since Blender 3.4, late 2022; current bpy 5.2.2, 2026-09-15, CPython 3.13 only; wheels for manylinux_2_28 x86_64, win_amd64, win_arm64, macosx_11_0_arm64; gaps: Linux ARM, macOS Intel; old versions purged from PyPI, archived at download.blender.org/pypi). Blender line today: 4.5 LTS (Jul 2025) + 5.x (5.0 Nov 2025 onward).
- bpy API surface for LLM authoring: prefer `bpy.data` direct construction over `bpy.ops` (ops are context/modal-dependent): `bpy.data.meshes.new()` + `mesh.from_pydata(vertices, edges, faces)` + `mesh.validate()`; `bpy.data.objects.new()` + `collection.objects.link()`; materials via Principled BSDF node graph; animation via keyframe_insert / f-curves; transforms via obj.location/rotation_euler/scale. Use `bpy.ops.*` only for I/O: `bpy.ops.wm.save_as_mainfile(filepath, compress=True)`, `bpy.ops.export_scene.gltf(...)`, `bpy.ops.wm.obj_export`, `bpy.ops.wm.usd_export`, `bpy.ops.export_mesh.stl`, Alembic/FBX exporters. Blender headless is a universal geometry factory: one subprocess emits .blend and re-emits the same scene as glTF/USD/OBJ/STL/PLY/Alembic/FBX; a built-in cross-format verification path. (Blender also exports FBX, a pragmatic escape hatch even though FBX is RED open-wise.)
- Determinism: `--factory-startup` kills user-pref drift; from_pydata + explicit data is deterministic given identical code; avoid seeded randomness. .blend files are structurally reproducible, not byte-guaranteed (block/pointer ordering, compression); verify by re-open + structural diff, never by hash.
- Round-trip hook: re-open headless, dump stats JSON (object/mesh/vertex/face counts, bbox, quantized-normal checksum), diff.

#### COLLADA detail (dormant, YELLOW -> do not target)

Khronos COLLADA 1.5.1 (2008) is the last spec revision; the ecosystem (OpenCOLLADA) is unmaintained. Blender moved COLLADA I/O to legacy status with 4.2 LTS (May 2024), citing OpenCOLLADA maintenance/security, with dropping it on the table; the sharpest available dormancy signal. Licensing itself is fine (open). Rust: parsers (`collada` 0.17.0 piston, `dae-parser`); no production writer (`collada_io` 2020/stale). Verdict: dormant; read-tolerance only.

#### FBX detail (proprietary, RED, confirmed)

Autodesk owns it; the SDK is free-of-charge but closed spec + EULA (Blender's own dev blog: "its license and the software itself remains fully closed"). Blender ships a clean-room importer/exporter; `ufbx` (single-file C, dual MIT/Unlicense, aiming at FBX-SDK parity) is the open reader of record, adopted by Blender and Godot 4.3+, with official Rust bindings (`ufbx` crate v0.11.4, 2026-09, 523k downloads, read-only). `fbxcel`/`fbxcel-dom` = pure-Rust parser (read-only, light maintenance). No viable open write path -> RED for generation; if an FBX must exist, Blender headless's exporter is the escape hatch, but steer LLM output to glTF/USD instead.

#### Alembic (.abc) detail (GREEN license, subprocess-only in Rust)

Open framework from Sony Pictures Imageworks/ILM (2011); C++ lib + format; modified-Apache (DreamWorks-style) license, permissive. Ogawa backend (HDF5 legacy). Adoption: the baked-geometry/animation-cache standard in VFX; Maya, Houdini, Katana, Blender (core I/O), USD interop via UsdAbc. Rust: nothing viable (the `alembic` crate is a self-described 2020 placeholder; 2026 "alembic-core/engine" crates are an unrelated infra tool). Generation tier: SUBPROCESS (Blender headless bpy.ops.abc_export, or Alembic's Python). Round-trip verify via Blender re-import stats.

#### OpenVDB / nanoVDB detail (GREEN, read-only in Rust)

ASWF project (since 2018); v13.0 (2026) transitioned MPL-2.0 -> Apache-2.0 (ASWF lists, Aug 2026). Sparse volumetric grids; nanoVDB = the GPU-friendly subset. Adoption: Houdini volumes, Arnold/RenderMan/Karma, Blender volume I/O, UsdOpenVDBAsset in USD. Rust: `vdb-rs` (Traverse Research) pure-Rust reader whose README explicitly lists "VDB Writing" as missing; `nanovdb-rs` (2026-08, MPL-2.0) NanoVDB reader; `openvdb`/`openvdb-sys` are 2020 placeholders. Generation tier: SUBPROCESS (pyopenvdb / vdb_tool) or author via USD UsdVol + external grids.

#### Draco / meshopt / MaterialX (transforms and materials, not containers)

- Draco (Google, Apache-2.0): .drc container exists, but its real role is KHR_draco_mesh_compression inside glTF (ratified). Rust 2026: `draco-oxide` (reearth): pure-Rust encoder+decoder, bitstream 2.2/2.3, interop-tested vs Google Draco, glTF transcoder included (alpha); plus the young `draco-core`/`draco-io`/`draco-gltf` family (Apache-2.0). CPU-decode-heavy -> superseded by meshopt for most delivery.
- meshopt (zeux, MIT): EXT_meshopt_compression (ratified), the gltfpack reference pipeline, simplification + vertex-cache/fetch optimization. Rust: `meshopt` crate (gwihlidal, FFI bindings, active 2025-10). Treat both as post-process transforms on buffers you already authored; never as authoring targets.
- MaterialX (ASWF, Apache-2.0, v1.39.x 2025-2026): shading-network exchange; integrates into USD (UsdMtlx), Substance/Arnold/Omniverse/Karma; Blender integration in progress (GSoC 2026 + third-party importer). No Rust story; carry it inside USD or generate via subprocess (MaterialX Python tools: mxdoc etc.).

#### Interchange verdict 2026 (3D)

The "glTF + USD" assumption is half right, lane-dependent. glTF is confirmed as the universal delivery/realtime standard (web viewers, Unity 6 official support, Unreal Interchange, Godot-native, Onshape exports, AR). USD is the scene-composition layer (film pipelines, Apple AR/Vision Pro, Omniverse; AOUSD + ISO path) but is not universal in classic CAD or casual engine land (Unity USD experimental; Godot import-only). The missing third leg for a harness that authors geometry/CAD is STEP AP242: the only format every mechanical CAD package speaks, and the only one of the three carrying exact B-rep + PMI. Effective trio for an authoring harness: glTF (delivery) + STEP (CAD truth) + USD (scene assembly), with Blender headless (bpy) as the universal geometry factory bridging all three, and FreeCAD headless as the STEP specialist.

#### 3D Rust toolchain summary (verified Sept 2026)

| Format | Read | Write | Notes |
|---|---|---|---|
| glTF/GLB | `gltf` 1.4.1 (active) | `gltf-json` Serialize + `Glb::to_writer/to_vec`; `draco-gltf` | no high-level exporter crate; glTF-Validator itself is Rust |
| STL | `stl_io` 0.11.0 (2026-03) | `stl_io` | trivial; `mesh-io` doesn't exist |
| OBJ/MTL | `tobj` 4.0.5 (2026-08) | `wavefront-obj-io` 0.3.1 (2026-09); `obj`/`obj-exporter` (stale) | MTL write = hand-roll |
| USD/USDZ | `openusd` 0.7.0 (pure Rust) | `openusd` (usda/usdc/usdz) | pre-1.0; `pxr_rs` = C++ bindings |
| STEP | `step-p21`, `stepperoni`, `ruststep` (deser.) | kernel-only: `truck-stepio`, `monstertruck-io`, `brepkit-io` (AGPL) | general write = subprocess freecadcmd |
| 3MF | `threemf` (0BSD) | `lib3mf` (pure Rust, conformant) | zip+XML, easy |
| FBX | `ufbx` crate (official bindings) / `fbxcel` | NO | read-only, production-grade |
| Alembic / OpenVDB | NO / `vdb-rs`, `nanovdb-rs` (read) | NO | subprocess |
| Draco / meshopt | `draco-oxide` (pure Rust enc+dec, alpha) / `meshopt` (FFI) | same | transforms, not containers |
| COLLADA / IGES | `collada`, `dae-parser` / none | NO / NO | dormant / deprecated |
| Universal fallback | `assimp` crates dead (repo 404, 2017) | (n/a) | use Blender headless instead; legacy `three` crate abandoned (2019/2022); no "three356" crate exists |

#### 3D round-trip verify hooks

- glTF/GLB: re-parse via `gltf` crate; assert node/mesh/accessor/vertex counts; run Khronos glTF-Validator (Rust) -> must be 0 errors; structural diff of min/max bounds.
- STL/OBJ: re-parse (`stl_io`/`tobj`); triangle/vertex/face counts + quantized normal checksum; mtllib reference set for OBJ.
- USD/USDZ: `usdcat --flatten` -> canonical text diff; `usdchecker`; or pure-Rust re-read via `openusd` (layer prim/property counts).
- STEP: FreeCAD headless re-read; topology census (solids/faces/edges/vertices), volume + bbox within tolerance; strip/ignore the FILE_NAME timestamp before diffing.
- BLEND: re-open in `blender --background`; dump scene-stats JSON (objects, meshes, verts/faces, bbox, normal checksum) and diff; never byte-hash.
- 3MF: re-parse (`lib3mf`/`threemf`) + unzip -> 3D/3dmodel.model XML diff.
- Compressed buffers: decode (draco-oxide/meshopt) and compare vertex counts + attribute hashes vs source accessors.

#### 3D TOP-5 (adoption x licensing x feasibility)

1. glTF 2.0 / GLB. Adoption: highest of any open format (web/AR/Unity/Unreal/Godot/Blender/Onshape); licensing: royalty-free + ISO/IEC 12141, cleanest IP story in 3D; feasibility: pure-Rust read (`gltf`) + pure-Rust generation (`gltf-json` + GLB writer), Rust-native official validator. The default authoring target.
2. STEP AP242 (Ed3 2022, Ed4 in dev). Adoption: THE CAD interchange, mandated across aerospace/automotive, spoken by every MCAD package incl. FreeCAD; licensing: YELLOW only for the paywalled ISO text (usage unrestricted); feasibility: subprocess generation via freecadcmd/OCCT (pure-Rust write exists only kernel-scoped: truck/brepkit). The only lane that carries exact B-rep + PMI.
3. USD + USDZ. Adoption: film pipelines, Apple AR/Vision Pro, Omniverse, AOUSD/Linux-Foundation governance with an ISO path; licensing: GREEN (TomorrowOpen = Apache-2.0 minus trademark section); feasibility: pure-Rust `openusd` read+write (pre-1.0) + mature subprocess fallback (usdcat, Blender USD export). The scene-composition layer above 1-2.
4. STL (binary). Adoption: universal floor, every tool since 1987; licensing: GREEN (no owner, dead patents); feasibility: trivial pure-Rust R/W (`stl_io`, active 2026). Crude (no units/normals/colors/scene); keep as the compatibility emit + verify fixture.
5. BLEND. Adoption: the Blender ecosystem (the largest open-3D community); licensing: YELLOW (GPL tool, format documented but with zero cross-version stability contract); feasibility: excellent via headless bpy subprocess (official pip wheels, deterministic-enough with --factory-startup + bpy.data construction). The flagship capability format: author scene in bpy, save .blend, and re-emit glTF/USD/OBJ/STL from the same scene for cross-format verification.

Runner-up: 3MF. GREEN, pure-Rust R/W, strictly better than STL for printing; take it over STL whenever the consumer is a slicing/printing pipeline.

#### Blender headless capability note (quotable)

Blender headless python is the harness's universal 3D-generation engine. Two invocation paths, both official and CI-safe: `blender --background --factory-startup --python gen.py -- <argv>` (full binary, any platform) and `pip install bpy` (official Python wheel since Blender 3.4, late 2022; current bpy 5.2.2, Sept 2026, CPython 3.13 only; wheels for Linux x86_64 manylinux_2.28, Windows x64 and ARM64, macOS arm64; no Linux-ARM/macOS-Intel wheels, and old versions are archived off PyPI at download.blender.org/pypi). Supported Blender lines today: 4.5 LTS (Jul 2025) and 5.x (5.0, Nov 2025 onward); pin one version per harness environment because .blend internals and the bpy API shift per release. Deterministic output is achievable in practice: --factory-startup eliminates preference drift, and scene construction via `bpy.data` + `mesh.from_pydata(vertices, edges, faces)` (avoid context-dependent bpy.ops modeling; use bpy.ops only for import/export/save) is reproducible for identical inputs, but .blend files are structurally reproducible, not byte-reproducible (block ordering, pointers, optional Zstd), so verification must re-open the file headless and diff a stats dump (object/mesh/vertex/face counts, bbox, quantized-normal checksum) rather than a hash. The same session can export the authored scene to glTF/GLB, USD/USDZ, OBJ, STL, PLY, Alembic, FBX, COLLADA and save .blend via `bpy.ops.wm.save_as_mainfile(compress=True)`, giving one deterministic factory with built-in cross-format round-trip evidence (glTF output additionally passes the Rust-native Khronos glTF-Validator). Cost model: ~300-500 MB per environment, seconds per scene on typical LLM-authored geometry, zero licensing exposure (GPL covers Blender the tool; generated .blend files are the user's data). For CAD-exact B-rep/STEP, pair with FreeCAD's `freecadcmd` (OCCT-backed) rather than bpy; Blender has no B-rep kernel.

### 3.4 Audio / Video cluster (lane 4, part I)

*Deep-research snapshot, Sept 2026. Verified against the IETF datatracker (CELLAR WG RFCs), Xiph, Google/AOMedia license texts, Wikipedia licensing sections, and crate repos. Headline: the open A/V stack is now formally anchored at IETF level (FLAC = RFC 9639, Dec 2024; Matroska = RFC 9559, Oct 2024, both Proposed Standards), MP3 is verifiably patent-expired, and the user anchor holds: AV1/VP9/Opus GREEN, H.264/H.265/AAC RED.*

#### A/V verdict recap (audio codecs)

| Format | Spec body / version (year) | MIME / magic | Verdict | One-line law | Gen tier |
|---|---|---|---|---|---|
| Opus | IETF RFC 6716 (2012); Ogg-Opus RFC 7845 | audio/opus, audio/ogg / OggS | GREEN | IETF Standards-Track with royalty-free declarations from all major holders (Xiph, Microsoft/Skype, Broadcom); no patent pool targets Opus; residual third-party assertion risk is theoretical | S (libopus via ffmpeg/bindings; symphonia's opus codec is status "-" i.e. unimplemented) |
| FLAC | Xiph BSD-3 + IETF RFC 9639 (Dec 2024, Proposed Standard) | audio/flac / fLaC | GREEN | Dual-anchored (Xiph reference + IETF PS), patent-free, lossless; the only A/V format fully generable in pure Rust | Pure Rust (symphonia decode Excellent + claxon; flacenc encode) |
| Vorbis | Xiph Vorbis I spec (2004) | audio/vorbis / in Ogg | GREEN | Xiph BSD + patent grants; superseded by Opus, declining | P decode (symphonia Excellent) / S encode (libvorbis) |
| WAV (PCM) | IBM/MS RIFF, de-facto since 1991; RF64/BW64 for >4 GiB (EBU Tech 3285) | audio/vnd.wave / RIFF....WAVE | GREEN | No licensing at all; "spec" is convention | Pure Rust (`hound`, excellent R/W) |
| MP3 | ISO/IEC 11172-3:1993 + 13818-3 | audio/mpeg (RFC 3003) / ID3 or 0xFFFx sync | GREEN | All known patents expired (last US April 2017; Fraunhofer terminated its licensing program the same year); verified current, not folklore; not a formal open standard, a patent-expired ISO spec | P decode (symphonia Excellent) / S encode (libmp3lame) |
| AAC (LC/HE) | ISO/IEC 13818-7, 14496-3:2019 | audio/aac, audio/mp4 / ADTS 0xFFFx | RED | Via Licensing AAC pool regime alive ("Free format? No"); some core LC patents aging out 2023-2026 but the pool persists; do not generate | S (avoid generating) |
| ALAC (context) | Apple, open-sourced 2011 (Apache-2.0) | in MP4/M4A | GREEN | Clean since 2011; Apple-ecosystem niche | S (symphonia decode Great) |

#### A/V verdict recap (video codecs, containers, subtitles)

| Format | Spec body / version (year) | MIME / magic | Verdict | One-line law | Gen tier |
|---|---|---|---|---|---|
| AV1 | AOMedia AV1 v1.0.0 (2018) + errata | video/AV1 / in container | GREEN | AOMedia Media Patent Cross-License (royalty-free from 50+ members incl. Apple/Google/MS/Intel/Netflix) with defensive termination; Sisvel runs a voluntary AV1/VP9 pool; assertions filed (Elemedia v. Google re VP9, InterDigital v. Lenovo, a reported 2026 Dolby suit naming AV1, unverified details) have produced no sustained ruling; Unified Patents keeps invalidating claims; EU antitrust probe of the cross-license closed with no action (2023) | S (SVT-AV1 = fastest production encoder; libaom = reference; rav1e = pure-Rust option, BSD-2 + PATENTS, weekly releases) |
| VP9 | Google WebM VP9 spec | video/VP9 / V_VP9 in MKV | GREEN | Google royalty-free reciprocity grant; MPEG LA abandoned its VP8/9 assertion attempt in 2013 after DOJ scrutiny; Elemedia suit (2023) unresolved with no public final verdict: background noise given the shipped base | S (libvpx-vp9) |
| Theora | Xiph, VP3-derived (2004) | video/theora / in Ogg | GREEN but DEAD, confirmed | On2/Xiph free grant, but libtheora's last meaningful release was ~2010-2013; VP8/9/AV1 superseded it; even Wikimedia discourages new uploads | Do not generate |
| Matroska (MKV) | IETF RFC 9559 (Oct 2024, Proposed Standard); EBML = RFC 8794 (2020) | video/x-matroska / EBML 1A 45 DF A3 | GREEN | Now a formal IETF standard (CELLAR WG); codec/tags companion drafts in the IESG pipeline; MKV+FFV1 is the library/archival standard pairing | S (symphonia mkv demux Good; mux via ffmpeg) |
| WebM | Matroska profile (Google): VP8/VP9/AV1 + Vorbis/Opus | video/webm / same EBML | GREEN | BSD spec + Google patent grant for the VP codecs; web-native (HTML5 video, MSE/WebRTC) | S |
| MP4 / BMFF / M4A | ISO/IEC 14496-12/-14; readable free via ISO Online Browsing Platform | video/mp4, audio/mp4 / ftyp at offset +4 | YELLOW container, RED typical payload | The container itself is an open ISO standard with no royalty regime; M4A is only "problematic" through its AAC payload; MP4 holding AV1/Opus/ALAC is clean | S (mp4 / mp4san pure-Rust demux; symphonia isomp4 demux Great; mux via ffmpeg) |
| Ogg | RFC 3533 (2003); MIME RFC 5334 | application/ogg / OggS | GREEN | Xiph open, IETF-documented; canonical carrier for Opus/Vorbis/FLAC-in-Ogg | P (muxing trivial; symphonia Ogg Great) |
| Y4M (YUV4MPEG2) | de-facto, community-documented | none / YUV4MPEG2 header | GREEN | Raw planar frames + per-frame headers; nothing to license; the pipe format between encoders (ffmpeg, rav1e input, dav1d output) | Pure Rust (trivial) |
| FFV1 | IETF RFC 9043 (2021, v0/1/3; v4 draft expired) | FourCC FFV1 in MKV | GREEN | Born in ffmpeg, now IETF Informational; lossless archival-grade; LoC/film-archive standard in the MKV+FFV1 pairing | S (ffmpeg -c:v ffv1) |
| "Filmux" | none found | none | NOT VERIFIABLE | No notable format by this name surfaced in Sept 2026 verification; the real archival intermediate path is y4m (interchange) + FFV1-in-MKV (archival) | Ignore |
| SRT | de-facto, no standards body | application/x-subrip / plain text | GREEN | Nothing to license; no real spec (dubious timing semantics) | Pure Rust (trivial) |
| WebVTT | W3C (HTML5 family) | text/vtt / WEBVTT | GREEN | W3C Royalty-Free policy; the web standard (HTML5 track) | Pure Rust (trivial) |
| ASS/SSA | Aegisub/libass de-facto | text/x-ssa / [Script Info] | GREEN | Free; the styling/render engine (libass, ISC) is C | P authoring / S render |

#### ffmpeg encoder-flag cleanliness (the harness's actual generation surface)

| Flag | Library / license | Clean? | Notes |
|---|---|---|---|
| -c:a libopus | libopus, BSD-3 | GREEN | enabled in Debian/Ubuntu/brew/BtbN builds |
| -c:v libvpx-vp9 | libvpx, BSD + WebM grant | GREEN | ubiquitous |
| -c:v libsvtav1 | SVT-AV1, BSD-2 + AOM patent terms | GREEN | in Debian 12+ / Ubuntu 24.04+ / gyan / BtbN builds; verify per build: `ffmpeg -hide_banner -encoders` plus `ffmpeg -buildconf` |
| -c:v libaom-av1 | libaom, BSD-2 + AOM | GREEN | reference quality, very slow |
| -c:a flac (native) | in-tree, LGPL | GREEN | no external lib needed |
| -c:a libmp3lame | LAME, LGPL | GREEN | patents expired; build-clean |
| -c:v ffv1 | in-tree, LGPL | GREEN | archival lossless |
| -c:a libvorbis, -c:v libtheora | BSD / Xiph | GREEN | theora is a dead target |
| -c:a aac (native) | in-tree, experimental-ish | YELLOW | tool-clean, content remains encumbered |
| -c:a libfdk_aac | Fraunhofer FDK | RED | requires --enable-nonfree: the binary becomes unredistributable; never ship it or let CI depend on it |
| -c:v libx264 / libx265 | GPL + commercial pools | RED | content and tool licensing both (MPEG LA / Access Advance / Velos pools) |

Key legal pattern: invoke a distro-provided ffmpeg as a subprocess. ffmpeg core is LGPL-2.1+; subprocess invocation (no linking, no redistribution of a modified ffmpeg) keeps the harness clear of ffmpeg's own licensing entirely; only output-content licensing then matters, which the flag table controls. A WASM tier exists (ffmpeg.wasm) but is roughly 10x slower; reserve it for browser-only contexts.

#### A/V round-trip hooks

- Verify: `ffprobe -show_format -show_streams -print_format json out.{webm,mkv,opus,flac}`; assert codec name, duration delta < 1 ms, sample rate, stream count.
- Lossy codecs (Opus/VP9/AV1): never assert byte equality; assert duration + stream count + decode success, optionally PSNR/VMAF vs source.
- FLAC/WAV: assert PCM hash equality via `ffmpeg -i x -map 0:a -f md5 -`.
- WAV > 4 GiB must become RF64. MKV remux can shift timestamps/resolution. MP4: check ftyp brand compatibility; modern builds avoid `-strict experimental` for AV1.

#### A/V TOP-3 (adoption x licensing x Rust feasibility)

1. Opus (RFC 6716): default codec of the realtime/streaming web (WhatsApp/Zoom/Discord/YouTube); ffmpeg libopus everywhere; the one cost is the pure-Rust decode gap (symphonia opus unimplemented), so the tier is subprocess/bindings.
2. FLAC (RFC 9639 + Xiph BSD): bit-exact round-trips; symphonia decode is Excellent and pure-Rust encode exists (flacenc); the only cluster-I format fully generable in pure Rust today.
3. AV1 via SVT-AV1 (ffmpeg libsvtav1), with VP9 as the compatibility fallback for old hardware decoders; both anchored by AOM/Google grants with zero sustained court losses; container Matroska/WebM (RFC 9559), and MKV+FFV1 additionally covers the archival lane; pure-Rust rav1e is a credible niche encoder.

### 3.5 Scientific / data / interchange cluster (lane 4, part II)

*Deep-research snapshot, Sept 2026. Verified against ASF repos, zarr-specs/zarrs, sqlite.org, IANA, and the serde ecosystem. Headline: Parquet/Arrow are the pure-Rust columnar backbone; HDF5 is open-licensed but single-vendor; the YAML crate ecosystem re-aligned (serde_yaml archived 2024); SQLite's file format is contractually frozen.*

#### Data verdict recap

| Format | Spec body / version (year) | MIME / magic | Verdict | One-line law | Gen tier |
|---|---|---|---|---|---|
| Parquet | Apache parquet-format (ASF), 2.x/3.x line; donated 2013 | application/vnd.apache.parquet / PAR1 | GREEN | Apache-2.0 spec + implementations; no patents asserted, ASF governance; THE columnar standard (Spark/DuckDB/polars/pandas/BigQuery external/GDAL 3.5+) | Pure Rust (`parquet` in arrow-rs: reference-grade, actively shipped) |
| Arrow IPC / Feather v2 | Apache Arrow format spec 1.x (ASF) | application/vnd.apache.arrow.stream / ARROW1 + 0xFF continuation | GREEN | Apache-2.0; in-memory columnar standard with formal cross-language conformance | Pure Rust (arrow-rs ipc) |
| HDF5 | The HDF Group 1.14.x; magic 89 48 44 46 0D 0A 1A 0A | application/x-hdf5 | YELLOW | BSD-like permissive license but single-vendor stewardship (The HDF Group is developer/maintainer/steward, verified Sept 2026); spec = the implementation; gripes: no in-place space reclamation (h5repack), UTF-8 unenforced | S/C bindings (`hdf5` crate, slow-moving; no credible pure Rust) |
| NetCDF-4 | Unidata/UCAR; classic CDF header, v4 = HDF5 file | application/x-netcdf | GREEN/YELLOW | Permissive Unidata license but inherits HDF5's single-vendor nature; weather/climate standard (xarray ecosystem) | S/C bindings (`netcdf` crate) |
| FITS | IAU FITS Working Group, FITS 4.0 (2016); born 1981 | application/fits / ASCII `SIMPLE  = ` | GREEN | NASA/IAU open and royalty-free by design since 1981; alive and dominant in astronomy (JWST, Rubin/LSST, ESO all emit it) | S/C bindings (`fitsio` over CFITSIO) |
| NPY/NPZ | NumPy npy-format spec (openly published) | application/x-npy (de-facto) / 93 NUMPY (\x93NUMPY) | GREEN | Trivial documented format (header + raw buffer); NPZ = zip; ML interchange lingua franca | Pure Rust (`ndarray-npy`) |
| JSON / JSONL | RFC 8259; I-JSON RFC 7493; jsonlines de-facto | application/json, application/x-ndjson | GREEN | IETF, patent-free, universal | Pure Rust (serde_json) |
| CSV | RFC 4180 (2005, informational) | text/csv | GREEN | IETF informational; encoding/BOM/quote dialect chaos is the only hazard | Pure Rust (`csv`, BurntSushi, best-in-class) |
| YAML | yaml.org 1.2.2 (2021); media type RFC 9512 (application/yaml) | application/yaml | YELLOW | Spec open, but serde_yaml is archived (2024); avoid serde_yml (quality controversy); prefer serde_yaml_ng or saphyr (ex-yaml-rust2); pick deliberately | Pure Rust |
| TOML | TOML 1.0 (MIT-licensed spec) | application/toml (IANA) | GREEN | MIT spec, single benevolent editor, zero drama; Rust-native config | Pure Rust (`toml`) |
| SQLite (as interchange file) | sqlite.org file-format doc; stable and frozen since 3.x | application/vnd.sqlite3 / SQLite format 3\0 | GREEN | Code public domain; format documented and frozen by contract ("SQLite as an application file format"); nuance: not designed for multi-tool semantic interchange, the schema is yours; ship single file, journal_mode=DELETE, checkpoint WAL, set application_id/user_version | P via bundled C (rusqlite, public-domain lib) |
| Protobuf | Google proto3 (BSD spec) | application/x-protobuf / none | GREEN | BSD + patent non-assert norms; gRPC wire format | Pure Rust (prost, tokio-rs) |
| FlatBuffers | Google, Apache-2.0 | application/octet-stream / none | GREEN | Apache-2.0; gaming/embedded zero-copy niche | Pure Rust (official crate, moderate cadence) |
| Cap'n Proto | MIT | none | GREEN | MIT; single maintainer but healthy | Pure Rust (`capnp`, active 0.20 line) |
| ZSTD | Meta, BSD-3 + explicit patent grant in repo | application/zstd / 28 B5 2F FD | GREEN | Modern BSD with Meta's OSS patent policy; the compression layer under Parquet/Arrow | Pure Rust (`zstd` bindings, `ruzstd` decode) |
| gzip / lz4 | RFC 1952/1951 (deflate patent-clear by design) | application/gzip / 1F 8B | GREEN | Permissive licenses; universal | Pure Rust (flate2/miniz_oxide, lz4_flex) |
| xz | liblzma, public-domain-ish | application/x-xz | YELLOW (operational, not legal) | Licensing fine; the 2023 supply-chain backdoor (CVE-2024-3094) means pin/verify versions; socialized risk, not legal risk | Pure Rust options limited; often subprocess |
| Zarr v3 | zarr-specs (CC-BY-4.0), Zarr 3.x finalized with ZEP process | directory store / zarr.json | GREEN | Community governance, open specs; the credible cloud-native HDF5 challenger (OME-Zarr in bioimaging, Pangeo in geo) | Pure Rust (`zarrs`, ANU: conformant through Zarr 3.1, all ZEPs, Apache-2.0/MIT, actively released) |

#### Data round-trip hooks

- Parquet: read back with duckdb (`SELECT count(*), sum(...) FROM 'x.parquet'`) or polars/pyarrow; assert row count + schema fingerprint (names/types). Risks: logical-type drift (timestamps/Date64 nuance, decimals); dictionary-encoding differences are benign.
- Arrow IPC: re-read via arrow-rs; compare `Schema::eq` plus record-batch byte lengths.
- HDF5/NetCDF: `h5dump -n` / `ncdump -h`; risks: attribute models, chunking bloat, inability to reclaim space without h5repack.
- SQLite: reopen and run `PRAGMA integrity_check`; ensure no -wal/-shm sidecars remain after close.
- Zarr: re-open the store with `zarrs`; verify zarr.json metadata + chunk digests; risk: codec extension mismatch across readers.

#### Data TOP-3

1. Parquet: the columnar interchange standard of the entire data industry; arrow-rs is pure Rust, reference-grade, and actively shipped in lockstep with Java/C++.
2. Arrow IPC: zero-copy in-memory + file/stream standard; arrow-rs is one of the ecosystem's two reference implementations.
3. SQLite: public-domain code, frozen documented format, rusqlite bundles cleanly; the perfect single-file agent-output substrate (and the GeoPackage container). (Zarr v3 via zarrs is the designated big-array/cloud pick: pull it the moment outputs exceed RAM.)

### 3.6 Geospatial cluster (lane 4, part III)

*Deep-research snapshot, Sept 2026. Verified against OGC, ASPRS/rapidlasso announcements, the Mapbox vector-tile-spec repo, and georust crate metadata. Headline: GeoPackage 1.4 (Feb 2024) is the sanctioned Shapefile successor; the old LASzip licensing unease closed for good (Apache-2.0, late 2021) and LAZ became an OGC Community Standard (2026); MVT's legal text is explicitly free-use but Mapbox-governed.*

#### Geo verdict recap

| Format | Spec body / version (year) | MIME / magic | Verdict | One-line law | Gen tier |
|---|---|---|---|---|---|
| GeoTIFF | TIFF 6.0 (Aldus/Adobe 1992) + OGC GeoTIFF 1.1 (OGC 19-075, 2019) | image/tiff / 49 49 2A 00 or 4D 4D 00 2A | GREEN | TIFF spec freely published for decades, no live patents (LZW expired 2004); GeoTIFF keys standardized at OGC; every GIS speaks it | S (GDAL; pure-Rust read possible via `tiff` + manual GeoKeys, painful) |
| COG (Cloud-Optimized GeoTIFF) | Radiant Earth COG spec (community AWG; a TIFF profile: tiled, overviews, IFD ordering) | image/tiff; profile=cloud-optimized (GDAL convention) | GREEN | Open community spec; disciplined GeoTIFF; the current raster delivery standard (titiler/Planetary Computer/DEA/aws-earth) | S (`gdal_translate -of COG`; validate `gdalinfo` + cog_validator) |
| GeoPackage | OGC GeoPackage 1.4 (Feb 2024) | application/geopackage+sqlite3 (IANA-registered) / SQLite format 3\0 | GREEN | OGC open standard on a public-domain SQLite container; the sanctioned Shapefile successor; QGIS default, GDAL/OGR first-class, ArcGIS/Field Maps, US Natl Map | P (rusqlite + geozero WKB blobs) or S (GDAL OGR write) |
| Shapefile | ESRI white paper (1998), published open | (none registered) / .shp magic 0x0000270A | YELLOW (dated) | Free and published since 1998, no royalties, but the format itself is the liability: 2 GiB limits, 10-char field names, dBASE types, nulls/encoding chaos, 3+ mandatory sidecars; legacy-ubiquitous, nobody should generate in 2026 (emit GeoPackage) | P read-only policy (`shapefile` crate) |
| KML | OGC KML 2.3 (12-007r line) | application/vnd.google-earth.kml+xml / XML | YELLOW | OGC-open (originated Google/Keyhole), legally clean; adoption flat (Google Earth web declining, verbose styling) | P read (`kml` crate, georust) / S generate |
| GPX | TopoGrafix GPX 1.1 (XML Schema, free) | application/gpx+xml | GREEN | Free schema, no royalties; the GPS-track standard (Strava/Garmin/BaseCamp) | Pure Rust (`gpx`, georust, solid R/W) |
| LAS / LAZ / COPC | ASPRS LAS 1.4 R15 (2019); LAS 1.5 + new LASzip (May 2026); LAZ spec 1.4 (rapidlasso, Oct 2024); OGC LAZ 1.4 Community Standard (2026); COPC 1.0 (Hobu, 2021) | application/vnd.las / LASF | GREEN (verified resolution) | LAS is the ASPRS open standard; LASzip was LGPL with redistribution friction until it was relicensed Apache-2.0 in late 2021 (signed Isenburg grant); "REFLAS" could not be verified as a named thing; the operative fact is the 2021 Apache grant; alternatives laz-perf (Apache) and laz-rs (pure Rust) exist | Pure Rust (`las` + `laz` crates, R/W incl. LAZ compression) |
| GML | OGC GML 3.2.x / ISO 19136 | application/gml+xml | YELLOW (dated) | OGC-open but maximally verbose XML with namespaced schemas; lives on via INSPIRE/cadastral mandates and WFS payloads | S (GDAL/geozero read; generate only if legally mandated) |
| FlatGeobuf | Harrtell spec (MIT), FlatBuffers-based, single file, R-tree | application/x-flatgeobuf (de-facto) / fgb + version header | GREEN | MIT, community-governed; growing: GDAL/OGR R/W, QGIS, MapServer, geojson.xyz serving | Pure Rust (`flatgeobuf`, reader + writer) |
| MVT (Mapbox Vector Tiles) | Mapbox vector-tile-spec 2.1 (Jan 2016, still latest) | application/vnd.mapbox-vector-tile / protobuf, often gzip | YELLOW-GREEN | Spec text CC-BY 3.0 US with an explicit grant: "use of this spec in products and code is entirely free: no royalties, restrictions, or requirements"; caveat: governed by Mapbox's repo, NOT an OGC standard; OGC API-Tiles Part 1 (20-057) references MVT as a conformance class but never adopted the format itself; de-facto web-mapping standard (MapLibre/OpenMapTiles/Protomaps) | Pure Rust (`geozero` emits MVT; hand-rolled prost also common) |
| WKT / WKB / EWKB | OGC Simple Features (SFSQL 1.2.1 / ISO 19125); WKT2 = ISO 19162:2019 | none (geometry encoding) | GREEN | OGC/ISO open; the encoding layer under PostGIS/GPKG; universal geometry interchange | Pure Rust (`wkt` georust R/W, `geozero` WKB/EWKB) |
| CRS sidecars (.prj / WKT2) | OGC WKT2:2019 (ISO 19162); EPSG registry free | .prj text | GREEN | Open; EPSG codes freely usable; GDAL/PROJ canonical | S/C bindings (`proj` crate; PROJ is MIT) |
| GeoJSON (bonus) | IETF RFC 7946 (2016) | application/geo+json | GREEN | IETF standard, no patents; omitted from the brief's list but unavoidable in practice; the web/GIS default lightweight vector | Pure Rust (`geojson`, georust, R/W) |

#### The LAS/LAZ licensing story (resolved, quotable)

LAS 1.4 R15 (2019) is the ASPRS open standard; LAS 1.5 plus a new LASzip emerged May 2026. LAZ compression rode LASzip, which spent years as LGPL (redistribution friction) until Martin Isenburg signed an Apache-2.0 grant in late 2021; rapidlasso published the LAZ 1.4 spec (Oct 2024) and OGC adopted LAZ 1.4 as a Community Standard in 2026. "REFLAS" produced no verifiable primary sources in Sept 2026 checks: the operative compliance fact is the 2021 Apache-2.0 relicensing, which mooted the historical unease. COPC 1.0 (Hobu, 2021) adds cloud-optimized point-cloud chunking on top. Rust: the `las` + `laz` crates (pure Rust; LAZ via the laz-rs lineage) read and write, including LAZ compression.

#### Geo round-trip hooks

- Raster COG/GeoTIFF: `gdalinfo -json x.tif` (assert profile=cloud-optimized for COG); re-read with GDAL/rasterio and compare pixel stats + geotransform; risks: overview misalignment, nodata semantics; DEFLATE/ZSTD compression safe.
- GeoPackage: `ogrinfo -so -al x.gpkg` plus `PRAGMA integrity_check`; SRID must be registered in gpkg_spatial_ref_sys; WKB must be little-endian standard; RTree extension optional.
- LAS/LAZ: `pdal info --summary` / `lasinfo`; risks: scale/offset precision loss (int32 coords!), point format vs classification mapping; COPC wants octree chunking (use PDAL tooling).
- MVT: decode with a spec-conformant reader; risks: extent quantization, ring winding (spec leaves polygon validity open), buffer clipping artifacts, gzip layer framing.
- WKT/WKB: round-trip via geozero in both directions; the #1 geospatial bug class is CRS axis order (lat,lon vs lon,lat): normalize to EPSG:4326 lon,lat per RFC 7946 convention.

#### Geo TOP-3

1. GeoPackage 1.4: the modern single-file vector(+raster) OGC standard, readable by every validator that matters (QGIS/GDAL/PostGIS-ingestible); writable pure Rust (rusqlite + geozero) or via GDAL.
2. COG: the raster delivery standard of the cloud-GIS world; generation stays a GDAL subprocess; the validation story is mature (gdalinfo, cog-validator).
3. MVT (+ GeoJSON as the lightweight sibling): the web-mapping interchange of record; pure-Rust generation via geozero; carry the "Mapbox-governed spec, OGC-referenced not adopted" caveat in the plan. (LAS/LAZ with pure-Rust `las` + `laz` crates is the high-leverage niche add now that LASzip is Apache-2.0.)

#### Lane 4 landmine paragraph (quotable)

The safe-by-default stack is: Opus/FLAC/WAV audio, AV1/VP9 + FFV1 video in Matroska/WebM (IETF RFC 9559), Parquet/Arrow/Zarr/SQLite data, GeoPackage/COG/MVT/GeoJSON/LAZ geometry: every one anchored by an IETF RFC, an Apache/OGC/Xiph/AOM instrument, or patent expiry. The precise traps: (1) MP3 is verified patent-free (last patents expired 2017, Fraunhofer closed its program): the lingering fear is folklore; AAC is not, its pool regime is alive, and ffmpeg's libfdk_aac additionally taints the binary with --enable-nonfree so it must never ship. (2) H.264/H.265 stay RED: multiple active pools (MPEG LA/Access Advance/Velos) plus GPL-or-commercial x264/x265; note the market itself is fleeing (Disney+ EU switched HEVC to VP9 in Aug 2026 over the InterDigital dispute). (3) AV1/VP9 "royalty-free" carries asterisks that have never materialized: Sisvel sells a voluntary pool, Elemedia/InterDigital suits (and a reported 2026 Dolby filing) are in flight, and the EU probed AOMedia's defensive-termination cross-license before closing with no action (2023); zero court wins against the codecs to date; treat as GREEN with litigation-watch. (4) Container is not codec: MP4/BMFF is an open ISO container; M4A is only "problematic" because of its AAC payload; MP4+Opus/AV1/ALAC is clean. (5) Single-vendor openness: HDF5 (and NetCDF-4 on top of it) is BSD-licensed but HDF-Group-stewarded; fine to emit, risky to bet architecture on; Zarr v3 is the open challenger. (6) Spec-governance nuances: MVT's text is Mapbox CC-BY with an explicit free-use grant but OGC referenced rather than adopted it; ISO media specs (MP4/H.264) are readable free online yet formally ISO-royalty-governed. (7) LASzip is Apache-2.0 only since late 2021: pre-2021 LGPL binaries and forks are the residual compliance surface, now mooted by the signed grant and the 2026 OGC LAZ Community Standard. (8) Tool licensing is not content licensing: invoking a distro ffmpeg as a subprocess isolates the harness from LGPL/GPL obligations; only the output content's licensing then matters, and the flag table above controls that.

Residual risks / unverified items: Elemedia v. Google's final disposition and the reported 2026 Dolby AV1 suit (no public final verdict found this pass); "Filmux" and "REFLAS" produced no verifiable primary sources; polars' internal parquet engine lineage (arrow2-derived) noted without independent verification, and irrelevant to output validity since DuckDB/pyarrow/GDAL all read arrow-rs-written Parquet.

## 4. Cross-cutting toolchain matrices

### LibreOffice headless (documents)

- Command shape: `soffice --headless --convert-to X`. Verified clean pairs: docx <-> odt <-> rtf <-> doc (read); xlsx <-> ods <-> csv (with `--infilter` dialect control, e.g. `CSV:59,34,76,1`); pptx <-> odp; any-of-them -> pdf; Writer docs -> epub (filter since LO 6.0); -> html / txt / png; flat-XML fodt.
- Ops musts: one `-env:UserInstallation=file:///tmp/lo_N` profile per concurrent process (a second instance on a shared profile silently exits 1); batch many files per invocation (measured 5.96s vs 13.52s for 4 files); hard timeout (corrupt input hangs rather than errors); fonts must be OS-installed or layout silently shifts; complex Word layouts drift, diff-test.
- EPUB export understands character/paragraph styles only (frames/tables/page styles degrade).

### pandoc (universal tier-b pivot)

- Reads: CommonMark/GFM variants, AsciiDoc (input only), HTML, LaTeX (subset), DocBook, EPUB, DOCX, ODT, RST, Org, djot. Writes: DOCX, ODT, EPUB 2/3, PPTX, PDF (via typst/LaTeX/weasyprint engines), HTML5, LaTeX/beamer, man.
- `--reference-doc` gives style control for docx/odt/pptx. Cannot: XLSX/spreadsheets (use rust_xlsxwriter or LibreOffice), DjVu, AsciiDoc output.
- GPL: fine as a sandboxed subprocess, not linkable.

### ffmpeg (A/V generation surface)

- Flag cleanliness table in section 3.4. Pattern: invoke a distro-provided ffmpeg as a subprocess (LGPL-2.1+ core; no linking, no redistribution of a modified ffmpeg).
- Verify per build: `ffmpeg -buildconf` plus `-encoders | grep svt` (libsvtav1 ships in Debian 12+/Ubuntu 24.04+/gyan/BtbN builds). Never `libfdk_aac` (nonfree binary) or `libx264`/`libx265` (RED content).

### Blender headless (bpy; universal geometry factory)

- Two invocation paths, both official: `blender --background --factory-startup --python gen.py -- <argv>` (full binary) or `pip install bpy` (official wheel; bpy 5.2.2, Sept 2026, CPython 3.13 only; win_amd64/win_arm64/manylinux_2_28/macos-arm64).
- Construct scenes via `bpy.data` (+ `mesh.from_pydata` + `mesh.validate()`); use `bpy.ops` only for I/O (save/export). Pin one Blender version per environment (4.5 LTS or 5.x): .blend internals and the bpy API shift per release.
- Determinism: structurally reproducible, not byte-reproducible; verify by re-open + stats-dump diff, never by hash. `--factory-startup` kills user-pref drift.
- One session emits .blend + glTF/GLB + USD + OBJ + STL + PLY + Alembic + FBX: a built-in cross-format verification factory. Cost ~300-500 MB per environment.

### FreeCAD headless (STEP specialist)

- `freecadcmd script.py` (or `-c`); same Python modules minus GUI. Export: `import Part; Part.makeBox(...).exportStep(path)` or `Import.export([obj], path)`. FreeCAD 1.0.x, OCCT-backed; writes AP214-flavored by default, reads AP242 solid (PMI partial). The pragmatic STEP engine; Blender has no B-rep kernel.

### GDAL (geo raster/vector)

- Raster: `gdal_translate -of COG` for COG generation; `gdalinfo -json` asserts the cloud-optimized profile; DEFLATE/ZSTD compression safe.
- Vector: `ogrinfo -so -al` for GeoPackage inspection; GDAL 3.5+ reads Parquet (GeoParquet nuance).
- PROJ axis order (lat,lon vs lon,lat) is the #1 geospatial bug class: normalize to EPSG:4326 lon,lat per RFC 7946 convention.

### Verification sidecar tools by lane

- Documents: `qpdf --check`, `mutool`, `pdftotext`, veraPDF (PDF/A), epubcheck (Java).
- Images: usvg structural parse + resvg render + perceptual hash (SVG); ExifTool as the metadata oracle.
- 3D: Khronos glTF-Validator (itself Rust), usdchecker, headless stats-dump diffs (Blender/FreeCAD).
- A/V: ffprobe JSON assertions; `ffmpeg -f md5` for lossless PCM equality.
- Data: duckdb/polars read-back + schema fingerprint; `PRAGMA integrity_check` (SQLite); h5dump/ncdump (HDF/NetCDF).
- Geo: cog_validator, pdal info / lasinfo, ogrinfo, geozero two-way round-trip.

## 5. Notable 2025-2026 status changes

- JPEG XL reversal completed: Google reversed position Nov 2025; Chrome 145 shipped the decoder behind a flag Feb 2026; Firefox 152 flag Jun 2026; Firefox 157 default-on end of Sept 2026; Chrome default flip expected H2 2026. (Corrections: JXL is ISO/IEC 18181, not 18477; Chrome's removal was Feb 2023, Chrome 110.)
- PNG: W3C Third Edition Recommendation 2025-06-24 (formalizes image/apng MIME and the eXIf chunk).
- DNG: ISO 12234-4:2026 published 2026-03-24 (Adobe's revocation clause remains; verdict stays YELLOW).
- IETF CELLAR wave: FLAC = RFC 9639 (Dec 2024), Matroska = RFC 9559 (Oct 2024), EBML = RFC 8794 (2020), FFV1 = RFC 9043 (2021): the open A/V stack is now formally anchored.
- ODF 1.4: OASIS Standard 2025-12-03; ODF 1.3 = ISO/IEC 26300:2025; Germany mandates ODF + PDF/UA from March 2026; MS 365 writes ODF 1.4 since v2404 (mid-2024).
- EPUB 3.4 revision chartered 2025-02-11 (to 2027); TeX Live 2025: all engines emit PDF 1.7 by default.
- OpenUSD: license renamed TomorrowOpen (May 2024, Apache-2.0 differing only in Section 6); AOUSD formed Aug 1 2023 under LF/JDF with an explicit ISO path; pure-Rust `openusd` 0.7.0 (Sept 2026) reads and writes usda/usdc/usdz.
- glTF: transposed as ISO/IEC 12141:2022 (Khronos retains control); meshopt/Draco/KTX2 extensions ratified; 3D Gaussian-splat extensions added Sept 2025.
- OpenVDB v13 (2026): MPL-2.0 -> Apache-2.0.
- Lidar: LASzip relicensed Apache-2.0 (late 2021, signed grant); LAZ spec 1.4 (rapidlasso, Oct 2024); OGC LAZ 1.4 Community Standard (2026); LAS 1.5 + new LASzip (May 2026); COPC 1.0 (2021).
- Geo: GeoPackage 1.4 (Feb 2024); MVT still 2.1 (2016) and NOT an OGC standard (OGC API-Tiles 20-057 references it as a conformance class).
- HEVC in retreat: Disney+ EU switched HEVC -> VP9 (Aug 2026, InterDigital dispute); EU AOMedia antitrust probe closed with no action (May 2023); a Dolby suit naming AV1 reported 2026 (unverified); no court has ever sustained a claim against VP8/VP9/AV1.
- Rust data ecosystem: serde_yaml archived (2024) -> serde_yaml_ng / saphyr; YAML media type registered as RFC 9512; xz supply-chain backdoor CVE-2024-3094 (Mar 2024) -> pin/verify.
- Blender line: 4.5 LTS (Jul 2025), 5.0 (Nov 2025), bpy 5.2.2 (Sept 2026, CPython 3.13 only); COLLADA I/O moved to legacy status with 4.2 LTS (May 2024).
- 3D Rust: symphonia's Opus codec still unimplemented (status "-") while FLAC/WAV/MP3 decode is Excellent; `brepkit-io` went AGPL-3.0-or-commercial at v3; `ruststep` remains deserialize-only and dormant.

## 6. Recommended slate

One ranked list reconciling the lanes' TOP-N verdicts with the user-approved sets (SVG, glTF, .blend, PDF, audio; STEP, GeoTIFF/KML, HDF5/Parquet; DOCX/XLSX/PPTX) plus evidence-based additions. Every user-approved format lands in CORE or carries an explicit DEFERRED rationale.

### CORE (build generation + verification first, in this order)

| # | Format(s) | Why (one line) | Primary generation/verify path |
|---|---|---|---|
| 1 | PDF (+ PDF/A when archival) | User-approved; universal; Adobe grant | krilla/printpdf; qpdf + pdftotext (+ veraPDF for PDF/A) |
| 2 | PNG / APNG | Lossless workhorse; W3C 2025 RF policy | png/image crate; decode-back + pixel hash |
| 3 | SVG | User-approved; the only universal vector format | templating -> usvg validate -> resvg render + perceptual hash |
| 4 | JPEG | The lossy interchange everything reads | zune-jpeg/jpeg-encoder; PSNR floor |
| 5 | WebP | Cleanest modern-codec licensing paper trail | image-webp lossless pure Rust; ffmpeg for lossy/animated |
| 6 | AVIF | ~94% browser reach; GREEN-with-footnote | ravif encode (slow, pure Rust); ffmpeg decode-back |
| 7 | DOCX + XLSX (+ PPTX) | User-approved; industry default | rust_xlsxwriter, docx-rs, umya-spreadsheet; PPTX via pandoc/LO; LO no-op smoke test |
| 8 | Markdown + HTML5 | Trivial, GREEN, universal | comrak/pulldown-cmark; maud/askama; self-contained single-file HTML |
| 9 | glTF 2.0 / GLB | User-approved; THE 3D delivery standard; Rust-native validator | gltf-json + Glb::to_writer; Khronos glTF-Validator gate |
| 10 | .blend via bpy headless | User-approved flagship; universal geometry factory with built-in cross-format verification | blender --background --factory-startup; stats-dump structural diff |
| 11 | STEP AP242 via freecadcmd | User-approved CAD truth; the only exact B-rep + PMI lane | freecadcmd export; topology census diff (strip FILE_NAME timestamp) |
| 12 | Opus + FLAC + WAV | User-approved audio anchor; RFC 6716/9639; FLAC fully pure-Rust | ffmpeg libopus; symphonia + flacenc; ffprobe + `ffmpeg -f md5` |
| 13 | AV1/VP9 in WebM/MKV | The H.264/H.265 replacement; containers now IETF RFC 9559 | ffmpeg libsvtav1 / libvpx-vp9; ffprobe assertions; MKV+FFV1 archival variant |
| 14 | Parquet + Arrow IPC | User-approved; the data-industry backbone | arrow-rs; duckdb/polars read-back + schema fingerprint |
| 15 | SQLite | Public-domain single-file substrate; GeoPackage container | rusqlite; PRAGMA integrity_check |
| 16 | GeoTIFF / COG | User-approved; the cloud-raster delivery standard | GDAL subprocess (gdal_translate -of COG); gdalinfo -json assert |
| 17 | GeoPackage 1.4 + GeoJSON | The sanctioned Shapefile successor + IETF lightweight vector | rusqlite + geozero (or GDAL); ogrinfo round-trip |
| 18 | JSON/JSONL, CSV, TOML | Trivial GREEN substrate lanes | serde_json, csv, toml |
| 19 | EXIF / XMP sidecars | Metadata layer across all image lanes | kamadak-exif; ExifTool oracle |
| 20 | KML | User-approved; trivial XML emit; carry the YELLOW adoption-flat note | templated XML; XML parse + geozero check |

### DEFERRED (implement on trigger)

- JPEG XL: implement decode now (jxl-oxide, pure Rust) + cjxl-subprocess encode; promote to CORE the day Chrome flips default (expected H2 2026).
- USD/USDZ: the scene-assembly lane; `openusd` 0.7.0 is pre-1.0; Blender's USD exporter is the bridge until it stabilizes.
- 3MF: take over STL whenever the consumer is a printing pipeline (pure-Rust `lib3mf`, conformance-tested).
- Zarr v3: the moment array outputs exceed RAM (`zarrs`, Zarr 3.1 conformant).
- HDF5 (+ NetCDF-4): user-approved pair; DEFERRED because stewardship is single-vendor and Rust is C-bindings-only; emit via subprocess tooling only when a consumer demands it, Parquet covers the actual need.
- EPUB 3.3, ODF (ODS/ODT), LaTeX emit, AsciiDoc: doc niches, all reachable through pandoc/LibreOffice at zero marginal cost; epubcheck gate if EPUB ships.
- EXR, KTX2, DDS, NPY/NPZ: art/GPU/ML niches; pure-Rust decode exists for all four.
- MP4 container (AV1/Opus/ALAC payloads), Ogg, Vorbis, MP3, ALAC: compatibility emits via ffmpeg.
- FFV1+MKV (archival), Y4M (internal encoder pipe), ASS subtitles (libass render), ALAC: specialist lanes.
- Protobuf/FlatBuffers/Cap'n Proto, YAML, xz: infrastructure picks, not deliverable formats.
- Alembic, OpenVDB, Draco, meshopt, MaterialX: reached through Blender/USD subprocesses when scenes need them.
- GPX, FlatGeobuf, MVT, LAS/LAZ/COPC, FITS: domain lanes with documented pure-Rust or GDAL paths (section 3.6).

### EXCLUDE (never generate)

- H.264/H.265 in any container; AAC generation (and never ship a libfdk_aac binary); HEIC/HEIF-with-HEVC; JPEG 2000; proprietary camera RAW; FLIF; JPEG LS/XR; DNG (skip; Adobe DNG Converter subprocess only if ever forced); QOI (no consumer in the plan).
- Theora (dead); "Filmux" (does not exist); DjVu (dormant, RED for generation); UOF (out of scope).
- FBX write (RED; ufbx read-only if a legacy corpus demands it); IGES (deprecated); COLLADA (dormant); legacy DOC/XLS/PPT generation (read via LibreOffice only); Shapefile generation (emit GeoPackage instead); GML (unless legally mandated).

### Reconciliation check (user-approved sets)

SVG -> CORE 3; glTF -> CORE 9; .blend -> CORE 10; PDF -> CORE 1; audio -> CORE 12 (Opus/FLAC/WAV); STEP -> CORE 11; GeoTIFF -> CORE 16; KML -> CORE 20; HDF5 -> DEFERRED with rationale (Parquet = CORE 14 covers the need); DOCX/XLSX/PPTX -> CORE 7. Evidence-based additions beyond the user sets: AVIF, WebP, JPEG, PNG (image lane); AV1/VP9 + Matroska/WebM (video); Markdown/HTML5; SQLite, JSON/CSV/TOML, Arrow IPC; GeoPackage, GeoJSON, WKT/WKB; EXIF/XMP sidecars.
