# pdf-adapter fixture fonts (todo M13)

`latin-cyrillic.ttf` and `cjk.otf` are fonttools subsets of SIL Open Font
License fonts, produced once at dev time by the M13 fixture builder (network
+ fonttools at BUILD-FIXTURE time only; tests stay hermetic):

- `latin-cyrillic.ttf`  <- Noto Sans (hinted ttf), subset to printable ASCII,
  Cyrillic (incl. Ё/ё) and common punctuation. Source license: SIL OFL 1.1.
- `cjk.otf`             <- Noto Sans SC (SubsetOTF, CFF outlines), subset to
  the CJK corpus text used by the M13 round-trip tests. SIL OFL 1.1.

SIL OFL 1.1 permits redistribution and subsetting of font software; the
reserved-name clause does not apply because these subsets keep the "Noto"
family naming only as internal name records, not as a competing family
release. Both files are embedded into the `austringer-media` `pdf` feature via
`include_bytes!` (the writer must ship real glyph coverage for its
non-Latin1 round-trip law; see `src/adapter/pdf_generate.rs`).

Upstream licenses: https://openfontlicense.org/ (Noto project,
https://github.com/notofonts). Full OFL text:
https://github.com/notofonts/notofonts.github.io/blob/main/fonts/LICENSE.
