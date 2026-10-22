//! The mock's hand-rolled little-endian classic TIFF reader: the IFD
//! chain walk (dims, strip/tile chunk table, geo tags, COG ghost
//! area) the `gdalinfo` mode and the baseline validation share.

use super::write::GHOST_COG;

// Reader-side tag numbers (TIFF 6.0 + OGC 19-075).
const TAG_WIDTH: u16 = 256;
const TAG_HEIGHT: u16 = 257;
const TAG_TILE_WIDTH: u16 = 322;
const TAG_STRIP_OFFSETS: u16 = 273;
const TAG_STRIP_COUNTS: u16 = 279;
const TAG_TILE_OFFSETS: u16 = 324;
const TAG_TILE_COUNTS: u16 = 325;
const TAG_PIXEL_SCALE: u16 = 33550;
const TAG_TIEPOINT: u16 = 33922;

const TYPE_SHORT: u16 = 3;
const TYPE_LONG: u16 = 4;
const TYPE_DOUBLE: u16 = 12;

pub(super) struct Geo {
    pub(super) origin: [f64; 2],
    pub(super) scale: [f64; 2],
}

pub(super) struct Ifd {
    pub(super) width: u32,
    pub(super) height: u32,
    strips: Vec<(u32, u32)>,
    pub(super) geo: Option<Geo>,
}

pub(super) struct Parsed {
    ifds: Vec<Ifd>,
    pub(super) cog_layout: bool,
}

impl Parsed {
    /// The main IFD: the largest raster (overviews are strictly
    /// smaller; ties keep the later entry).
    pub(super) fn main_ifd(&self) -> Option<&Ifd> {
        self.ifds
            .iter()
            .enumerate()
            .max_by_key(|(index, ifd)| (ifd.width * ifd.height, *index))
            .map(|(_, ifd)| ifd)
    }
}

impl Ifd {
    pub(super) fn pixel_bytes(&self, bytes: &[u8]) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        for &(offset, len) in &self.strips {
            let start = offset as usize;
            let end = start
                .checked_add(len as usize)
                .ok_or("data length overflows")?;
            if end > bytes.len() {
                return Err("raster data runs past the file".into());
            }
            out.extend_from_slice(&bytes[start..end]);
        }
        Ok(out)
    }
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    let slice = bytes.get(at..at.checked_add(2)?)?;
    Some(u16::from_le_bytes([slice[0], slice[1]]))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    let slice = bytes.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn f64_at(bytes: &[u8], at: usize) -> Option<f64> {
    let slice = bytes.get(at..at.checked_add(8)?)?;
    let mut buffer = [0_u8; 8];
    buffer.copy_from_slice(slice);
    Some(f64::from_le_bytes(buffer))
}

fn doubles_at(bytes: &[u8], at: usize, count: usize) -> Option<Vec<f64>> {
    (0..count)
        .map(|index| f64_at(bytes, at + 8 * index))
        .collect()
}

fn longs_at(bytes: &[u8], at: usize, count: usize) -> Option<Vec<u32>> {
    (0..count)
        .map(|index| u32_at(bytes, at + 4 * index))
        .collect()
}

fn type_size(kind: u16) -> usize {
    match kind {
        TYPE_SHORT => 2,
        TYPE_LONG => 4,
        TYPE_DOUBLE => 8,
        _ => 1,
    }
}

pub(super) fn parse_tiff(bytes: &[u8]) -> Result<Parsed, String> {
    if bytes.get(0..2) != Some(b"II") || u16_at(bytes, 2) != Some(0x2A) {
        return Err("not a little-endian classic tiff".into());
    }
    let cog_layout = bytes.get(8..8 + GHOST_COG.len()) == Some(GHOST_COG);
    let mut ifds = Vec::new();
    let mut next = u32_at(bytes, 4).ok_or("header has no first-IFD offset")?;
    while next != 0 {
        if ifds.len() >= 64 {
            return Err("IFD chain exceeds the 64-directory cap".into());
        }
        let at = next as usize;
        let count = u16_at(bytes, at).ok_or("truncated IFD")? as usize;
        let mut width = 0;
        let mut height = 0;
        let mut data_offsets: Option<Vec<u32>> = None;
        let mut data_lens: Option<Vec<u32>> = None;
        let mut scale = None;
        let mut tiepoint = None;
        for entry in 0..count {
            let base = at + 2 + entry * 12;
            let tag = u16_at(bytes, base).ok_or("truncated entry")?;
            let kind = u16_at(bytes, base + 2).ok_or("truncated entry")?;
            let value_count = u32_at(bytes, base + 4).ok_or("truncated entry")? as usize;
            let field = u32_at(bytes, base + 8).ok_or("truncated entry")?;
            let payload = if value_count * type_size(kind) <= 4 {
                base + 8
            } else {
                field as usize
            };
            match (tag, kind) {
                (TAG_WIDTH, TYPE_LONG | TYPE_SHORT) => width = field,
                (TAG_HEIGHT, TYPE_LONG | TYPE_SHORT) => height = field,
                (TAG_TILE_WIDTH, TYPE_LONG) => {}
                (TAG_STRIP_OFFSETS | TAG_TILE_OFFSETS, TYPE_LONG) => {
                    data_offsets = if value_count > 1 {
                        longs_at(bytes, field as usize, value_count)
                    } else {
                        Some(vec![field])
                    };
                }
                (TAG_STRIP_COUNTS | TAG_TILE_COUNTS, TYPE_LONG) => {
                    data_lens = if value_count > 1 {
                        longs_at(bytes, field as usize, value_count)
                    } else {
                        Some(vec![field])
                    };
                }
                (TAG_PIXEL_SCALE, TYPE_DOUBLE) => {
                    scale = doubles_at(bytes, payload, 3).map(|values| [values[0], values[1]]);
                }
                (TAG_TIEPOINT, TYPE_DOUBLE) => tiepoint = doubles_at(bytes, payload, 6),
                _ => {}
            }
        }
        let strips: Vec<(u32, u32)> = data_offsets
            .zip(data_lens)
            .map(|(offsets, lens)| offsets.into_iter().zip(lens).collect())
            .unwrap_or_default();
        let geo = scale.zip(tiepoint).map(|(scale, tiepoint)| Geo {
            origin: [tiepoint[3], tiepoint[4]],
            scale,
        });
        ifds.push(Ifd {
            width,
            height,
            strips,
            geo,
        });
        next = u32_at(bytes, at + 2 + count * 12).ok_or("truncated IFD tail")?;
    }
    Ok(Parsed { ifds, cog_layout })
}
