//! Guarded pre-check artifacts, common disclaimer, HTML and JUnit.

use crate::{Error, Result};
use serde::Serialize;
use std::path::Path;

/// Required framing in all outputs.
pub const DISCLAIMER: &str = "PRE-CHECK only. Not a certification; does not replace platform-holder required testing (e.g. Harding FPA) or formal compliance processes. No compliance claims. PASS means no trigger detected in this capture under these checks; review coverage and limitations below.";
const SENTINEL: &str = ".saccade-precheck";

pub(crate) fn prepare(out: &Path, inputs: &[&Path], marker: &str) -> Result<()> {
    crate::run::guard_output_dir(out, inputs, &[marker, SENTINEL])?;
    for leaf in [marker, "index.html", "report.txt", "images", SENTINEL] {
        let artifact = crate::run::normalise_path(&out.join(leaf));
        if inputs
            .iter()
            .any(|p| crate::run::normalise_path(p).starts_with(&artifact))
        {
            return Err(Error::Config(
                "a pre-check artifact would overwrite an input/config file".into(),
            ));
        }
    }
    std::fs::create_dir_all(out).map_err(crate::run::io_err(
        "creating pre-check report directory".into(),
    ))?;
    for leaf in [marker, "index.html", "report.txt", "images", SENTINEL] {
        let path = out.join(leaf);
        if let Ok(meta) = std::fs::symlink_metadata(&path) {
            let removed = if meta.is_dir() && !meta.file_type().is_symlink() {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
            };
            removed.map_err(crate::run::io_err(
                "removing stale pre-check artifact".into(),
            ))?;
        }
    }
    std::fs::write(out.join(SENTINEL), b"incomplete pre-check\n")
        .map_err(crate::run::io_err("writing pre-check sentinel".into()))?;
    std::fs::create_dir(out.join("images"))
        .map_err(crate::run::io_err("creating pre-check images".into()))?;
    Ok(())
}

pub(crate) fn image(out: &Path, leaf: &str, image: &image::RgbImage) -> Result<String> {
    let relative = format!("images/{leaf}.png");
    let path = out.join(&relative);
    image
        .save(&path)
        .map_err(|source| Error::Encode { path, source })?;
    Ok(relative)
}

pub(crate) fn heatmap(mask: &[bool], width: u32, height: u32) -> image::RgbImage {
    image::RgbImage::from_fn(width, height, |x, y| {
        if mask[y as usize * width as usize + x as usize] {
            image::Rgb([255, 110, 20])
        } else {
            image::Rgb([12, 16, 24])
        }
    })
}

pub(crate) fn finish<T: Serialize>(
    out: &Path,
    marker: &str,
    report: &T,
    text: &str,
    title: &str,
) -> Result<()> {
    let json = serde_json::to_string_pretty(report)?;
    std::fs::write(out.join(marker), &json)
        .map_err(crate::run::io_err("writing pre-check JSON".into()))?;
    std::fs::write(out.join("report.txt"), text)
        .map_err(crate::run::io_err("writing pre-check text".into()))?;
    let safe = json
        .replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029");
    let html = format!(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{title}</title><style>{}\n{}</style><main><h1>{title}</h1><p>{DISCLAIMER}</p><p id=\"summary\"></p><div id=\"warnings\"></div><nav id=\"timeline\" aria-label=\"Risk segments or images\"></nav><section id=\"detail\" aria-live=\"polite\"></section><details><summary>THRESHOLDS to verify</summary><pre id=\"thresholds\"></pre></details></main><script type=\"application/json\" id=\"precheck-data\">{safe}</script><script>{}</script></html>",
        include_str!("../../assets/tokens.css"),
        include_str!("../../assets/precheck.css"),
        include_str!("../../assets/precheck.js")
    );
    std::fs::write(out.join("index.html"), html)
        .map_err(crate::run::io_err("writing pre-check HTML".into()))?;
    std::fs::remove_file(out.join(SENTINEL))
        .map_err(crate::run::io_err("completing pre-check report".into()))?;
    Ok(())
}

fn xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Write JUnit, protecting the inputs and refusing symlink destinations.
pub fn junit(
    path: &Path,
    suite: &str,
    cases: &[(String, bool, String)],
    inputs: &[&Path],
) -> Result<()> {
    let normalized = crate::run::normalise_path(path);
    if inputs
        .iter()
        .any(|p| normalized.starts_with(crate::run::normalise_path(p)))
        || std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
    {
        return Err(Error::Config(
            "JUnit destination must not overwrite an input or follow a symlink".into(),
        ));
    }
    let mut body = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><testsuite name=\"{}\" tests=\"{}\" failures=\"{}\"><properties><property name=\"framing\" value=\"{}\"/></properties>",
        xml(suite),
        cases.len(),
        cases.iter().filter(|c| c.1).count(),
        xml(DISCLAIMER)
    );
    for (name, failed, message) in cases {
        body.push_str(&format!("<testcase name=\"{}\">", xml(name)));
        if *failed {
            body.push_str(&format!("<failure message=\"{}\"/>", xml(message)));
        }
        body.push_str(&format!(
            "<system-out>{}</system-out></testcase>",
            xml(message)
        ));
    }
    body.push_str("</testsuite>\n");
    std::fs::write(path, body).map_err(crate::run::io_err("writing pre-check JUnit".into()))
}
