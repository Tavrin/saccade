//! Bounded perceptual-target delivery search behind generic transform adapters.
use crate::{agent::CliError, product_io as io, sweep_cmd::pattern};
use clap::{Args, Subcommand};
use image::{GenericImageView, ImageEncoder};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub(crate) const AUDIT_SCHEMA: &str = "saccade-imgtune-audit.v1";
pub(crate) const INPUT_SCHEMA: &str = "saccade-imgtune.v1";
pub(crate) const SEARCH_SCHEMA: &str = "saccade-imgtune-search.v1";
#[derive(Args)]
pub(crate) struct ImgtuneArgs {
    #[command(subcommand)]
    operation: Operation,
}
#[derive(Subcommand)]
enum Operation {
    /// Record actual HTTP content negotiation, bytes and decoded dimensions.
    Audit {
        #[arg(long)]
        urls: PathBuf,
        #[arg(long, required = true)]
        accept: Vec<String>,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Search a finite quality/format grid, retaining source and delivery evidence.
    Search {
        manifest: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        json: bool,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Parameters {
    pub(crate) width: u32,
    pub(crate) format: String,
    pub(crate) quality: u8,
}
pub(crate) struct Encoded {
    pub(crate) bytes: Vec<u8>,
    pub(crate) content_type: String,
}
/// A transform returns exact served bytes and the transport's declared content type.
pub(crate) trait TransformAdapter {
    fn transform(
        &self,
        source: &[u8],
        source_url: &str,
        parameters: &Parameters,
    ) -> Result<Encoded, CliError>;
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Adapter {
    Local,
    UrlTemplate { template: String, accept: String },
}
impl TransformAdapter for Adapter {
    fn transform(
        &self,
        source: &[u8],
        source_url: &str,
        p: &Parameters,
    ) -> Result<Encoded, CliError> {
        match self {
            Self::Local => {
                let image = io::image(source)?;
                let resized = resize(&image, p.width)?;
                let mut bytes = Vec::new();
                let content_type = match p.format.as_str() {
                    "jpeg" => {
                        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, p.quality)
                            .encode_image(&resized)
                            .map_err(|_| CliError::new("encode", "JPEG encoding failed"))?;
                        "image/jpeg"
                    }
                    "webp" => {
                        image::codecs::webp::WebPEncoder::new_lossless(&mut bytes)
                            .write_image(
                                resized.as_raw(),
                                resized.width(),
                                resized.height(),
                                image::ExtendedColorType::Rgb8,
                            )
                            .map_err(|_| CliError::new("encode", "WebP encoding failed"))?;
                        "image/webp"
                    }
                    "avif" => {
                        #[cfg(feature = "imgtune-avif")]
                        {
                            image::codecs::avif::AvifEncoder::new_with_speed_quality(
                                &mut bytes, 8, p.quality,
                            )
                            .with_num_threads(Some(1))
                            .write_image(
                                resized.as_raw(),
                                resized.width(),
                                resized.height(),
                                image::ExtendedColorType::Rgb8,
                            )
                            .map_err(|_| CliError::new("encode", "AVIF encoding failed"))?;
                            "image/avif"
                        }
                        #[cfg(not(feature = "imgtune-avif"))]
                        {
                            return Err(CliError::new(
                                "feature_unavailable",
                                "local AVIF requires imgtune-avif (ravif/rav1e encoder and dav1d decoder)",
                            ));
                        }
                    }
                    _ => return Err(CliError::usage("local format must be jpeg, webp or avif")),
                };
                Ok(Encoded {
                    bytes,
                    content_type: content_type.into(),
                })
            }
            Self::UrlTemplate { template, accept } => {
                let source = io::url(source_url)?;
                let base = source.origin().ascii_serialization();
                let target = template
                    .replace("{base}", &base)
                    .replace("{path}", source.path())
                    .replace("{w}", &p.width.to_string())
                    .replace("{fm}", &p.format)
                    .replace("{q}", &p.quality.to_string());
                if target.contains('{') || target.contains('}') {
                    return Err(CliError::usage("unknown URL template placeholder"));
                }
                let response = io::get(&target, &[("Accept", accept)])?;
                Ok(Encoded {
                    bytes: response.bytes,
                    content_type: response.content_type,
                })
            }
        }
    }
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    pub(crate) id: String,
    pub(crate) source: String,
    pub(crate) current: String,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Search {
    pub(crate) schema: String,
    pub(crate) images: Vec<Input>,
    pub(crate) widths: Vec<u32>,
    pub(crate) formats: Vec<String>,
    pub(crate) qualities: Vec<u8>,
    pub(crate) target_score: f64,
    pub(crate) butteraugli_ceiling: Option<f64>,
    pub(crate) adapter: Adapter,
}
fn resize(image: &image::DynamicImage, width: u32) -> Result<image::RgbImage, CliError> {
    if !(8..=16384).contains(&width) || image.width() == 0 {
        return Err(CliError::usage("width must be 8..16384"));
    }
    let height = ((image.height() as u64 * width as u64) / image.width() as u64).max(1);
    if !(8..=16384).contains(&height) || height * width as u64 > 64 * 1024 * 1024 {
        return Err(CliError::new(
            "input_budget",
            "resized image outside scoring limits",
        ));
    }
    if image.color().has_alpha() {
        return Err(CliError::usage(
            "normalize alpha against a declared background before tuning",
        ));
    }
    Ok(image
        .resize_exact(width, height as u32, image::imageops::FilterType::Lanczos3)
        .to_rgb8())
}
fn load(source: &str, root: &Path, accept: &str) -> Result<Encoded, CliError> {
    if source.starts_with("https://") || source.starts_with("http://") {
        let response = io::get(source, &[("Accept", accept)])?;
        Ok(Encoded {
            bytes: response.bytes,
            content_type: response.content_type,
        })
    } else {
        Ok(Encoded {
            bytes: io::read(&root.join(source))?,
            content_type: "local/unknown".into(),
        })
    }
}
fn normalized(bytes: &[u8]) -> Result<image::DynamicImage, CliError> {
    use image::ImageDecoder;
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| CliError::io("unknown image"))?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader
        .into_decoder()
        .map_err(|_| CliError::new("decode", "image decode failed"))?;
    if decoder
        .icc_profile()
        .map_err(|_| CliError::new("decode", "ICC read failed"))?
        .is_some()
        || decoder
            .exif_metadata()
            .map_err(|_| CliError::new("decode", "EXIF read failed"))?
            .is_some()
    {
        return Err(CliError::usage(
            "normalize ICC profiles and EXIF orientation before image tuning",
        ));
    }
    image::DynamicImage::from_decoder(decoder)
        .map_err(|_| CliError::new("decode", "image decode failed"))
}
pub(crate) fn audit(urls: &[String], accepts: &[String]) -> Result<Value, CliError> {
    if urls.is_empty() || urls.len() > 1000 || accepts.is_empty() || accepts.len() > 8 {
        return Err(CliError::usage(
            "audit accepts 1..1000 URLs and 1..8 Accept headers",
        ));
    }
    let mut rows = Vec::new();
    for (index, url) in urls.iter().enumerate() {
        for accept in accepts {
            let result=io::get(url,&[("Accept",accept)]).and_then(|r|{let image=io::image(&r.bytes)?;Ok(json!({"content_type":r.content_type,"bytes":r.bytes.len(),"dimensions":[image.width(),image.height()],"decoded_format":format!("{:?}",image::guess_format(&r.bytes).map_err(|_|CliError::new("decode","unknown image format"))?)}))});
            let mut row =
                json!({"image_index":index,"group":pattern(&io::url(url)?),"accept":accept});
            match result {
                Ok(value) => row["served"] = value,
                Err(e) => row["error"] = json!({"code":e.code,"message":e.message}),
            };
            rows.push(row);
        }
    }
    let failed = rows.iter().any(|r| r.get("error").is_some());
    Ok(
        json!({"schema":AUDIT_SCHEMA,"verdict":if failed{"incomplete"}else{"complete"},"items":rows}),
    )
}
pub(crate) fn search(input: Search, root: &Path) -> Result<Value, CliError> {
    if input.schema != INPUT_SCHEMA {
        return Err(CliError::new(
            "version_skew",
            "unsupported imgtune manifest",
        ));
    }
    if input.images.is_empty()
        || input.images.len() > 1000
        || input.widths.is_empty()
        || input.widths.len() > 16
        || input.widths.iter().any(|w| !(8..=16384).contains(w))
        || input.formats.is_empty()
        || input.formats.len() > 8
        || input
            .formats
            .iter()
            .any(|s| !matches!(s.as_str(), "jpeg" | "webp" | "avif" | "png"))
        || input.qualities.is_empty()
        || input.qualities.iter().any(|q| *q == 0 || *q > 100)
        || input.qualities.len() * input.formats.len() > 512
        || !input.target_score.is_finite()
        || input.target_score > 100.0
        || input
            .butteraugli_ceiling
            .is_some_and(|v| !v.is_finite() || v < 0.0)
    {
        return Err(CliError::usage("invalid bounded image tuning policy"));
    }
    let mut ids = std::collections::BTreeSet::new();
    if input
        .images
        .iter()
        .any(|i| i.id.trim().is_empty() || !ids.insert(&i.id))
    {
        return Err(CliError::usage("image IDs must be unique and nonempty"));
    }
    let accept = match &input.adapter {
        Adapter::Local => "image/*",
        Adapter::UrlTemplate { accept, .. } => accept,
    };
    let mut rows = Vec::new();
    let mut groups: BTreeMap<String, Value> = BTreeMap::new();
    for item in &input.images {
        let group = if item.source.starts_with("http") {
            pattern(&io::url(&item.source)?)
        } else {
            "local".into()
        };
        for width in &input.widths {
            let result = (|| -> Result<Value, CliError> {
                let original = load(&item.source, root, accept)?;
                let current = load(&item.current, root, accept)?;
                let reference = resize(&normalized(&original.bytes)?, *width)?;
                let mut candidates = Vec::new();
                let mut selected: Option<Value> = None;
                let mut complete = true;
                for format in &input.formats {
                    for quality in &input.qualities {
                        if matches!(input.adapter, Adapter::Local)
                            && format == "webp"
                            && quality != &input.qualities[0]
                        {
                            continue;
                        }
                        let parameters = Parameters {
                            width: *width,
                            format: format.clone(),
                            quality: *quality,
                        };
                        let measured=input.adapter.transform(&original.bytes,&item.source,&parameters).and_then(|encoded|{
                        let decoded=normalized(&encoded.bytes)?;
                        if decoded.dimensions()!=reference.dimensions() || decoded.color().has_alpha(){return Err(CliError::new("decode","adapter output dimensions/alpha differ from declared reference"));}
                        let decoded=decoded.to_rgb8();let score=saccade_core::quality::score(&reference,&decoded)?;
                        let distance=if input.butteraugli_ceiling.is_some(){Some(saccade_core::quality::butteraugli_distance(&reference,&decoded)?)}else{None};
                        let meets=score>=input.target_score && input.butteraugli_ceiling.is_none_or(|ceiling|distance.is_some_and(|v|v<=ceiling));
                        Ok(json!({"parameters":parameters,"served_content_type":encoded.content_type,"decoded_format":format!("{:?}",image::guess_format(&encoded.bytes).map_err(|_|CliError::new("decode","unknown output format"))?),"bytes":encoded.bytes.len(),"score":score,"butteraugli":distance,"meets_target":meets,"sha256":format!("{:x}",sha2::Sha256::digest(&encoded.bytes))}))
                    });
                        match measured {
                            Ok(value) => {
                                if value["meets_target"] == true
                                    && selected.as_ref().is_none_or(|s| {
                                        value["bytes"].as_u64() < s["bytes"].as_u64()
                                    })
                                {
                                    selected = Some(value.clone());
                                }
                                candidates.push(value);
                            }
                            Err(e) => {
                                complete = false;
                                candidates.push(json!({"parameters":parameters,"error":{"code":e.code,"message":e.message}}));
                            }
                        }
                    }
                }
                if !complete {
                    selected = None;
                }
                let saved = selected
                    .as_ref()
                    .and_then(|s| s["bytes"].as_i64())
                    .map(|bytes| current.bytes.len() as i64 - bytes);
                Ok(
                    json!({"id":item.id,"group":group,"width":width,"reference_dimensions":[reference.width(),reference.height()],"source_sha256":format!("{:x}",sha2::Sha256::digest(&original.bytes)),"current_bytes":current.bytes.len(),"current_content_type":current.content_type,"candidates":candidates,"selected":selected,"bytes_saved":saved,"coverage":if !complete{"incomplete"}else if saved.is_none(){"no_candidate"}else{"complete"}}),
                )
            })();
            rows.push(match result{Ok(v)=>v,Err(e)=>json!({"id":item.id,"group":group,"width":width,"coverage":"incomplete","error":{"code":e.code,"message":e.message}})});
        }
    }
    for row in &rows {
        let group = row["group"].as_str().unwrap_or("unknown").to_owned();
        let aggregate=groups.entry(group).or_insert_with(||json!({"items":0,"selected":0,"current_bytes":0,"bytes_saved":0,"worst_saved_fraction":null}));
        aggregate["items"] = json!(aggregate["items"].as_u64().unwrap_or(0) + 1);
        if let Some(saved) = row["bytes_saved"].as_i64() {
            let current = row["current_bytes"].as_u64().unwrap_or(0);
            let fraction = if current == 0 {
                0.0
            } else {
                saved as f64 / current as f64
            };
            aggregate["selected"] = json!(aggregate["selected"].as_u64().unwrap_or(0) + 1);
            aggregate["current_bytes"] =
                json!(aggregate["current_bytes"].as_u64().unwrap_or(0) + current);
            aggregate["bytes_saved"] =
                json!(aggregate["bytes_saved"].as_i64().unwrap_or(0) + saved);
            aggregate["worst_saved_fraction"] = json!(
                aggregate["worst_saved_fraction"]
                    .as_f64()
                    .map_or(fraction, |v| v.min(fraction))
            );
        }
    }
    let complete = rows.iter().all(|r| r["coverage"] == "complete");
    Ok(
        json!({"schema":SEARCH_SCHEMA,"verdict":if complete{"complete"}else{"incomplete"},"policy":{"target_score":input.target_score,"butteraugli_ceiling":input.butteraugli_ceiling,"metric":"SSIMULACRA2 0.5.1; Butteraugli 0.9.3 at 80 cd/m2","reference":"opaque normalized sRGB; Lanczos3 resize","search":"lowest bytes among declared grid; not a global optimum"},"items":rows,"groups":groups}),
    )
}
use sha2::Digest;
pub(crate) fn run(args: ImgtuneArgs) -> Result<u8, CliError> {
    let (value, out, json) = match args.operation {
        Operation::Audit {
            urls,
            accept,
            out,
            json,
        } => {
            let text = String::from_utf8(io::read(&urls)?)
                .map_err(|_| CliError::usage("URL list must be UTF-8"))?;
            let urls = text
                .lines()
                .map(str::trim)
                .filter(|s| !s.is_empty() && !s.starts_with('#'))
                .map(str::to_owned)
                .collect::<Vec<_>>();
            (audit(&urls, &accept)?, out, json)
        }
        Operation::Search {
            manifest,
            out,
            json,
        } => {
            let input = io::json(&manifest)?;
            (
                search(input, manifest.parent().unwrap_or(Path::new(".")))?,
                out,
                json,
            )
        }
    };
    io::write(&out, &value)?;
    io::emit(
        &value,
        json,
        "image tuning evidence written; originals retained",
    )?;
    Ok(u8::from(value["verdict"] != "complete"))
}
#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    #[test]
    fn local_jpeg_and_webp_keep_source_and_content_type() {
        let image =
            image::RgbImage::from_fn(32, 32, |x, y| image::Rgb([x as u8 * 7, y as u8 * 7, 90]));
        let mut source = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image)
            .write_to(&mut source, image::ImageFormat::Png)
            .expect("png");
        let bytes = source.into_inner();
        for format in ["jpeg", "webp"] {
            let encoded = Adapter::Local
                .transform(
                    &bytes,
                    "local",
                    &Parameters {
                        width: 16,
                        format: format.into(),
                        quality: 70,
                    },
                )
                .expect("encode");
            assert!(encoded.content_type.starts_with("image/"));
            assert_eq!(
                io::image(&encoded.bytes).expect("decode").dimensions(),
                (16, 16)
            );
        }
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    }
    #[test]
    #[ignore = "heavy: imgtune-avif"]
    fn local_avif_roundtrip() {
        let mut source = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            32,
            32,
            image::Rgb([40, 60, 80]),
        ))
        .write_to(&mut source, image::ImageFormat::Png)
        .expect("png");
        let encoded = Adapter::Local
            .transform(
                source.get_ref(),
                "local",
                &Parameters {
                    width: 32,
                    format: "avif".into(),
                    quality: 70,
                },
            )
            .expect("avif");
        assert_eq!(encoded.content_type, "image/avif");
        assert_eq!(
            io::image(&encoded.bytes).expect("decode").dimensions(),
            (32, 32)
        );
    }
    #[test]
    #[ignore = "heavy: imgtune-http"]
    fn negotiated_formats_and_search_use_local_http_fixture() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listen");
        let origin = format!("http://{}", listener.local_addr().expect("address"));
        let mut source = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            32,
            32,
            image::Rgb([40, 60, 80]),
        ))
        .write_to(&mut source, image::ImageFormat::Png)
        .expect("png");
        let png = source.into_inner();
        let source_copy = png.clone();
        let server = std::thread::spawn(move || {
            for _ in 0..5 {
                let (mut stream, _) = listener.accept().expect("accept");
                let mut request = [0u8; 4096];
                let length = stream.read(&mut request).expect("request");
                let request = String::from_utf8_lossy(&request[..length]);
                let modern = request.contains("image/webp");
                let output = if modern {
                    Adapter::Local
                        .transform(
                            &png,
                            "local",
                            &Parameters {
                                width: 32,
                                format: "webp".into(),
                                quality: 100,
                            },
                        )
                        .expect("encode")
                } else {
                    Encoded {
                        bytes: png.clone(),
                        content_type: "image/png".into(),
                    }
                };
                write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",output.content_type,output.bytes.len()).expect("headers");
                stream.write_all(&output.bytes).expect("body");
            }
        });
        let urls = vec![format!("{origin}/image.png")];
        let audit = audit(&urls, &["image/webp".into(), "image/*".into()]).expect("audit");
        assert_eq!(audit["items"][0]["served"]["content_type"], "image/webp");
        assert_eq!(audit["items"][1]["served"]["content_type"], "image/png");
        let result = search(
            Search {
                schema: INPUT_SCHEMA.into(),
                images: vec![Input {
                    id: "fixture".into(),
                    source: urls[0].clone(),
                    current: urls[0].clone(),
                }],
                widths: vec![32],
                formats: vec!["png".into()],
                qualities: vec![70],
                target_score: 90.0,
                butteraugli_ceiling: None,
                adapter: Adapter::UrlTemplate {
                    template: "{base}{path}?width={w}&format={fm}&quality={q}".into(),
                    accept: "image/*".into(),
                },
            },
            Path::new("."),
        )
        .expect("search");
        assert_eq!(result["verdict"], "complete");
        assert_eq!(result["items"][0]["bytes_saved"], 0);
        assert!(png_header(&source_copy));
        server.join().expect("server");
    }
    fn png_header(bytes: &[u8]) -> bool {
        bytes.starts_with(b"\x89PNG")
    }
}
