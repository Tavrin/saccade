//! Bounded unsigned XMP/IPTC fields. Free-form metadata stays data; GPS is opt-in.
use serde_json::{Value, json};
const LIMIT: usize = 1024 * 1024;
/// Parse known XMP properties from RDF attributes/elements, including common capture/edit tags.
/// Unknown properties and arbitrary nested packets are not exposed to avoid location leakage.
pub fn xmp(packet: &[u8], gps: bool) -> Value {
    if packet.len() > LIMIT {
        return json!({"status":"limit_exceeded"});
    }
    let Ok(text) = std::str::from_utf8(packet) else {
        return json!({"status":"invalid_utf8"});
    };
    let Ok(doc) = roxmltree::Document::parse(text) else {
        return json!({"status":"malformed"});
    };
    if doc.descendants().count() > 10000 {
        return json!({"status":"node_limit_exceeded"});
    }
    let mut fields = serde_json::Map::new();
    let allowed = |namespace: Option<&str>, name: &str| -> bool {
        match namespace {
            Some("http://ns.adobe.com/xap/1.0/") => matches!(
                name,
                "CreateDate" | "ModifyDate" | "MetadataDate" | "CreatorTool"
            ),
            Some("http://ns.adobe.com/tiff/1.0/") => {
                matches!(name, "Make" | "Model" | "Orientation")
            }
            Some("http://ns.adobe.com/exif/1.0/") => {
                matches!(
                    name,
                    "DateTimeOriginal"
                        | "DateTimeDigitized"
                        | "OffsetTimeOriginal"
                        | "LensModel"
                        | "ColorSpace"
                ) || gps
                    && matches!(
                        name,
                        "GPSLatitude"
                            | "GPSLongitude"
                            | "GPSAltitude"
                            | "GPSTimeStamp"
                            | "GPSDateStamp"
                    )
            }
            Some("http://ns.adobe.com/exif/1.0/aux/") => matches!(name, "Lens" | "LensInfo"),
            Some("http://ns.adobe.com/photoshop/1.0/") => {
                matches!(
                    name,
                    "DateCreated" | "ICCProfile" | "Credit" | "Source" | "Headline"
                )
            }
            Some("http://purl.org/dc/elements/1.1/") => {
                matches!(name, "creator" | "rights" | "description" | "subject")
            }
            _ => false,
        }
    };
    for node in doc.descendants().take(10000).filter(|n| n.is_element()) {
        if allowed(node.tag_name().namespace(), node.tag_name().name())
            && let Some(value) = node.text().filter(|s| s.len() <= 4096)
        {
            fields.insert(node.tag_name().name().into(), json!(value));
        }
        if allowed(node.tag_name().namespace(), node.tag_name().name())
            && node.children().any(|n| n.is_element())
        {
            let values: Vec<_> = node
                .descendants()
                .filter(|n| {
                    n.is_element()
                        && n.tag_name().namespace()
                            == Some("http://www.w3.org/1999/02/22-rdf-syntax-ns#")
                        && n.tag_name().name() == "li"
                })
                .filter_map(|n| n.text())
                .filter(|s| s.len() <= 4096)
                .take(256)
                .collect();
            if !values.is_empty() {
                fields.insert(node.tag_name().name().into(), json!(values));
            }
        }
        for attribute in node.attributes() {
            if allowed(attribute.namespace(), attribute.name()) && attribute.value().len() <= 4096 {
                fields.insert(attribute.name().into(), json!(attribute.value()));
            }
        }
    }
    json!({"status":"parsed_known_properties","fields":fields,"gps_policy":if gps{"included_if_present"}else{"withheld"},"sha256":crate::localized::digest(packet),"assurance":"unsigned metadata; namespace-checked whitelist; unknown fields omitted"})
}
/// Decode IPTC IIM records in Photoshop APP13 image-resource blocks.
/// Handles UTF-8 (coded charset ESC % G) and preserves repeated keywords; no location fields.
pub fn iptc(app13: &[u8]) -> Value {
    if app13.len() > LIMIT {
        return json!({"status":"limit_exceeded"});
    }
    let Some(mut at) = app13.starts_with(b"Photoshop 3.0\0").then_some(14) else {
        return json!({"status":"unsupported_app13"});
    };
    let mut datasets = Vec::new();
    while at + 7 <= app13.len() {
        if app13.get(at..at + 4) != Some(b"8BIM") {
            return json!({"status":"malformed_resources"});
        }
        let id = u16::from_be_bytes([app13[at + 4], app13[at + 5]]);
        at += 6;
        let len = usize::from(app13[at]);
        at += (len + 2) & !1;
        let Some(raw) = app13.get(at..at + 4) else {
            return json!({"status":"malformed_resources"});
        };
        let len = u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]) as usize;
        at += 4;
        let Some(data) = app13.get(at..at.saturating_add(len)) else {
            return json!({"status":"malformed_resources"});
        };
        if id == 0x0404 {
            datasets.push(data);
        }
        at = at.saturating_add(len + (len & 1));
    }
    let mut records = Vec::new();
    let mut utf8 = false;
    for data in datasets {
        let mut at = 0;
        while at + 5 <= data.len() {
            if data[at] != 0x1c {
                return json!({"status":"malformed_iim"});
            }
            let (record, tag) = (data[at + 1], data[at + 2]);
            let len = usize::from(u16::from_be_bytes([data[at + 3], data[at + 4]]));
            if len & 0x8000 != 0 {
                return json!({"status":"extended_iim_lengths_unsupported"});
            }
            at += 5;
            let Some(value) = data.get(at..at.saturating_add(len)) else {
                return json!({"status":"malformed_iim"});
            };
            at += len;
            if record == 1 && tag == 90 {
                utf8 = value == b"\x1b%G";
            }
            if record == 2
                && matches!(tag, 25 | 55 | 60 | 62 | 63 | 65 | 70 | 80 | 110 | 116 | 120)
                && value.len() <= 4096
            {
                records.push((tag, value.to_vec()));
                if records.len() > 256 {
                    return json!({"status":"record_limit"});
                }
            }
        }
        if at != data.len() {
            return json!({"status":"malformed_iim"});
        }
    }
    let records: Vec<_> = records.into_iter().map(|(tag,bytes)|json!({"dataset":tag,"text":if utf8{String::from_utf8_lossy(&bytes).into_owned()}else{bytes.iter().map(|&b|char::from(b)).collect::<String>()},"encoding":if utf8{"UTF-8"}else{"legacy bytes displayed as Latin-1; charset unqualified"}})).collect();
    json!({"status":"parsed_known_datasets","records":records,"location_fields":"withheld; unknown/free-form/location datasets not exposed","sha256":crate::localized::digest(app13),"assurance":"unsigned IPTC IIM metadata; dates/byline/copyright only"})
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn xmp_namespace_and_gps_privacy() {
        let packet=br#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description xmlns:exif="http://ns.adobe.com/exif/1.0/" xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmp:CreatorTool="editor" exif:GPSLatitude="48,0N" exif:GPSLongitude="2,0E"/></rdf:RDF></x:xmpmeta>"#;
        let redacted = xmp(packet, false);
        assert_eq!(redacted["fields"]["CreatorTool"], "editor");
        assert!(redacted["fields"].get("GPSLatitude").is_none());
        assert_eq!(xmp(packet, true)["fields"]["GPSLatitude"], "48,0N");
        assert_eq!(xmp(b"<broken", false)["status"], "malformed");
    }
    #[test]
    fn iptc_rejects_truncation_and_preserves_coded_utf8() {
        let mut iim = b"\x1c\x01\x5a\x00\x03\x1b%G".to_vec();
        let text = "Zoé".as_bytes();
        iim.extend_from_slice(&[0x1c, 2, 80, 0, text.len() as u8]);
        iim.extend_from_slice(text);
        let mut app = b"Photoshop 3.0\0".to_vec();
        app.extend_from_slice(b"8BIM\x04\x04\x00\x00");
        app.extend_from_slice(&(iim.len() as u32).to_be_bytes());
        app.extend_from_slice(&iim);
        if iim.len() % 2 == 1 {
            app.push(0);
        }
        assert_eq!(iptc(&app)["records"][0]["text"], "Zoé");
        app.truncate(app.len() - 3);
        assert_eq!(iptc(&app)["status"], "malformed_resources");
    }
}
