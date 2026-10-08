//! Capability catalogue and explicit question routing; unavailable families never silently fall back.
use crate::{agent::CliError, general_cmd};
use saccade_core::general::{hashing, input};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
pub(crate) const SCHEMA: &str = "saccade-capabilities.v1";
pub(crate) const CHOICE_SCHEMA: &str = "saccade-pipeline-choice.v1";
pub(crate) const CHOICE_FILE: &str = "saccade-pipeline-choice.v1.json";
pub(crate) const QUESTION_SCHEMA: &str = "saccade-question-report.v1";
pub(crate) const NEAR_SCHEMA: &str = "saccade-near-duplicate.v1";
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum Question {
    SameRender,
    SameContent,
    SameText,
    NearDuplicate,
    Quality,
}
impl Question {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::SameRender => "same-render",
            Self::SameContent => "same-content",
            Self::SameText => "same-text",
            Self::NearDuplicate => "near-duplicate",
            Self::Quality => "quality",
        }
    }
}
#[derive(clap::Args)]
pub(crate) struct Args {
    #[arg(long)]
    json: bool,
}
pub(crate) fn catalogue() -> Value {
    let mut families = Vec::new();
    let mut add = |family: &str,
                   command: &str,
                   inputs: &str,
                   features: Vec<&str>,
                   status: &str,
                   question: &str,
                   limits: &str| {
        families.push(json!({"family":family,"command":command,"inputs":inputs,"features_required":features,"status":status,"question":question,"limits":limits}))
    };
    add(
        "optical_code",
        "optical-code",
        "final SDR raster or declared PDF/SVG page",
        vec!["optical-code"],
        if cfg!(feature = "optical-code") {
            "available_bounded"
        } else {
            "feature_unavailable"
        },
        "Does the code decode to the expected payload?",
        "one symbol per declared region; QR pixel indicators are heuristic; no print grade or visual-similarity inference",
    );
    add(
        "print",
        "print",
        "paired CMYK TIFF/JPEG/PDF rasters",
        vec!["print"],
        if cfg!(feature = "print") {
            "available_bounded"
        } else {
            "feature_unavailable"
        },
        "How do colour, separations and ink coverage differ?",
        "Requires CMYK ICC; bounded raster PDF subset; ICC gamut model and heuristic text-like marks; no press approval",
    );
    add(
        "native_rasters",
        "geo compare; geo mask-metrics; geo tiles",
        "paired native multichannel TIFFs, class rasters or z/x/y tile trees",
        vec!["geo"],
        if cfg!(feature = "geo") {
            "available_bounded"
        } else {
            "feature_unavailable"
        },
        "What native band, class or tile-coverage changes are present?",
        "Exact grid metadata required; nodata-aware; RGB ranges must be declared; no reprojection or map rendering",
    );
    add(
        "pixel_perceptual",
        "compare; prove identity",
        "paired rasters or matching directories",
        vec![],
        "available",
        "Are supplied decoded samples or perceptual renderings different?",
        "threshold passing never approves a baseline",
    );
    add(
        "colour",
        "compare (tone/profile diagnostics)",
        "paired SDR rasters",
        vec![],
        "available",
        "Do colour/tone changes explain observed perceptual differences?",
        "conditional on capture/profile/viewing settings",
    );
    add(
        "hdr",
        "compare --hdr-tonemapper --hdr-exposures",
        "paired EXR/HDR",
        vec![],
        "available",
        "How visible is the HDR difference under the declared exposure range?",
        "existing HDR-FLIP; native samples and nonfinite evidence stay separate",
    );
    add(
        "timed_text",
        "timed-text",
        "plain SRT/WebVTT, frame map, declared text region and image-bound OCR",
        vec![],
        "available_bounded",
        "Does expected timed text appear in the supplied samples, on time and legibly?",
        "OCR absence explicitly skips; text-quality enables pixel evidence; external video extraction and sampling gaps remain unverified",
    );
    add(
        "video_temporal",
        "experiment temporal --fps",
        "numbered SDR frame directories",
        vec!["graphics"],
        if cfg!(feature = "graphics") {
            "available"
        } else {
            "feature_unavailable"
        },
        "How visible are spatial and temporal video differences?",
        "existing ColorVideoVDP; frame-rate/display conditions required",
    );
    add(
        "geometry",
        "experiment geometry; prove mesh-identity",
        "meshes in declared common units",
        vec!["geometry"],
        if cfg!(feature = "geometry") {
            "available"
        } else {
            "feature_unavailable"
        },
        "Did static mesh surfaces or ordered native mesh data change?",
        "appearance and dynamic geometry not implied",
    );
    add(
        "registration",
        "compare --align similarity|affine|homography|auto",
        "paired 8-bit SDR rasters/directories",
        vec![],
        "available",
        "How similar are the images over registered geometric overlap?",
        "inlier refusal, interpolation and explicit geometry exclusions",
    );
    add(
        "hashing",
        "hash; dedupe",
        "8-bit SDR images/directory",
        vec![],
        "available",
        "Which images are near-duplicate candidates?",
        "<=100000 images; hash collisions and transitive clusters require review",
    );
    add(
        "split-review",
        "split-review",
        "declared split manifest; 1..128 8-bit SDR images",
        vec![],
        "available",
        "Which reuse candidates cross splits, and which bursts stay within a split?",
        "hash/geometric candidates; embeddings opt in with provisioned models; a clean list never proves no leakage",
    );
    add(
        "embeddings",
        "similar; index build|query",
        "8-bit SDR images, supplied ONNX contract/cache/runtime",
        vec!["embeddings"],
        if cfg!(feature = "embeddings") {
            "conditional_runtime"
        } else {
            "feature_unavailable"
        },
        "How similar are the supplied model's visual embeddings?",
        "export-inputs/export script and calibrate provide frozen parity/holdout qualification; runtime evidence still required",
    );
    // N18
    add(
        "derivative-sheet",
        "derivative-sheet SOURCE DECLARATION --out DIR",
        "8-bit SDR source; declared crops, display sizes and optional renditions",
        vec![],
        "available",
        "Do known subjects and small text survive at declared derivative sizes?",
        "cached faces or declared boxes; no detection is not established; pixel thresholds do not certify human readability; delivered subject content unverified",
    );
    add(
        "gate_sensitivity",
        "sensitivity --catalogue --config --out; optional --before-config",
        "bounded baseline directory, frozen injection catalogue and compare policies",
        vec![],
        "available",
        "Which declared defects does the configured gate miss, and at what tested strengths?",
        "injected sensitivity is not field recall; actual discrete minima, no automatic tolerance changes; sidecar-dependent gates unsupported",
    );
    add(
        "critical-text",
        "critical-text --policy; --a-source --b-source or --ocr",
        "equal-scale rasters, frozen critical-region/string policy and bound text observations",
        vec![],
        "available_imports",
        "Do every declared critical string and pixel threshold survive?",
        "source imports work on stock; cached OCR needs ocr/runtime/models; no human readability or semantic correctness guarantee",
    );
    add(
        "text",
        "text; PP-OCRv5 default with ocr, or --a-source --b-source",
        "images plus image-bound observations",
        vec![],
        "available_imports",
        "Which observed strings/positions changed, or are expected strings readable?",
        "OCR execution requires ocr and pinned runtime/models; confidence is uncertain",
    );
    add(
        "no_reference",
        "assess --compare-to",
        "one 8-bit SDR image; optional reference",
        vec![],
        "available",
        "What blur/noise/block/banding/clipping indicators are present?",
        "content-dependent, no universal quality pass; learned score unavailable",
    );
    add(
        "documents",
        "compare a.svg b.pdf --dpi 96",
        "SVG/PDF at declared DPI, pages streamed",
        vec!["documents"],
        if cfg!(feature = "documents") {
            "available_bounded"
        } else {
            "feature_unavailable"
        },
        "How do corresponding rendered document pages differ?",
        "static path SVG and supported PDF pages; external resources and unavailable fonts refused; renderer-wide qualification pending",
    );
    add(
        "a11y",
        "a11y auto; experiment a11y",
        "rasters with automatic candidates or declared text/coverage regions",
        vec!["prechecks"],
        if cfg!(feature = "prechecks") {
            "available_precheck"
        } else {
            "feature_unavailable"
        },
        "Which contrast/accessibility prechecks require attention?",
        "not accessibility certification or formal compliance",
    );
    add(
        "performance_statistics",
        "prove performance; history stats/onset",
        "qualified timing sidecars/repeats",
        vec!["graphics"],
        if cfg!(feature = "graphics") {
            "available"
        } else {
            "feature_unavailable"
        },
        "Do comparable repeats support a performance claim?",
        "source/clock/noise qualification is independent of image thresholds",
    );
    add(
        "ai_assist",
        "review",
        "measured evidence/closed requests",
        vec!["ai"],
        if cfg!(feature = "ai") {
            "conditional_provider"
        } else {
            "feature_unavailable"
        },
        "What advisory explanations should a human inspect?",
        "startup/egress/budget authority required; never approval or deterministic override",
    );
    add(
        "single_image_integrity",
        "inspect-image",
        "one SDR image; optional archive and OCR observations",
        vec![],
        "partial",
        "What provenance/integrity/publication indicators can be checked?",
        "offline C2PA needs credentials; forensic specificity unqualified; no real/fake verdict",
    );
    add(
        "batch_intake",
        "batch INPUTS --out RESULTS",
        "folder or versioned intake manifest; paired references optional",
        vec![],
        "available",
        "Which inputs completed, failed or remain partial?",
        "bounded concurrency/deadlines; immutable receipts; JSONL/CSV/thumbnail index; no approval",
    );
    add(
        "experimental_assist",
        "assist explain|audit-mask|check-ui|batch; review explain|audit-mask|check-ui|assist",
        "measured reports, source-bound requests and immutable sidecars",
        vec!["assist"],
        if cfg!(feature = "assist") {
            "experimental_unqualified"
        } else {
            "feature_unavailable"
        },
        "What advisory visible-condition or exclusion evidence requires review?",
        "--experimental; provider authority and budget required; never overrides deterministic measurements",
    );
    add(
        "local_vision",
        "locate; faces; crop-check; assess --faces; inspect-image --faces",
        "images plus shared registry/cache and explicit CPU runtime",
        vec!["local-models"],
        if cfg!(feature = "local-models") {
            "conditional_runtime"
        } else {
            "feature_unavailable"
        },
        "Where are detected objects or faces, and which crops retain them?",
        "source/export parity and domain accuracy remain separate; no identity recognition",
    );
    add(
        "watermark_decoding",
        "watermark; inspect-image --watermark",
        "one image; known legacy payload optional",
        vec![],
        "partial",
        "Do named compatible decoders recover a marker?",
        "TrustMark complete decoding unavailable; absence establishes no origin verdict",
    );
    add(
        "hosted_vision_mapping",
        "review check-ui --vision-provider; provider-map",
        "exact image-bound request and explicit recorded provider response",
        vec!["assist", "vision-providers"],
        if cfg!(all(feature = "assist", feature = "vision-providers")) {
            "fixture_only"
        } else {
            "feature_unavailable"
        },
        "What advisory observations does the named adapter map?",
        "no live qualification or provider calls; never changes deterministic authority",
    );
    add(
        "browser_assertions",
        "saccade-playwright toMatchSaccade",
        "Playwright page or locator and retained baseline",
        vec![],
        "conditional_browser",
        "Does the stabilized browser capture pass its declared comparison?",
        "external Playwright peer/browser required; masks are declared; registration is opt-in",
    );
    add(
        "page_sweeps",
        "sweep plan; sweep compare",
        "URL manifest and complete capture receipts",
        vec!["products"],
        if cfg!(feature = "products") {
            "available"
        } else {
            "feature_unavailable"
        },
        "Which sampled pages differ?",
        "capture errors remain failures; registration is explicit",
    );
    add(
        "image_delivery",
        "imgtune audit; imgtune search",
        "source images and bounded transform adapter grids",
        vec!["products"],
        if cfg!(feature = "products") {
            "available"
        } else {
            "feature_unavailable"
        },
        "Which served encodings meet the declared perceptual target?",
        "incomplete grids cannot select; AVIF requires imgtune-avif and dav1d",
    );
    add(
        "design_source",
        "design pull; design compare",
        "design exports, mappings and implementation captures",
        vec!["products"],
        if cfg!(feature = "products") {
            "conditional_source"
        } else {
            "feature_unavailable"
        },
        "How do design frames and captured implementation differ?",
        "live adapter needs explicit egress; missing variable access degrades coverage",
    );
    add(
        "notifications_history",
        "notify; compare --baseline last-good; sweep compare --baseline last-good",
        "verified passing run or report summary",
        vec!["products"],
        if cfg!(feature = "products") {
            "conditional_authority"
        } else {
            "feature_unavailable"
        },
        "Which verified passing run or explicit summary should be used?",
        "passing history is not human approval; sending needs explicit authorization",
    );
    // wave9: deterministic evidence commands and opt-in policies.
    add(
        "rendering_evidence",
        "compare --config (required_effects, spatial, layers); compare --intended-variable",
        "paired captures, declared effect occupancy, layer sidecars and experiment metadata",
        vec![],
        "available",
        "Is the required effect present, and is the difference texture or clustered structural bias?",
        "opt-in policies; graphics also enables experiment ablate --intended-variable; zero FLIP proves no effect; structural classes do not qualify timing",
    );
    add(
        "temporal_tiles",
        "experiment sequence --config (temporal_tiles)",
        "fixed-camera frame sequences and declared provenance",
        vec!["graphics"],
        if cfg!(feature = "graphics") {
            "available"
        } else {
            "feature_unavailable"
        },
        "Did per-tile temporal shimmer increase under qualified fixed-camera conditions?",
        "moving cameras are refused; missing qualification remains explicit",
    );
    add(
        "noisy_reference",
        "experiment reference",
        "render, offline reference, optional independent seeds or sample-mean variance and mask",
        vec![],
        "available",
        "Does the render differ beyond the declared reference noise floor?",
        "input hashes and fitted exposure retained; sample-mean variance is not a universal tolerance",
    );
    add(
        "blind_trials",
        "review trial register|start|vote|import",
        "immutable preregistered plan, image pairs and explicit judgments",
        vec![],
        "available",
        "What preferences were recorded against a hash-bound blind trial?",
        "only public galleries go to judges; preferences grant no baseline or performance approval",
    );
    let mut features: Vec<_> = saccade_core::COMPILED_FEATURES
        .iter()
        .map(|s| s.to_string())
        .collect();
    if cfg!(feature = "ocr") {
        features.push("ocr".into());
    }
    if cfg!(feature = "ocr-provider") {
        features.push("ocr-provider".into());
    }
    if cfg!(feature = "mcp") {
        features.push("mcp".into());
    }
    if cfg!(feature = "products") {
        features.push("products".into());
    }
    if cfg!(feature = "geo") {
        features.push("geo".into());
    }
    if cfg!(feature = "print") {
        features.push("print".into());
    }
    if cfg!(feature = "imgtune-avif") {
        features.push("imgtune-avif".into());
    }
    features.sort();
    features.dedup();
    json!({"schema":SCHEMA,"operation":"capabilities","compiled_features":features,"families":families,"routing":{"same-render":"compare or explicit registration","same-content":"supplied-model embeddings","same-text":"image-bound text/OCR observations","near-duplicate":"pHash Hamming pair search","quality":"paired no-reference measures; no universal pass"},"limitations":["available means executable implementation, not broad qualification","conditional runtimes/providers need their declared artifacts and authority","deferred families never fall back to a different question"]})
}
pub(crate) fn run(args: Args) -> Result<u8, CliError> {
    let value = catalogue();
    if args.json {
        crate::emit(&format!("{}\n", serde_json::to_string_pretty(&value)?))?;
    } else {
        for family in value["families"].as_array().into_iter().flatten() {
            crate::emit(&format!(
                "{}: {} — {}\n",
                family["family"].as_str().unwrap_or("unknown"),
                family["status"].as_str().unwrap_or("unknown"),
                family["command"].as_str().unwrap_or("unknown")
            ))?;
        }
    }
    Ok(0)
}
fn choice(question: Question, family: &str, command: &str, reason: &str) -> Value {
    json!({"schema":CHOICE_SCHEMA,"question":question.name(),"family":family,"command":command,"reason":reason,"fallback":"none"})
}
pub(crate) fn validate(options: &general_cmd::CompareArgs) -> Result<(), CliError> {
    if options.resample.is_some() && options.align.is_none() {
        return Err(CliError::usage(
            "resample requires an explicit alignment policy",
        ));
    }
    if options.question != Some(Question::SameContent)
        && (options.model.is_some() || options.cache.is_some() || options.library.is_some())
    {
        return Err(CliError::usage(
            "model/cache/library apply only to --question same-content",
        ));
    }
    if options.question != Some(Question::SameText)
        && (options.reference_source.is_some()
            || options.capture_source.is_some()
            || options.ocr_contract.is_some())
    {
        return Err(CliError::usage(
            "source/OCR options apply only to --question same-text",
        ));
    }
    if options.question.is_some_and(|q| q != Question::SameRender) && options.align.is_some() {
        return Err(CliError::usage(
            "alignment applies only to same-render; other questions never silently discard it",
        ));
    }
    Ok(())
}
/// The immutable ordinary report remains intact; a hash-bound report component records the route.
pub(crate) fn record_render(
    report: &saccade_core::Report,
    out: &Path,
    options: &general_cmd::CompareArgs,
) -> Result<Value, CliError> {
    let family = if options.align.is_some() {
        "registration"
    } else {
        "pixel_perceptual"
    };
    let mut value = choice(
        Question::SameRender,
        family,
        "compare",
        "explicit same-render selection; selected metric and evidence policies remain authoritative",
    );
    let file = out.join(saccade_core::report::REPORT_FILE_NAME);
    value["measurement"] = json!({"schema":saccade_core::report::REPORT_SCHEMA,"path":saccade_core::report::REPORT_FILE_NAME,"sha256":input::sha256(&file,128*1024*1024)?,"verdict":if report.is_regression(){"regression"}else{"pass"}});
    general_cmd::write_new(&out.join(CHOICE_FILE), &value)?;
    Ok(value)
}
type Pair = (String, Option<PathBuf>, Option<PathBuf>);
fn pairs(a: &Path, b: &Path) -> Result<Vec<Pair>, CliError> {
    if a.is_file() && b.is_file() {
        return Ok(vec![(
            b.file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| CliError::usage("filenames must be UTF-8"))?
                .into(),
            Some(a.into()),
            Some(b.into()),
        )]);
    }
    if !a.is_dir() || !b.is_dir() {
        return Err(CliError::usage("both inputs must be files or directories"));
    }
    let files = |root: &Path| -> Result<std::collections::BTreeMap<String, PathBuf>, CliError> {
        let mut out = std::collections::BTreeMap::new();
        for path in input::files(root, 100000)? {
            let name = path
                .strip_prefix(root)
                .map_err(|e| CliError::io(e.to_string()))?
                .to_str()
                .ok_or_else(|| CliError::usage("filenames must be UTF-8"))?
                .replace('\\', "/");
            out.insert(name, path);
        }
        Ok(out)
    };
    let aa = files(a)?;
    let bb = files(b)?;
    let path_bytes = aa
        .values()
        .chain(bb.values())
        .map(|p| p.as_os_str().len())
        .sum::<usize>();
    if path_bytes > 64 * 1024 * 1024 {
        return Err(CliError::usage("pair paths exceed 64 MiB"));
    }
    let names: std::collections::BTreeSet<_> = aa.keys().chain(bb.keys()).cloned().collect();
    if names.len() > 100000 {
        return Err(CliError::usage(
            "routed comparison supports at most 100000 pairs",
        ));
    }
    Ok(names
        .into_iter()
        .map(|name| {
            let a = aa.get(&name).cloned();
            let b = bb.get(&name).cloned();
            (name, a, b)
        })
        .collect())
}
pub(crate) fn measure(
    a: &Path,
    b: &Path,
    out: &Path,
    options: &general_cmd::CompareArgs,
    threshold: Option<f64>,
) -> Result<Value, CliError> {
    validate(options)?;
    let question = options
        .question
        .ok_or_else(|| CliError::usage("question required"))?;
    let mut value = match question {
        Question::SameRender => {
            return Err(CliError::usage(
                "same-render uses the normal comparison route",
            ));
        }
        Question::SameContent => {
            if threshold.is_some() {
                return Err(CliError::usage(
                    "semantic cosine is uncalibrated; no universal threshold",
                ));
            }
            if !a.is_file() || !b.is_file() {
                return Err(CliError::usage(
                    "same-content routing currently requires a file pair; use index build/query for directories",
                ));
            }
            crate::embedding_cmd::routed(
                a,
                b,
                options
                    .model
                    .as_deref()
                    .ok_or_else(|| CliError::usage("same-content requires --model"))?,
                options
                    .cache
                    .as_deref()
                    .ok_or_else(|| CliError::usage("same-content requires --cache"))?,
                options
                    .library
                    .as_deref()
                    .ok_or_else(|| CliError::usage("same-content requires --library"))?,
                out,
            )?
        }
        Question::SameText => {
            if threshold.is_some() {
                return Err(CliError::usage(
                    "FLIP threshold does not apply to observed text equality",
                ));
            }
            if !a.is_file() || !b.is_file() {
                return Err(CliError::usage(
                    "same-text routing currently requires a file pair and image-bound sources or OCR contract",
                ));
            }
            crate::text_cmd::routed(
                a,
                b,
                options.reference_source.as_deref(),
                options.capture_source.as_deref(),
                options.ocr_contract.as_deref(),
                out,
            )?
        }
        Question::NearDuplicate | Question::Quality => {
            if question == Question::Quality && threshold.is_some() {
                return Err(CliError::usage(
                    "quality measures are content-dependent; no universal threshold",
                ));
            }
            let t = threshold.unwrap_or(6.);
            if question == Question::NearDuplicate
                && (!t.is_finite() || !(0.0..=64.).contains(&t) || t.fract() != 0.)
            {
                return Err(CliError::usage(
                    "near-duplicate threshold is an integer Hamming radius 0..64",
                ));
            }
            let pairs = pairs(a, b)?;
            general_cmd::prepare_out(out, &[a, b])?;
            let mut entries = Vec::new();
            let mut failed = 0;
            let mut errors = 0;
            for (i, (name, aa, bb)) in pairs.into_iter().enumerate() {
                let (Some(aa), Some(bb)) = (aa, bb) else {
                    failed += 1;
                    entries.push(json!({"name":name,"status":"missing_or_new"}));
                    continue;
                };
                let measurement = (|| -> Result<Value, CliError> {
                    if question == Question::NearDuplicate {
                        let aa = input::bytes(&aa, input::MAX_BYTES)?;
                        let bb = input::bytes(&bb, input::MAX_BYTES)?;
                        let ha = hashing::hash(&input::decode(&aa)?);
                        let hb = hashing::hash(&input::decode(&bb)?);
                        let distance = (ha.phash ^ hb.phash).count_ones();
                        Ok(
                            json!({"name":name,"status":if distance>t as u32{"fail"}else{"pass"},"a_sha256":saccade_core::localized::digest(&aa),"b_sha256":saccade_core::localized::digest(&bb),"a_phash":format!("{:016x}",ha.phash),"b_phash":format!("{:016x}",hb.phash),"hamming":distance,"threshold":t as u32}),
                        )
                    } else {
                        let dir = out.join(format!("pair-{i:06}"));
                        let value = crate::assess_cmd::measure(&bb, Some(&aa), &dir)?;
                        let artifact = crate::general_cmd::persist_document(&value, &dir)?;
                        Ok(
                            json!({"name":name,"status":"unknown","measurement":value,"artifact":artifact.file_name().and_then(|n|n.to_str()),"directory":format!("pair-{i:06}")}),
                        )
                    }
                })();
                match measurement {
                    Ok(v) => {
                        if v["status"] == "fail" {
                            failed += 1;
                        }
                        entries.push(v);
                    }
                    Err(e) => {
                        failed += 1;
                        errors += 1;
                        entries.push(json!({"name":name,"status":"error","error":{"code":e.code,"message":e.message}}));
                    }
                }
            }
            if entries.is_empty() {
                failed += 1;
            }
            json!({"schema":if question==Question::NearDuplicate{NEAR_SCHEMA}else{QUESTION_SCHEMA},"operation":question.name(),"verdict":if failed>0{"regression"}else if question==Question::Quality{"unknown"}else{"pass"},"counts":{"total":entries.len(),"failures":failed,"errors":errors},"entries":entries,"limitations":["hash similarity is candidate retrieval; collisions can pass","quality has no universal good/bad threshold","missing/new/decode failures are retained; no fallback pipeline"]})
        }
    };
    let (family, command, reason) = match question {
        Question::SameContent => (
            "embeddings",
            "similar",
            "explicit same-content selects supplied-model cosine; calibration remains conditional",
        ),
        Question::SameText => (
            "text",
            "text",
            "explicit same-text selects image-bound text/OCR observations",
        ),
        Question::NearDuplicate => (
            "hashing",
            "hash pair",
            "explicit near-duplicate selects pHash Hamming distance; threshold units are bits",
        ),
        Question::Quality => (
            "no_reference",
            "assess --compare-to",
            "explicit quality selects paired content-dependent indicators; never a universal pass",
        ),
        Question::SameRender => ("pixel_perceptual", "compare", "same-render"),
    };
    value["pipeline_choice"] = choice(question, family, command, reason);
    Ok(value)
}
pub(crate) fn route(
    a: &Path,
    b: &Path,
    out: &Path,
    options: &general_cmd::CompareArgs,
    threshold: Option<f64>,
    json_output: bool,
) -> Result<u8, CliError> {
    let value = measure(a, b, out, options, threshold)?;
    general_cmd::emit_document(value, Some(out), json_output)
}
#[cfg(feature = "mcp")]
pub(crate) fn schemas() -> Vec<Value> {
    vec![
        json!({"type":"object","properties":{"operation":{"const":"capabilities","type":"string"}},"required":["operation"],"additionalProperties":false}),
        json!({"type":"object","properties":{"operation":{"const":"compare_question","type":"string"},"reference":{"type":"string"},"capture":{"type":"string"},"out":{"type":"string"},"question":{"enum":["same-render","same-content","same-text","near-duplicate","quality"]},"align":{"enum":["none","translation","similarity","affine","homography","auto"]},"resample":{"enum":["reference","common"]},"threshold":{"type":"number"},"model":{"type":"string"},"cache":{"type":"string"},"library":{"type":"string"},"reference_source":{"type":"string"},"capture_source":{"type":"string"}},"required":["operation","reference","capture","out","question"],"additionalProperties":false}),
    ]
}

/// Discoverable companions; availability and authority remain in the catalogue.
pub(crate) fn related_commands() -> Value {
    json!([
        "capabilities --json",
        "review explain",
        "review audit-mask",
        "review check-ui",
        "review assist batch submit|status|collect",
        "sweep plan|compare",
        "imgtune audit|search",
        "design pull|compare",
        "notify",
        "compare --baseline last-good",
        "analyze-media --json",
        "experiment reference",
        "review trial register|start|vote|import"
    ])
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    #[test]
    fn wave9_families_retain_commands_limits_and_feature_availability() {
        let catalogue = super::catalogue();
        let families = catalogue["families"].as_array().unwrap();
        for (name, command) in [
            ("rendering_evidence", "compare --config"),
            ("noisy_reference", "experiment reference"),
            ("blind_trials", "review trial register|start|vote|import"),
            ("temporal_tiles", "experiment sequence"),
        ] {
            let family = families.iter().find(|v| v["family"] == name).unwrap();
            assert!(family["command"].as_str().unwrap().contains(command));
            assert!(!family["limits"].as_str().unwrap().is_empty());
            assert_eq!(
                family["status"],
                if name == "temporal_tiles" && !cfg!(feature = "graphics") {
                    "feature_unavailable"
                } else {
                    "available"
                }
            );
        }
    }
}
