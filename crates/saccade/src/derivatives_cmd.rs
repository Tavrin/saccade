//! N18 contact sheets over bounded local declarations; no provider calls.
use crate::agent::CliError;
use image::{DynamicImage, RgbaImage, imageops};
use saccade_core::{
    derivatives as ds,
    general::input,
    localized::digest,
    wave7::{faces::FaceReport, vision::VisionImage},
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

#[derive(clap::Args)]
pub(crate) struct Args {
    /// Source raster (opaque 8-bit SDR for face inference).
    source: PathBuf,
    /// saccade-derivatives.v1: source boxes, crops, display sizes and rendition paths.
    declaration: PathBuf,
    /// New empty directory outside inputs; writes HTML, PNG and manifest-bound JSON.
    #[arg(long)]
    out: PathBuf,
    /// Optional image-bound face receipt; explicitly labelled replay.
    #[arg(long)]
    faces_report: Option<PathBuf>,
    /// Cached face model ID; uses shared model config, never downloads.
    #[arg(long, default_value = "yunet-2026may")]
    detector: String,
    #[arg(long)]
    json: bool,
}
fn face_evidence(
    args: &Args,
    image: &VisionImage,
    opaque: bool,
) -> Result<(Option<FaceReport>, Value), CliError> {
    if let Some(path) = &args.faces_report {
        let mut report: FaceReport = crate::parse_contract(
            &input::bytes(path, 1 << 20)?,
            saccade_core::wave7::faces::FACES_SCHEMA,
        )?;
        report.validate(image).map_err(crate::wave7_cmd::error)?;
        report.provenance.runtime = "replay".into();
        report.provenance.source_parity = false;
        let evidence = json!({"state":if report.faces.is_empty() {"not_established"} else {"observed"},"reason":"explicit source-bound face receipt replay","report":report});
        return Ok((Some(report), evidence));
    }
    let result = if opaque {
        cached_faces(image, &args.detector)
    } else {
        Err(CliError::new(
            "vision_unavailable",
            "face inference requires opaque source pixels",
        ))
    };
    match result {
        Ok(report) => {
            let evidence = json!({"state":if report.faces.is_empty() {"not_established"} else {"observed"},"reason":"cached local model inference; no detection never establishes safety","report":report});
            Ok((Some(report), evidence))
        }
        Err(error) => Ok((
            None,
            json!({"state":"unavailable","reason":error.message,"report":null}),
        )),
    }
}
fn cached_faces(image: &VisionImage, detector: &str) -> Result<FaceReport, CliError> {
    #[cfg(feature = "local-models")]
    {
        use saccade_core::wave7::{faces, runtime::OnnxModel, runtime_install};
        let reg = crate::wave7_cmd::registry(None)?;
        let cache = crate::wave7_cmd::cache(None)?;
        let library =
            runtime_install::resolve(crate::wave7_cmd::runtime_flag(None)?.as_deref(), &cache)
                .map_err(crate::wave7_cmd::error)?;
        let mut model = OnnxModel::load(
            reg.model(detector).map_err(crate::wave7_cmd::error)?,
            &cache,
            &library,
            false,
        )
        .map_err(crate::wave7_cmd::error)?;
        faces::detect(image, &mut model).map_err(crate::wave7_cmd::error)
    }
    #[cfg(not(feature = "local-models"))]
    {
        let _ = (image, detector);
        Err(CliError::new(
            "feature_unavailable",
            "face inference unavailable: build lacks local-models",
        ))
    }
}
fn write_png(path: &Path, image: &RgbaImage) -> Result<String, CliError> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image.clone())
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(|e| CliError::io(e.to_string()))?;
    let bytes = bytes.into_inner();
    crate::schema_cmd::write_new(path, &bytes)?;
    Ok(digest(&bytes))
}
fn html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
pub(crate) fn run(args: Args) -> Result<u8, CliError> {
    let encoded = input::bytes(&args.source, input::MAX_BYTES)?;
    let source = input::decode(&encoded)?;
    let declared_bytes = input::bytes(&args.declaration, 1 << 20)?;
    let declaration: ds::Declaration = serde_json::from_slice(&declared_bytes)?;
    declaration.validate([source.width(), source.height()])?;
    let vision = VisionImage {
        pixels: DynamicImage::ImageRgba8(source.clone()).to_rgb8(),
        sha256: digest(&encoded),
    };
    let (faces, face_section) =
        face_evidence(&args, &vision, source.pixels().all(|p| p[3] == 255))?;
    let mut inputs = vec![args.source.clone(), args.declaration.clone()];
    inputs.extend(args.faces_report.iter().cloned());
    let parent = args.declaration.parent().unwrap_or(Path::new("."));
    let mut previews = Vec::new();
    let mut rows = Vec::new();
    for d in &declaration.derivatives {
        let mut rendition_sha = None;
        let mut rendition_size = None;
        let preview = if let Some(path) = &d.path {
            let path = parent.join(path);
            inputs.push(path.clone());
            (|| -> Result<RgbaImage, CliError> {
                let bytes = input::bytes(&path, input::MAX_BYTES)?;
                let image = input::decode(&bytes)?;
                rendition_sha = Some(digest(&bytes));
                rendition_size = Some([image.width(), image.height()]);
                Ok(imageops::resize(
                    &image,
                    d.display_size[0],
                    d.display_size[1],
                    imageops::FilterType::Lanczos3,
                ))
            })()
        } else {
            ds::preview(&source, d).map_err(CliError::from)
        };
        let (display, error) = match preview {
            Ok(image) => (Some(image), None),
            Err(error) => (None, Some(error.message)),
        };
        let mut row = ds::row(
            &source,
            d,
            &declaration,
            faces.as_ref(),
            display.as_ref(),
            error.as_deref(),
        )?;
        row["rendition_path"] = json!(d.path);
        row["rendition_sha256"] = json!(rendition_sha);
        row["rendition_size"] = json!(rendition_size);
        rows.push(row);
        previews.push(display);
    }
    let sizes: Vec<_> = declaration
        .derivatives
        .iter()
        .map(|d| d.display_size)
        .collect();
    let labels: Vec<_> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            format!(
                "{} {} {}x{}",
                i + 1,
                r["verdict"].as_str().unwrap_or("not_established"),
                sizes[i][0],
                sizes[i][1]
            )
        })
        .collect();
    let (sheet, placements) = ds::contact_sheet(&previews, &sizes, &labels)?;
    // Everything above is read-only; refuse nonempty/overlapping output before writing.
    crate::general_cmd::prepare_out(
        &args.out,
        &inputs.iter().map(PathBuf::as_path).collect::<Vec<_>>(),
    )?;
    let mut body = String::from(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Derivative review sheet</title><style>body{font:16px sans-serif;margin:24px;background:#fafafa;color:#222}article{border:1px solid #bbb;padding:16px;margin:16px 0;overflow:auto}img{max-width:none}pre{white-space:pre-wrap}figure{margin:12px 0}.preview{border:1px solid #555}</style><h1>Derivative review sheet</h1><p>Previews use declared display pixels (one CSS pixel per sampled pixel). Browser zoom changes viewing size. Known subject geometry and pixel text thresholds are evidence, never human readability or subject completeness.</p>",
    );
    body.push_str(&format!(
        "<h2>Face evidence</h2><pre>{}</pre>",
        html(&serde_json::to_string_pretty(&face_section)?)
    ));
    for (index, ((row, preview), d)) in rows
        .iter_mut()
        .zip(&previews)
        .zip(&declaration.derivatives)
        .enumerate()
    {
        row["sheet_rect_px"] = json!(placements[index]);
        let preview_name = format!("derivative-{index:03}.png");
        row["preview"] = if let Some(preview) = preview {
            let sha = write_png(&args.out.join(&preview_name), preview)?;
            json!({"path":preview_name,"sha256":sha})
        } else {
            Value::Null
        };
        body.push_str(&format!(
            "<article><h2>{}. {} — {}</h2>",
            index + 1,
            html(&d.id),
            html(row["verdict"].as_str().unwrap_or("not_established"))
        ));
        if preview.is_some() {
            body.push_str(&format!("<figure><figcaption>Display {} × {} pixels</figcaption><img class=\"preview\" src=\"{preview_name}\" width=\"{}\" height=\"{}\" alt=\"Derivative preview\"></figure>", sizes[index][0], sizes[index][1], sizes[index][0], sizes[index][1]));
            body.push_str(&format!("<figure><figcaption>Known subject boxes projected from declared crop (delivered content remains unverified)</figcaption><svg role=\"img\" aria-label=\"Declared subject projections\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\"><image href=\"{preview_name}\" width=\"{}\" height=\"{}\"/>", sizes[index][0], sizes[index][1], sizes[index][0], sizes[index][1], sizes[index][0], sizes[index][1]));
            for subject in row["subject_preservation"]["subjects"]
                .as_array()
                .into_iter()
                .flatten()
            {
                if let Some(r) = subject["display_rect_px"].as_array() {
                    let color = if subject["state"] == "included" {
                        "#00803a"
                    } else {
                        "#d00000"
                    };
                    body.push_str(&format!("<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"2\"><title>{}</title></rect>", r[0], r[1], r[2], r[3], html(subject["label"].as_str().unwrap_or("subject"))));
                }
            }
            body.push_str("</svg></figure>");
        } else {
            body.push_str("<p>Rendition unavailable; this row failed.</p>");
        }
        for (text_index, text) in declaration.text_regions.iter().enumerate() {
            let name = format!("source-text-{text_index:03}.png");
            if index == 0 {
                let r = text.rect_px;
                write_png(
                    &args.out.join(&name),
                    &imageops::crop_imm(&source, r[0], r[1], r[2], r[3]).to_image(),
                )?;
            }
            body.push_str(&format!("<figure><figcaption>Original-resolution text: {}</figcaption><img src=\"{name}\" width=\"{}\" height=\"{}\" alt=\"Source text region\"></figure>", html(&text.label), text.rect_px[2], text.rect_px[3]));
        }
        body.push_str(&format!("<details open><summary>Evidence and limitations</summary><pre>{}</pre></details></article>", html(&serde_json::to_string_pretty(row)?)));
    }
    body.push_str("<p><a href=\"contact-sheet.png\">Contact sheet PNG</a> · <a href=\"saccade-derivative-sheet.v1.json\">JSON rows</a> · <a href=\"saccade-manifest.json\">Artifact manifest</a></p></html>");
    let sheet_sha = write_png(&args.out.join("contact-sheet.png"), &sheet)?;
    crate::schema_cmd::write_new(&args.out.join("index.html"), body.as_bytes())?;
    let verdict = if rows.iter().any(|r| r["verdict"] == "fail") {
        "fail"
    } else if rows.iter().all(|r| r["verdict"] == "pass") {
        "pass"
    } else {
        "not_established"
    };
    let report = json!({"schema":ds::REPORT_SCHEMA,"verdict":verdict,"source_sha256":vision.sha256,"source_size":vision.size(),"declaration_sha256":digest(&declared_bytes),"faces":face_section,"rows":rows,"contact_sheet":{"path":"contact-sheet.png","sha256":sheet_sha},"limitations":[saccade_core::wave7::faces::FACE_LIMIT,"Declared boxes are not complete subject annotations; no detection is not established, never safe.","Generated renditions use exact source crop and Lanczos3 resize. Delivered crop declarations do not verify delivered subject content.","Text measurements use declared regions and display pixels; no separable glyphs means not established. Pixel thresholds and OCR do not certify human readability.","No OCR, live providers, model download, approval or MCP mirror executed."]});
    let value = saccade_core::report_links::write(
        &args.out.join(format!("{}.json", ds::REPORT_SCHEMA)),
        &report,
    )?;
    saccade_core::manifest::write(&args.out, Default::default())?;
    if args.json {
        crate::emit(&format!("{}\n", serde_json::to_string(&value)?))?;
    } else {
        crate::emit(&format!(
            "derivative-sheet: {verdict} ({} rows)\n{}\n",
            declaration.derivatives.len(),
            args.out.join("index.html").display()
        ))?;
    }
    Ok(match verdict {
        "fail" => 1,
        "pass" => 0,
        _ => 4,
    })
}
