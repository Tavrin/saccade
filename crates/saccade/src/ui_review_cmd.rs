//! Source-first UI packet and optional pinned external OCR.
use crate::agent::CliError;
use saccade_core::{localized, ui_review};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
#[derive(clap::Args)]
pub(crate) struct Args {
    reference: PathBuf,
    candidate: PathBuf,
    /// Source JSON exported by the Playwright ingest, or a DOM/AX producer.
    #[arg(long)]
    reference_source: Option<PathBuf>,
    #[arg(long)]
    candidate_source: Option<PathBuf>,
    /// Frozen reference inclusion region; protected complement is exact by default.
    #[arg(long, conflicts_with = "bbox")]
    region: Option<PathBuf>,
    /// Intended pixel box x,y,width,height.
    #[arg(
        long = "box",
        value_delimiter = ',',
        num_args = 1,
        required_unless_present = "region"
    )]
    bbox: Option<Vec<u32>>,
    /// Optional saccade-tesseract.v1 runtime/model contract; requires the ocr feature.
    #[arg(long)]
    ocr_contract: Option<PathBuf>,
    #[arg(long)]
    perceptual_outside: bool,
    #[arg(long, default_value_t = 0.01)]
    maximum_outside_flip: f32,
    #[arg(long, default_value_t = 67.0)]
    ppd: f32,
    #[arg(long)]
    out: PathBuf,
}
fn bytes(p: &Path) -> Result<Vec<u8>, CliError> {
    saccade_core::general::input::bytes(p, 64 * 1024 * 1024).map_err(Into::into)
}
fn load_source(
    path: Option<&Path>,
    image: &[u8],
    dimensions: [u32; 2],
    contract: Option<&Path>,
) -> Result<ui_review::Source, CliError> {
    if let Some(path) = path {
        let source: ui_review::Source =
            crate::parse_contract(&bytes(path)?, "saccade-ui-source.v1")?;
        source.validate(&localized::digest(image), dimensions)?;
        return Ok(source);
    }
    let contract = contract.ok_or_else(|| {
        CliError::usage(
            "UI review requires source metadata, or an explicitly pinned optional OCR contract",
        )
    })?;
    #[cfg(feature = "ocr")]
    {
        ocr::recognize(contract, image, dimensions)
    }
    #[cfg(not(feature = "ocr"))]
    {
        let _ = contract;
        Err(CliError::new(
            "feature_unavailable",
            "OCR fallback requires the optional ocr feature",
        ))
    }
}
pub(crate) fn run(args: Args, json: bool) -> Result<u8, CliError> {
    let b = bytes(&args.reference)?;
    let a = bytes(&args.candidate)?;
    let reference = image::load_from_memory(&b).map_err(|e| CliError::usage(e.to_string()))?;
    let candidate = image::load_from_memory(&a).map_err(|e| CliError::usage(e.to_string()))?;
    let dimensions = [reference.width(), reference.height()];
    if [candidate.width(), candidate.height()] != dimensions {
        return Err(CliError::usage("UI screenshot dimensions differ"));
    }
    let before = load_source(
        args.reference_source.as_deref(),
        &b,
        dimensions,
        args.ocr_contract.as_deref(),
    )?;
    let after = load_source(
        args.candidate_source.as_deref(),
        &a,
        dimensions,
        args.ocr_contract.as_deref(),
    )?;
    let region = if let Some(path) = args.region {
        crate::parse_contract(&bytes(&path)?, "saccade-frozen-region.v1")?
    } else {
        let rect: [u32; 4] = args
            .bbox
            .ok_or_else(|| CliError::usage("intended region missing"))?
            .try_into()
            .map_err(|_| CliError::usage("box needs four coordinates"))?;
        localized::boxes(
            localized::digest(&b),
            dimensions,
            &[rect],
            BTreeMap::from([("method".into(), "box".into())]),
        )?
    };
    let measurement = localized::measure(
        &reference,
        &candidate,
        &localized::digest(&b),
        localized::digest(&a),
        region,
        localized::Policy {
            ppd: args.ppd,
            exact_outside: !args.perceptual_outside,
            maximum_outside_flip: args.maximum_outside_flip,
        },
    )?;
    let packet = ui_review::compare(&before, &after, measurement)?;
    crate::local_cmd::write_value(&args.out, &serde_json::to_value(&packet)?)?;
    let review_needed = !packet.findings.is_empty()
        || packet.localized.collateral != "preserved"
        || !packet.localized.intended_change_detected
        || !packet.sources.iter().all(|s| s.complete);
    if json {
        crate::emit(&format!("{}\n", serde_json::to_string_pretty(&packet)?))?;
    } else {
        crate::emit(&format!(
            "UI review: {} text/layout findings; {}; {}\n",
            packet.findings.len(),
            packet.localized.collateral,
            args.out.display()
        ))?;
    }
    Ok(u8::from(review_needed))
}
#[cfg(feature = "ocr")]
mod ocr {
    use super::*;
    use saccade_core::evidence::canonical::{self, Digest};
    use saccade_core::ui_review::OcrContract as Contract;
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    fn wait(command: &mut Command, timeout: u64, stdout: Stdio) -> Result<(), CliError> {
        let mut child = command
            .stdout(stdout)
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| CliError::io(e.to_string()))?;
        let start = Instant::now();
        loop {
            if let Some(status) = child.try_wait().map_err(|e| CliError::io(e.to_string()))? {
                return if status.success() {
                    Ok(())
                } else {
                    Err(CliError::usage("pinned Tesseract process failed"))
                };
            }
            if start.elapsed() >= Duration::from_millis(timeout) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(CliError::usage("pinned Tesseract timed out"));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    pub(super) fn recognize(
        path: &Path,
        image: &[u8],
        dimensions: [u32; 2],
    ) -> Result<ui_review::Source, CliError> {
        let contract: Contract = crate::parse_contract(&bytes(path)?, "saccade-tesseract.v1")?;
        if contract.schema != "saccade-tesseract.v1"
            || contract.version.is_empty()
            || contract.models.is_empty()
            || contract.models.len() > 8
            || ![6, 11].contains(&contract.psm)
            || !(1..=60_000).contains(&contract.timeout_ms)
            || contract
                .models
                .keys()
                .any(|s| s.is_empty() || !s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_'))
        {
            return Err(CliError::usage(
                "invalid pinned Tesseract model/settings contract",
            ));
        }
        let dir = path.parent().unwrap_or(Path::new("."));
        let executable = std::fs::canonicalize(dir.join(&contract.executable))
            .map_err(|e| CliError::io(e.to_string()))?;
        if Digest::of_bytes(&bytes(&executable)?) != contract.executable_sha256 {
            return Err(CliError::usage("Tesseract executable hash mismatch"));
        }
        let temp = tempfile::tempdir().map_err(|e| CliError::io(e.to_string()))?;
        let data = temp.path().join("tessdata");
        std::fs::create_dir(&data).map_err(|e| CliError::io(e.to_string()))?;
        for (language, pin) in &contract.models {
            let model = bytes(
                &dir.join(&contract.tessdata)
                    .join(format!("{language}.traineddata")),
            )?;
            if Digest::of_bytes(&model) != *pin {
                return Err(CliError::usage("Tesseract traineddata hash mismatch"));
            }
            std::fs::write(data.join(format!("{language}.traineddata")), model)
                .map_err(|e| CliError::io(e.to_string()))?;
        }
        let version_path = temp.path().join("version.txt");
        let version_file =
            std::fs::File::create(&version_path).map_err(|e| CliError::io(e.to_string()))?;
        let mut version_command = Command::new(&executable);
        version_command
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("LANG", "C")
            .arg("--version");
        wait(
            &mut version_command,
            contract.timeout_ms,
            Stdio::from(version_file),
        )?;
        if std::fs::metadata(&version_path)
            .map_err(|e| CliError::io(e.to_string()))?
            .len()
            > 65536
        {
            return Err(CliError::usage("Tesseract version output exceeds limit"));
        }
        let version = bytes(&version_path)?;
        if Digest::of_bytes(&version) != contract.version_output_sha256
            || !String::from_utf8_lossy(&version)
                .starts_with(&format!("tesseract {}\n", contract.version))
        {
            return Err(CliError::usage(
                "Tesseract runtime version/library fingerprint mismatch",
            ));
        }
        let input = temp.path().join("input.png");
        // Decode and stage a PNG rather than passing producer paths to the runtime.
        image::load_from_memory(image)
            .map_err(|e| CliError::usage(e.to_string()))?
            .save(&input)
            .map_err(|e| CliError::io(e.to_string()))?;
        let output = temp.path().join("output");
        let languages = contract
            .models
            .keys()
            .cloned()
            .collect::<Vec<_>>()
            .join("+");
        let mut command = Command::new(&executable);
        command
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("LANG", "C")
            .env("OMP_THREAD_LIMIT", "1")
            .arg(&input)
            .arg(&output)
            .arg("--tessdata-dir")
            .arg(&data)
            .args([
                "-l",
                &languages,
                "--psm",
                &contract.psm.to_string(),
                "-c",
                "tessedit_create_tsv=1",
            ]);
        wait(&mut command, contract.timeout_ms, Stdio::null())?;
        let tsv = output.with_extension("tsv");
        if std::fs::metadata(&tsv)
            .map_err(|e| CliError::io(e.to_string()))?
            .len()
            > 16 * 1024 * 1024
        {
            return Err(CliError::usage("Tesseract TSV exceeds 16 MiB"));
        }
        let text = std::fs::read_to_string(tsv).map_err(|e| CliError::io(e.to_string()))?;
        let producer = serde_json::json!({"backend":"tesseract","version":contract.version,"version_output_sha256":contract.version_output_sha256,"executable_sha256":contract.executable_sha256,"models":contract.models,"psm":contract.psm,"contract_sha256":canonical::Digest::of_bytes(&bytes(path)?),"licence":"Apache-2.0 engine and official traineddata; local runtime required","version_authority":"verified version output plus executable hash; linked-library versions recorded, not individual library byte hashes"});
        Ok(ui_review::tesseract_tsv(
            &text,
            localized::digest(image),
            dimensions,
            producer,
        )?)
    }
}

// wave6: reuse the existing pinned OCR adapter without guessing a new model licence.
pub(crate) fn recognize_text(
    contract: &Path,
    image: &[u8],
    dimensions: [u32; 2],
) -> Result<ui_review::Source, CliError> {
    #[cfg(feature = "ocr")]
    {
        ocr::recognize(contract, image, dimensions)
    }
    #[cfg(not(feature = "ocr"))]
    {
        let _ = (contract, image, dimensions);
        Err(CliError::new(
            "feature_unavailable",
            "OCR execution requires ocr; imported image-bound sources remain available",
        ))
    }
}
