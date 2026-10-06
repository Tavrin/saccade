//! Explicit hosted document OCR routing, off unless --ocr-provider is selected.
use crate::agent::CliError;
use clap::{Args, ValueEnum};
use serde_json::Value;
use std::path::{Path, PathBuf};
#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum Provider {
    Mistral,
}
#[derive(Args, Default)]
pub(crate) struct Options {
    /// Optional document OCR provider; selecting it exports images/PDFs only with --ocr-run.
    #[arg(long, value_enum)]
    pub(crate) ocr_provider: Option<Provider>,
    /// Explicit dated OCR model (aliases refused).
    #[arg(long, requires = "ocr_provider")]
    ocr_model: Option<String>,
    /// Zero-based pages selected explicitly; images accept only 0.
    #[arg(long, value_delimiter = ',', default_value = "0")]
    ocr_pages: Vec<u32>,
    /// Constructed/recorded request-bound fixture envelopes for both inputs; no network.
    #[arg(
        long,
        num_args = 2,
        requires = "ocr_provider",
        conflicts_with = "ocr_run"
    )]
    ocr_responses: Vec<PathBuf>,
    /// Explicitly authorize live document export under existing root policy.
    #[arg(long,requires_all=["ocr_provider","ocr_max_spend_usd","ocr_price_per_page_usd","ocr_price_policy"])]
    ocr_run: bool,
    /// Finite overall monetary cap; unestablished usage keeps the full reservation.
    #[arg(long, requires = "ocr_run")]
    ocr_max_spend_usd: Option<f64>,
    /// User-confirmed conservative per-selected-page billing ceiling.
    #[arg(long, requires = "ocr_run")]
    ocr_price_per_page_usd: Option<f64>,
    /// User-owned price-policy revision; no built-in unverified pricing.
    #[arg(long, requires = "ocr_run")]
    ocr_price_policy: Option<String>,
    /// Existing human-owned user.toml with explicit egress roots and Mistral credential binding.
    #[arg(long, requires = "ocr_run")]
    ocr_user_config: Option<PathBuf>,
}
#[cfg(feature = "ocr-provider")]
pub(crate) fn measure(
    options: &Options,
    files: [&Path; 2],
    expected: &[String],
) -> Result<Value, CliError> {
    use saccade_core::general::{
        document_ocr::{Mistral, Request},
        input,
    };
    use serde_json::json;
    let model = options
        .ocr_model
        .as_ref()
        .ok_or_else(|| CliError::usage("--ocr-model needs an explicit dated model"))?;
    if options.ocr_provider.is_none() || (!options.ocr_run && options.ocr_responses.len() != 2) {
        return Err(CliError::usage(
            "select Mistral plus two --ocr-responses fixtures or --ocr-run",
        ));
    }
    if expected.len() > 64 || expected.iter().any(|s| s.is_empty() || s.len() > 4096) {
        return Err(CliError::usage("document OCR expectation bound"));
    }
    let mut observations = Vec::new();
    let run_nonce =
        tempfile::NamedTempFile::new().map_err(|_| CliError::io("OCR run identity unavailable"))?;
    let run_id = saccade_core::localized::digest(run_nonce.path().to_string_lossy().as_bytes());
    for (i, path) in files.iter().enumerate() {
        let bytes = input::bytes(path, 16 * 1024 * 1024)?;
        let mime = if bytes.starts_with(b"%PDF-") {
            "application/pdf"
        } else {
            match image::guess_format(&bytes) {
                Ok(image::ImageFormat::Png) => "image/png",
                Ok(image::ImageFormat::Jpeg) => "image/jpeg",
                _ => {
                    return Err(CliError::usage(
                        "Mistral document input must be PNG, JPEG or PDF",
                    ));
                }
            }
        };
        let request = Request {
            bytes,
            media_type: mime.into(),
            model: model.clone(),
            pages: options.ocr_pages.clone(),
        };
        let observation = if options.ocr_run {
            live(options, &request, path, &run_id)?
        } else {
            let fixture: Value =
                serde_json::from_slice(&input::bytes(&options.ocr_responses[i], 4 * 1024 * 1024)?)?;
            let digest = saccade_core::evidence::canonical::digest(&Mistral::request(&request)?)
                .map_err(|e| CliError::usage(e.to_string()))?;
            if fixture["schema"] != "saccade-document-ocr-fixture.v1"
                || fixture["request_sha256"] != digest.as_str()
            {
                return Err(CliError::usage(
                    "document OCR fixture request identity mismatch",
                ));
            }
            Mistral::decode(&request, &serde_json::to_vec(&fixture["response"])?, true)?
        };
        observations.push(observation);
    }
    let rates = saccade_core::general::text::rates(&observations[0].text, &observations[1].text)?;
    let expectations: Vec<_> = expected
        .iter()
        .map(|s| json!({"text":s,"present":observations[1].text.contains(s),"readable":null}))
        .collect();
    let missing = expectations.iter().any(|e| e["present"] == false);
    Ok(
        json!({"schema":"saccade-document-text.v1","operation":"text","verdict":if missing || rates.character_edits>0{"regression"}else if !expected.is_empty(){"unknown"}else{"pass"},"counts":{"character_edits":rates.character_edits,"failed_expectations":expectations.iter().filter(|e|e["present"]==false).count()},"comparison":{"rates":rates,"expected":expectations},"documents":observations,"limitations":["provider observations, not source truth","Markdown page structure retained; word geometry and readability unavailable","constructed fixture mapping has no live API qualification"]}),
    )
}
#[cfg(feature = "ocr-provider")]
fn live(
    options: &Options,
    request: &saccade_core::general::document_ocr::Request,
    path: &Path,
    run_id: &str,
) -> Result<saccade_core::general::document_ocr::Observation, CliError> {
    use saccade_core::{
        budget_ledger::{Ledger, MoneyScope},
        general::document_ocr::Mistral,
        judge_provider::{Keys, transport},
        root_policy::RootPolicy,
    };
    let user = crate::review_cmd::load_user(&crate::review_cmd::user_file(
        options.ocr_user_config.as_deref(),
    ))?;
    let mut roots = RootPolicy::new(
        &user
            .roots
            .iter()
            .map(|r| r.path.clone())
            .collect::<Vec<_>>(),
        user.out_root.as_deref(),
        false,
        &[],
    )?;
    user.apply(&mut roots)
        .map_err(|_| CliError::new("egress_denied", "document OCR root policy"))?;
    let ledger = Ledger::new(&Keys::default_dir().join("attempts"), false);
    let keys = Keys::new(None);
    let network = transport::Network;
    let mut auth = crate::review_cmd::authorization(options.ocr_run, 2, run_id, &user);
    auth.scopes[0].caps.providers.insert("mistral".into(), 2);
    let transport = transport::Transport {
        user: &user,
        roots: &roots,
        authorization: &auth,
        ledger: &ledger,
        keys: &keys,
        http: &network,
    };
    let scopes = if options.ocr_run {
        vec![MoneyScope {
            id: format!("document-ocr/run/{run_id}"),
            cap_nano_usd: saccade_core::assist::execution::nano_usd(
                options
                    .ocr_max_spend_usd
                    .ok_or_else(|| CliError::usage("OCR spend cap required"))?,
            )?,
        }]
    } else {
        vec![]
    };
    let source = std::fs::canonicalize(path)
        .map_err(|_| CliError::io("OCR source unavailable"))?
        .to_string_lossy()
        .into_owned();
    Ok(Mistral::execute(
        request,
        &transport,
        &scopes,
        saccade_core::assist::execution::nano_usd(
            options
                .ocr_price_per_page_usd
                .ok_or_else(|| CliError::usage("OCR price ceiling required"))?,
        )?,
        options
            .ocr_price_policy
            .as_deref()
            .ok_or_else(|| CliError::usage("OCR price policy required"))?,
        &[source],
    )?)
}
#[cfg(not(feature = "ocr-provider"))]
pub(crate) fn measure(
    _options: &Options,
    _files: [&Path; 2],
    _expected: &[String],
) -> Result<Value, CliError> {
    Err(CliError::new(
        "feature_unavailable",
        "Mistral OCR requires ocr-provider",
    ))
}

#[cfg(feature = "mcp")]
pub(crate) fn schema() -> Value {
    serde_json::json!({"type":"object","properties":{"operation":{"const":"document_text"},"a":{"type":"string"},"b":{"type":"string"},"response_a":{"type":"string"},"response_b":{"type":"string"},"model":{"type":"string"},"pages":{"type":"array","minItems":1,"maxItems":64,"items":{"type":"integer","minimum":0}},"expect_text":{"type":"array","maxItems":64,"items":{"type":"string"}},"out":{"type":"string"}},"required":["operation","a","b","response_a","response_b","model","out"],"additionalProperties":false})
}
#[cfg(feature = "mcp")]
pub(crate) fn imported(
    files: [PathBuf; 2],
    responses: [PathBuf; 2],
    model: String,
    pages: Vec<u32>,
    expected: Vec<String>,
    out: &Path,
) -> Result<Value, CliError> {
    let options = Options {
        ocr_provider: Some(Provider::Mistral),
        ocr_model: Some(model),
        ocr_responses: responses.into(),
        ocr_pages: pages,
        ..Default::default()
    };
    let value = measure(&options, [&files[0], &files[1]], &expected)?;
    crate::general_cmd::prepare_out(out, &[&files[0], &files[1]])?;
    Ok(value)
}
