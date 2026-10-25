//! The mock's hand-rolled little-endian classic TIFF writer: the IFD
//! chain (overviews first, smallest leading - the COG layout; geotags
//! only on the main IFD), per-tile chunked storage with zero-padded
//! edge tiles, and out-of-line LONG arrays per the TIFF 6.0
//! value/offset rule.

pub(super) const TILE: u32 = 512;
pub(super) const GHOST_COG: &[u8] = b"GDAL_STRUCTURAL_METADATA\nLAYOUT=COG\n \n";

// Writer-side tag numbers (TIFF 6.0 + OGC 19-075).
const TAG_WIDTH: u16 = 256;
const TAG_HEIGHT: u16 = 257;
const TAG_BITS: u16 = 258;
const TAG_COMPRESSION: u16 = 259;
const TAG_PHOTOMETRIC: u16 = 262;
const TAG_STRIP_OFFSETS: u16 = 273;
const TAG_SAMPLES: u16 = 277;
const TAG_ROWS_PER_STRIP: u16 = 278;
const TAG_STRIP_COUNTS: u16 = 279;
const TAG_TILE_WIDTH: u16 = 322;
const TAG_TILE_LENGTH: u16 = 323;
const TAG_TILE_OFFSETS: u16 = 324;
const TAG_TILE_COUNTS: u16 = 325;
const TAG_PIXEL_SCALE: u16 = 33550;
const TAG_TIEPOINT: u16 = 33922;
const TAG_GEOKEYS: u16 = 34735;

const TYPE_SHORT: u16 = 3;
const TYPE_LONG: u16 = 4;
const TYPE_DOUBLE: u16 = 12;

fn push_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_f64(out: &mut Vec<u8>, value: f64) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// IFD-entry field for a LONG array: a single value inlines into the
/// 4-byte field, more write the array out-of-line and return its
/// offset (the TIFF 6.0 value/offset rule).
fn long_array_field(out: &mut Vec<u8>, values: &[u32]) -> u32 {
    if values.len() == 1 {
        return values[0];
    }
    let at = out.len() as u32;
    for value in values {
        push_u32(out, *value);
    }
    at
}

/// One image's stored chunks: a single strip, or TILE x TILE tiles
/// (edge tiles zero-padded - GDAL's own edge handling).
fn tile_chunks(width: u32, height: u32, pixels: &[u8]) -> Vec<Vec<u8>> {
    let across = width.div_ceil(TILE);
    let down = height.div_ceil(TILE);
    let mut chunks = Vec::new();
    for tile_row in 0..down {
        for tile_col in 0..across {
            let mut tile = vec![0_u8; (TILE * TILE) as usize];
            for row in 0..TILE {
                let y = tile_row * TILE + row;
                if y >= height {
                    break;
                }
                for col in 0..TILE {
                    let x = tile_col * TILE + col;
                    if x >= width {
                        break;
                    }
                    tile[(row * TILE + col) as usize] = pixels[(y * width + x) as usize];
                }
            }
            chunks.push(tile);
        }
    }
    chunks
}

/// Write the IFD chain: overview images first (smallest leading, the
/// COG layout) then the main image; geotags land only on the main IFD.
pub(super) fn write_tiff(
    images: &[(u32, u32, Vec<u8>)],
    geo: ([f64; 2], [f64; 2], u32),
    cog: bool,
) -> Vec<u8> {
    let (origin, scale, epsg) = geo;
    let main_index = images.len() - 1;
    let mut out = Vec::new();
    out.extend_from_slice(b"II\x2A\x00");
    push_u32(&mut out, 0);
    if cog {
        out.extend_from_slice(GHOST_COG);
    }
    let mut chunk_table = Vec::new();
    for (index, &(width, height, ref pixels)) in images.iter().enumerate() {
        let chunks: Vec<Vec<u8>> = if cog && index == main_index {
            tile_chunks(width, height, pixels)
        } else {
            vec![pixels.clone()]
        };
        let mut offsets = Vec::new();
        for chunk in &chunks {
            offsets.push(out.len() as u32);
            out.extend(chunk.iter().copied());
        }
        chunk_table.push(offsets);
    }
    let pixel_scale_at = out.len() as u32;
    for value in [scale[0], scale[1], 0.0] {
        push_f64(&mut out, value);
    }
    let tiepoint_at = out.len() as u32;
    for value in [0.0, 0.0, 0.0, origin[0], origin[1], 0.0] {
        push_f64(&mut out, value);
    }
    // GeoKey directory (OGC 19-075): header {1,1,0,3} then model type,
    // raster type (pixel-is-area), and the EPSG-coded CRS key.
    let model: u16 = if (4000..=4999).contains(&epsg) { 2 } else { 1 };
    let crs_key: u16 = if model == 2 { 2048 } else { 3072 };
    let geokeys: Vec<u16> = vec![
        1,
        1,
        0,
        3,
        1024,
        0,
        1,
        model,
        1025,
        0,
        1,
        1,
        crs_key,
        0,
        1,
        epsg as u16,
    ];
    let geokeys_at = out.len() as u32;
    for value in &geokeys {
        push_u16(&mut out, *value);
    }
    let mut tables = Vec::new();
    for (index, &(width, height, _)) in images.iter().enumerate() {
        let offsets = &chunk_table[index];
        let mut entries: Vec<(u16, u16, u32, u32)> = vec![
            (TAG_WIDTH, TYPE_LONG, 1, width),
            (TAG_HEIGHT, TYPE_LONG, 1, height),
            (TAG_BITS, TYPE_SHORT, 1, 8),
            (TAG_COMPRESSION, TYPE_SHORT, 1, 1),
            (TAG_PHOTOMETRIC, TYPE_SHORT, 1, 1),
            (TAG_SAMPLES, TYPE_SHORT, 1, 1),
        ];
        if cog && index == main_index {
            let counts: Vec<u32> = offsets.iter().map(|_| TILE * TILE).collect();
            let offsets_field = long_array_field(&mut out, offsets);
            let counts_field = long_array_field(&mut out, &counts);
            entries.extend([
                (TAG_TILE_WIDTH, TYPE_LONG, 1, TILE),
                (TAG_TILE_LENGTH, TYPE_LONG, 1, TILE),
                (
                    TAG_TILE_OFFSETS,
                    TYPE_LONG,
                    offsets.len() as u32,
                    offsets_field,
                ),
                (
                    TAG_TILE_COUNTS,
                    TYPE_LONG,
                    counts.len() as u32,
                    counts_field,
                ),
            ]);
        } else {
            entries.extend([
                (TAG_STRIP_OFFSETS, TYPE_LONG, 1, offsets[0]),
                (TAG_ROWS_PER_STRIP, TYPE_LONG, 1, height),
                (TAG_STRIP_COUNTS, TYPE_LONG, 1, width * height),
            ]);
        }
        if index == main_index {
            entries.extend([
                (TAG_PIXEL_SCALE, TYPE_DOUBLE, 3, pixel_scale_at),
                (TAG_TIEPOINT, TYPE_DOUBLE, 6, tiepoint_at),
                (TAG_GEOKEYS, TYPE_SHORT, geokeys.len() as u32, geokeys_at),
            ]);
        }
        entries.sort_by_key(|(tag, _, _, _)| *tag);
        tables.push(entries);
    }
    let mut next_fields = Vec::new();
    for (ordinal, entries) in tables.iter().enumerate() {
        let table_at = out.len() as u32;
        if ordinal == 0 {
            out[4..8].copy_from_slice(&table_at.to_le_bytes());
        } else {
            let position = next_fields[ordinal - 1];
            out[position..position + 4].copy_from_slice(&table_at.to_le_bytes());
        }
        push_u16(&mut out, entries.len() as u16);
        for (tag, kind, count, value) in entries {
            push_u16(&mut out, *tag);
            push_u16(&mut out, *kind);
            push_u32(&mut out, *count);
            // The TIFF 6.0 value/offset rule is about SIZE, not type: a
            // single SHORT inlines into the low half; anything larger
            // (LONG inline or an out-of-line array offset) occupies the
            // full 4-byte field.
            if *kind == TYPE_SHORT && *count == 1 {
                push_u16(&mut out, *value as u16);
                push_u16(&mut out, 0);
            } else {
                push_u32(&mut out, *value);
            }
        }
        next_fields.push(out.len());
        push_u32(&mut out, 0);
    }
    if out.len() % 2 == 1 {
        out.push(0);
    }
    out
}
