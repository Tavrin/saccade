//! Generate a local test-identity C2PA fixture and pinned tampered variants.
use std::{io::Cursor, path::PathBuf};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(std::env::var_os("SACCADE_C2PA_FIXTURE_DIR").ok_or(
        "set SACCADE_C2PA_FIXTURE_DIR to a directory containing certificate.pem and private-key.pem",
    )?);
    let cert = std::fs::read(root.join("certificate.pem"))?;
    let key = std::fs::read(root.join("private-key.pem"))?;
    let signer = c2pa::create_signer::from_keys(&cert, &key, c2pa::SigningAlg::Es256, None)?;
    let image = image::RgbImage::from_fn(64, 64, |x, y| {
        image::Rgb([(x * 3) as u8, (y * 3) as u8, 80])
    });
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 95).encode_image(&image)?;
    let mut builder = c2pa::Builder::from_context(c2pa::Context::new()).with_definition(
        r#"{"title":"Generated local test image; no real identity","claim_generator_info":[{"name":"saccade qualification fixture"}],"assertions":[{"label":"c2pa.actions","data":{"actions":[{"action":"c2pa.created","digitalSourceType":"http://cv.iptc.org/newscodes/digitalsourcetype/algorithmicMedia"}]}}]}"#,
    )?;
    let mut signed = Cursor::new(Vec::new());
    builder.sign(&*signer, "image/jpeg", &mut Cursor::new(bytes), &mut signed)?;
    let signed = signed.into_inner();
    std::fs::write(root.join("signed.jpg"), &signed)?;
    let mut stripped = Vec::new();
    image::codecs::jpeg::JpegEncoder::new(&mut stripped).encode_image(&image)?;
    std::fs::write(root.join("stripped.jpg"), &stripped)?;
    let mut claims = Vec::new();
    let mut at = 2;
    while at + 4 <= signed.len() && signed[at] == 0xff {
        let marker = signed[at + 1];
        if marker == 0xda || marker == 0xd9 {
            break;
        }
        let n = usize::from(u16::from_be_bytes([signed[at + 2], signed[at + 3]]));
        if n < 2 || at + 2 + n > signed.len() {
            return Err("malformed generated JPEG".into());
        }
        if marker == 0xeb {
            claims.extend_from_slice(&signed[at..at + 2 + n]);
        }
        at += 2 + n;
    }
    let mut transplant = vec![0xff, 0xd8];
    transplant.extend(claims);
    transplant.extend_from_slice(&stripped[2..]);
    std::fs::write(root.join("transplant.jpg"), &transplant)?;
    let mut pins = serde_json::Map::new();
    for name in [
        "signed.jpg",
        "stripped.jpg",
        "transplant.jpg",
        "certificate.pem",
    ] {
        let b = std::fs::read(root.join(name))?;
        pins.insert(
            name.into(),
            serde_json::json!({"sha256":saccade_core::localized::digest(&b),"bytes":b.len()}),
        );
    }
    std::fs::write(
        root.join("pins.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"generator":"c2pa 0.90.22; local ephemeral self-signed test certificate; no real identity","network":false,"files":pins}),
        )?,
    )?;
    Ok(())
}
