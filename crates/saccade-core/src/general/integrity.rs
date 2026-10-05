//! Bounded metadata/current JPEG quantisation evidence; never an authenticity classifier.
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
/// Single-image evidence schema.
pub const SCHEMA: &str = "saccade-inspect-image.v1";
fn invalid(message: &str) -> crate::Error {
    crate::Error::Config(message.into())
}
fn be16(b: &[u8]) -> Option<u16> {
    Some(u16::from_be_bytes([*b.first()?, *b.get(1)?]))
}
fn be32(b: &[u8]) -> Option<u32> {
    Some(u32::from_be_bytes([
        *b.first()?,
        *b.get(1)?,
        *b.get(2)?,
        *b.get(3)?,
    ]))
}
type Entries = BTreeMap<u16, (u16, Vec<u8>)>;
struct Tiff<'a> {
    bytes: &'a [u8],
    little: bool,
}
impl<'a> Tiff<'a> {
    fn new(bytes: &'a [u8]) -> Option<Self> {
        let little = match bytes.get(..2)? {
            b"II" => true,
            b"MM" => false,
            _ => return None,
        };
        let t = Self { bytes, little };
        (t.u16(2)? == 42).then_some(t)
    }
    fn u16(&self, at: usize) -> Option<u16> {
        let b = self.bytes.get(at..at.checked_add(2)?)?;
        Some(if self.little {
            u16::from_le_bytes([b[0], b[1]])
        } else {
            u16::from_be_bytes([b[0], b[1]])
        })
    }
    fn u32(&self, at: usize) -> Option<u32> {
        let b = self.bytes.get(at..at.checked_add(4)?)?;
        Some(if self.little {
            u32::from_le_bytes([b[0], b[1], b[2], b[3]])
        } else {
            u32::from_be_bytes([b[0], b[1], b[2], b[3]])
        })
    }
    fn entries(&self, offset: usize) -> Option<(Entries, usize)> {
        let count = usize::from(self.u16(offset)?);
        if count > 256 {
            return None;
        }
        let mut entries = BTreeMap::new();
        for i in 0..count {
            let at = offset.checked_add(2 + i * 12)?;
            let tag = self.u16(at)?;
            let kind = self.u16(at + 2)?;
            let n = self.u32(at + 4)? as usize;
            let unit = match kind {
                1 | 2 | 7 => 1,
                3 => 2,
                4 | 9 => 4,
                5 | 10 => 8,
                _ => continue,
            };
            let len = n.checked_mul(unit)?;
            if len > 4096 {
                continue;
            }
            let start = if len <= 4 {
                at + 8
            } else {
                self.u32(at + 8)? as usize
            };
            let data = self.bytes.get(start..start.checked_add(len)?)?;
            entries.insert(tag, (kind, data.to_vec()));
        }
        Some((entries, self.u32(offset + 2 + count * 12)? as usize))
    }
    fn scalar(&self, value: &(u16, Vec<u8>)) -> Option<u32> {
        let t = Tiff {
            bytes: &value.1,
            little: self.little,
        };
        match value.0 {
            3 => t.u16(0).map(u32::from),
            4 => t.u32(0),
            _ => None,
        }
    }
    fn ascii(value: &(u16, Vec<u8>)) -> Option<String> {
        (value.0 == 2).then(|| {
            String::from_utf8_lossy(&value.1)
                .trim_end_matches('\0')
                .trim()
                .to_owned()
        })
    }
    fn coordinate(&self, value: &(u16, Vec<u8>)) -> Option<f64> {
        if value.0 != 5 || value.1.len() != 24 {
            return None;
        }
        let t = Tiff {
            bytes: &value.1,
            little: self.little,
        };
        let mut parts = [0.; 3];
        for (i, p) in parts.iter_mut().enumerate() {
            let denominator = t.u32(i * 8 + 4)?;
            if denominator == 0 {
                return None;
            }
            *p = f64::from(t.u32(i * 8)?) / f64::from(denominator);
        }
        if parts[1] >= 60. || parts[2] >= 60. {
            return None;
        }
        Some(parts[0] + parts[1] / 60. + parts[2] / 3600.)
    }
}
#[allow(clippy::collapsible_if)]
fn exif(bytes: &[u8], gps: bool, main: &image::RgbaImage) -> Option<Value> {
    let t = Tiff::new(bytes)?;
    let (root, next) = t.entries(t.u32(4)? as usize)?;
    let mut out = serde_json::Map::new();
    let mut dirs = vec![root.clone()];
    let mut seen = BTreeSet::new();
    if let Some(offset) = root.get(&0x8769).and_then(|v| t.scalar(v)) {
        if seen.insert(offset) {
            dirs.push(t.entries(offset as usize)?.0);
        }
    }
    let keys = [
        (0x013b, "creator"),
        (0x8298, "copyright"),
        (0x010e, "caption"),
        (0x010f, "camera_make"),
        (0x0110, "camera_model"),
        (0x0131, "software"),
        (0x0132, "metadata_time"),
        (0x9003, "capture_time"),
        (0x9004, "digitized_time"),
        (0x9010, "metadata_timezone"),
        (0x9011, "capture_timezone"),
        (0x9012, "digitized_timezone"),
        (0xa434, "lens_model"),
    ];
    for dir in &dirs {
        for (tag, name) in keys {
            if let Some(text) = dir.get(&tag).and_then(Tiff::ascii) {
                out.insert(name.into(), json!(text));
            }
        }
    }
    if let Some(orientation) = root.get(&0x0112).and_then(|v| t.scalar(v)) {
        out.insert("orientation".into(), json!(orientation));
    }
    let gps_offset = root.get(&0x8825).and_then(|v| t.scalar(v));
    out.insert(
        "gps_status".into(),
        json!(if gps_offset.is_none() {
            "absent"
        } else if gps {
            "requested"
        } else {
            "withheld"
        }),
    );
    if gps {
        if let Some((dir, _)) = gps_offset.and_then(|v| t.entries(v as usize)) {
            let coordinate = |tag: u16, reference: u16, negative: &str, max: f64| -> Option<f64> {
                let mut v = t.coordinate(dir.get(&tag)?)?;
                let r = Tiff::ascii(dir.get(&reference)?)?;
                if !(if negative == "S" {
                    ["N", "S"]
                } else {
                    ["E", "W"]
                })
                .contains(&r.as_str())
                    || v > max
                {
                    return None;
                }
                if r == negative {
                    v = -v;
                }
                Some(v)
            };
            if let (Some(lat), Some(lon)) =
                (coordinate(2, 1, "S", 90.), coordinate(4, 3, "W", 180.))
            {
                out.insert(
                    "gps".into(),
                    json!({"latitude":lat,"longitude":lon,"authority":"unsigned metadata"}),
                );
            }
        }
    }
    if next != 0 {
        if let Some((thumbnail, _)) = t.entries(next) {
            if let (Some(offset), Some(length)) = (
                thumbnail.get(&0x0201).and_then(|v| t.scalar(v)),
                thumbnail.get(&0x0202).and_then(|v| t.scalar(v)),
            ) {
                if let Some(data) =
                    bytes.get(offset as usize..(offset as usize).checked_add(length as usize)?)
                {
                    let thumb = super::input::decode(data).ok();
                    let hamming = thumb.as_ref().map(|thumb| {
                        (super::hashing::hash(thumb).phash ^ super::hashing::hash(main).phash)
                            .count_ones()
                    });
                    out.insert("thumbnail".into(),json!({"bytes":data.len(),"sha256":crate::localized::digest(data),"phash_hamming":hamming,"possible_mismatch":hamming.map(|h|h>24),"limits":"unsigned thumbnail may legitimately be cropped, differently processed or outdated; hash disagreement is not tampering proof"}));
                }
            }
        }
    }
    Some(Value::Object(out))
}
// Conventional Annex K luminance coefficients, as recorded in image 0.25.9's encoder source.
const BASE: [u16; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69, 56,
    14, 17, 22, 29, 51, 87, 80, 62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81, 104, 113,
    92, 49, 64, 78, 87, 103, 121, 120, 101, 72, 92, 95, 98, 112, 100, 103, 99,
];
fn conventional_quality(table: &[u16]) -> Option<u32> {
    if table.len() != 64 {
        return None;
    }
    let mut order = Vec::new();
    for diagonal in 0usize..15 {
        let mut ys: Vec<_> = (0..8)
            .filter(|&y| diagonal >= y && diagonal - y < 8)
            .collect();
        if diagonal % 2 == 0 {
            ys.reverse();
        }
        for y in ys {
            order.push(y * 8 + diagonal - y);
        }
    }
    (1u32..=100).find(|&quality| {
        let scale = if quality < 50 {
            5000 / quality
        } else {
            200 - 2 * quality
        };
        order.iter().zip(table).all(|(&at, &value)| {
            ((u32::from(BASE[at]) * scale + 50) / 100).clamp(1, 255) == u32::from(value)
        })
    })
}
/// Extracts bounded EXIF, container presence and current JPEG quantisation tables.
/// XMP/IPTC known fields are decoded; top-level credentials separately validates C2PA.
/// GPS coordinates are never returned unless explicitly requested.
#[allow(clippy::collapsible_if)]
pub fn headers(bytes: &[u8], main: &image::RgbaImage, include_gps: bool) -> crate::Result<Value> {
    if bytes.len() as u64 > super::input::MAX_BYTES {
        return Err(invalid("metadata byte limit exceeded"));
    }
    let mut metadata = json!({"exif":null,"xmp":{"status":"absent"},"iptc":{"status":"absent"},"colour_profile":{"status":"absent"}});
    let mut tables = Vec::new();
    let mut jumbf = false;
    let mut kind = "other";
    if bytes.starts_with(&[0xff, 0xd8]) {
        kind = "jpeg";
        let mut at = 2;
        let mut segments = 0;
        while at < bytes.len() {
            segments += 1;
            if segments > 10000 {
                return Err(invalid("JPEG marker limit exceeded"));
            }
            if bytes[at] != 0xff {
                return Err(invalid("invalid JPEG header marker"));
            }
            while bytes.get(at) == Some(&0xff) {
                at += 1;
            }
            let marker = *bytes
                .get(at)
                .ok_or_else(|| invalid("truncated JPEG marker"))?;
            at += 1;
            if marker == 0xda || marker == 0xd9 {
                break;
            }
            if marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
                continue;
            }
            let length = usize::from(
                be16(
                    bytes
                        .get(at..)
                        .ok_or_else(|| invalid("JPEG length unavailable"))?,
                )
                .ok_or_else(|| invalid("truncated JPEG length"))?,
            );
            if length < 2 {
                return Err(invalid("invalid JPEG segment length"));
            }
            let end = at
                .checked_add(length)
                .ok_or_else(|| invalid("JPEG length overflow"))?;
            let data = bytes
                .get(at + 2..end)
                .ok_or_else(|| invalid("truncated JPEG segment"))?;
            at = end;
            match marker {
                0xe1 if data.starts_with(b"Exif\0\0") => {
                    metadata["exif"] = exif(&data[6..], include_gps, main)
                        .unwrap_or_else(|| json!({"status":"malformed_or_unsupported"}));
                }
                0xe1 if data.starts_with(b"http://ns.adobe.com/xap/1.0/\0") => {
                    metadata["xmp"] = super::metadata::xmp(
                        &data[b"http://ns.adobe.com/xap/1.0/\0".len()..],
                        include_gps,
                    )
                }
                0xed => metadata["iptc"] = super::metadata::iptc(data),
                0xe2 if data.starts_with(b"ICC_PROFILE\0") => {
                    metadata["colour_profile"] = json!({"status":"icc_segment_present_unvalidated","bytes":data.len(),"sha256":crate::localized::digest(data)})
                }
                0xeb if data.starts_with(b"JP") => jumbf = true,
                0xdb => {
                    let mut q = 0;
                    while q < data.len() {
                        let info = data[q];
                        q += 1;
                        let precision = info >> 4;
                        let id = info & 15;
                        if precision > 1 || id > 3 {
                            return Err(invalid("invalid JPEG quantisation table"));
                        }
                        let size = 64 * (usize::from(precision) + 1);
                        let values = data
                            .get(q..q + size)
                            .ok_or_else(|| invalid("truncated JPEG quantisation table"))?;
                        q += size;
                        let values: Vec<u16> = if precision == 0 {
                            values.iter().map(|&v| u16::from(v)).collect()
                        } else {
                            values
                                .as_chunks::<2>()
                                .0
                                .iter()
                                .map(|v| u16::from_be_bytes(*v))
                                .collect()
                        };
                        if values.contains(&0) {
                            return Err(invalid("zero JPEG quantisation coefficient"));
                        }
                        if tables.len() >= 16 {
                            return Err(invalid("too many JPEG quantisation definitions"));
                        }
                        tables.push(json!({"id":id,"precision_bits":if precision==0{8}else{16},"zigzag_values":values,"conventional_luminance_quality":if id==0{conventional_quality(&values)}else{None},"encoder_identity":"unknown; quantisation compatibility does not identify an encoder"}));
                    }
                }
                _ => {}
            }
        }
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        kind = "png";
        let mut at = 8;
        let mut chunks = 0;
        while at + 12 <= bytes.len() {
            chunks += 1;
            if chunks > 10000 {
                return Err(invalid("PNG chunk limit exceeded"));
            }
            let length = be32(&bytes[at..at + 4]).ok_or_else(|| invalid("PNG length"))? as usize;
            let end = at
                .checked_add(12)
                .and_then(|v| v.checked_add(length))
                .ok_or_else(|| invalid("PNG chunk overflow"))?;
            let data = bytes
                .get(at + 8..at + 8 + length)
                .ok_or_else(|| invalid("truncated PNG chunk"))?;
            let tag = &bytes[at + 4..at + 8];
            if tag == b"eXIf" {
                metadata["exif"] = exif(data, include_gps, main)
                    .unwrap_or_else(|| json!({"status":"malformed_or_unsupported"}));
            }
            if tag == b"iCCP" {
                metadata["colour_profile"] = json!({"status":"compressed_icc_present_unvalidated","bytes":length,"sha256":crate::localized::digest(data)});
            }
            if tag == b"iTXt" && data.starts_with(b"XML:com.adobe.xmp\0") {
                let rest = &data[b"XML:com.adobe.xmp\0".len()..];
                metadata["xmp"] = if rest.len() >= 2 && rest[0] == 0 && rest[1] == 0 {
                    let mut at = 2;
                    for _ in 0..2 {
                        if let Some(n) = rest[at..].iter().position(|&b| b == 0) {
                            at += n + 1;
                        } else {
                            at = rest.len();
                            break;
                        }
                    }
                    super::metadata::xmp(&rest[at..], include_gps)
                } else {
                    json!({"status":"compressed_xmp_unsupported"})
                };
            }
            if tag == b"tEXt" && data.starts_with(b"Software\0") {
                metadata["png_software"] =
                    json!(String::from_utf8_lossy(&data[9..data.len().min(4105)]));
            }
            at = end;
            if tag == b"IEND" {
                break;
            }
        }
    }
    let mut consistency = Vec::new();
    let e = &metadata["exif"];
    if let Some(software) = e["software"]
        .as_str()
        .or_else(|| metadata["png_software"].as_str())
    {
        let lower = software.to_lowercase();
        if ["editor", "photoshop", "gimp", "paint", "affinity"]
            .iter()
            .any(|s| lower.contains(s))
        {
            consistency.push(json!({"indicator":"editor_software_tag","observed":software,"can_show":"unsigned metadata names editing software","cannot_show":"does not prove the image is misleading; tags may be absent or forged"}));
        }
    }
    if e["capture_timezone"] == e["digitized_timezone"] && e["capture_timezone"].is_string() {
        if let (Some(capture), Some(digitized)) =
            (e["capture_time"].as_str(), e["digitized_time"].as_str())
        {
            if capture.len() == 19 && digitized.len() == 19 && digitized < capture {
                consistency.push(json!({"indicator":"digitization_precedes_capture","capture":capture,"digitized":digitized,"timezone":e["capture_timezone"],"can_show":"metadata chronology conflict under identical declared timezones","cannot_show":"tags and clocks may be wrong; no manipulation conclusion"}));
            }
        }
    }
    Ok(
        json!({"container":kind,"metadata":metadata,"consistency":consistency,"credentials":{"status":"header_container_observation","jumbf_app11_present":jumbf,"ai_generation":"unknown","can_show":"only header-level JUMBF container presence","cannot_show":"container presence does not validate a credential; see top-level credentials"},"compression":{"quantisation_tables":tables,"scope":"header before first JPEG scan","double_compression":{"status":"unavailable","reason":"DCT history detector and constructed qualification not implemented"},"resampling":{"status":"unavailable","reason":"frequency/resampling detector and constructed qualification not implemented"},"can_show":"current JPEG tables and exact conventional luminance scaling compatibility","cannot_show":"cannot establish original encoder, number of encodes, authenticity or generation source"},"limits":["EXIF is unsigned data; only listed camera/time/lens/orientation/GPS tags are decoded","XMP/IPTC known fields parsed; extended/compressed packets and unknown properties remain unavailable","ICC presence/hash does not validate colour profile contents","metadata cannot establish authenticity or image age"]}),
    )
}
#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn jpeg_tables_report_current_quality_without_encoder_claim() {
        let image = image::RgbImage::from_pixel(32, 32, image::Rgb([50, 70, 90]));
        let mut bytes = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 83)
            .encode_image(&image)
            .expect("encode");
        let rgba = image::DynamicImage::ImageRgb8(image).to_rgba8();
        let report = headers(&bytes, &rgba, false).expect("headers");
        assert_eq!(
            report["compression"]["quantisation_tables"][0]["conventional_luminance_quality"],
            83
        );
        assert_eq!(report["credentials"]["ai_generation"], "unknown");
    }
    #[test]
    fn malformed_segment_lengths_fail_closed() {
        let image = image::RgbaImage::new(16, 16);
        assert!(headers(&[0xff, 0xd8, 0xff, 0xdb, 0, 20, 0], &image, false).is_err());
    }
    #[test]
    fn exif_gps_is_opt_in() {
        let mut tiff = vec![0u8; 128];
        tiff[..8].copy_from_slice(&[b'I', b'I', 42, 0, 8, 0, 0, 0]);
        tiff[8..10].copy_from_slice(&1u16.to_le_bytes());
        tiff[10..12].copy_from_slice(&0x8825u16.to_le_bytes());
        tiff[12..14].copy_from_slice(&4u16.to_le_bytes());
        tiff[14..18].copy_from_slice(&1u32.to_le_bytes());
        tiff[18..22].copy_from_slice(&30u32.to_le_bytes());
        let image = image::RgbaImage::new(16, 16);
        let report = exif(&tiff, false, &image).expect("exif");
        assert_eq!(report["gps_status"], "withheld");
        assert!(report.get("gps").is_none());
    }
}
