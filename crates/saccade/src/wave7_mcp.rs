//! Root-contained wave 7 fixture observations on the existing six-tool interface.
use crate::agent::CliError;
use crate::wave7_cmd::error;
use saccade_core::{
    root_policy::RootPolicy,
    wave7::{
        faces::{self, CropSpec, FaceReport},
        models::{self, Registry, VisionError},
        quality::{LearnedMetric, QualityReport},
        vision::{LocateReport, VisionImage},
        watermark::{self, DwtConfig, WatermarkReport},
    },
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::PathBuf;
#[derive(Deserialize)]
#[serde(tag = "operation", deny_unknown_fields)]
enum Operation {
    #[serde(rename = "models_list")]
    Models { registry: PathBuf, cache: PathBuf },
    #[serde(rename = "models_pull")]
    Pull { id: String },
    #[serde(rename = "vision_locate")]
    Locate {
        image: PathBuf,
        phrase: String,
        observations: PathBuf,
        #[serde(default)]
        segment: bool,
        overlay: PathBuf,
    },
    #[serde(rename = "vision_quality")]
    Quality {
        image: PathBuf,
        reference: Option<PathBuf>,
        metric: LearnedMetric,
        observations: PathBuf,
    },
    #[serde(rename = "vision_faces")]
    Faces {
        image: PathBuf,
        observations: PathBuf,
        blur_faces: Option<PathBuf>,
    },
    #[serde(rename = "vision_crop")]
    Crop {
        image: PathBuf,
        observations: PathBuf,
        crops: Vec<CropSpec>,
        focal_point: Option<[f32; 2]>,
        blur_faces: Option<PathBuf>,
    },
    #[serde(rename = "vision_watermark")]
    Watermark {
        image: PathBuf,
        observations: Option<PathBuf>,
        expected_payload: Option<Vec<u8>>,
    },
    #[cfg(feature = "local-vlm")]
    #[serde(rename = "vision_local")]
    Local {
        request: PathBuf,
        response: PathBuf,
        endpoint: String,
        runtime_revision: String,
    },
    #[cfg(feature = "vision-providers")]
    #[serde(rename = "vision_provider")]
    Provider {
        request: PathBuf,
        response: PathBuf,
        provider: saccade_core::wave7::providers::Provider,
        coordinates: saccade_core::wave7::providers::Coordinates,
    },
}
pub(crate) fn handles(name: &str, operation: &str) -> bool {
    match name {
        "saccade_inspect" => operation == "models_list",
        "saccade_measure" => {
            matches!(
                operation,
                "models_pull"
                    | "vision_locate"
                    | "vision_quality"
                    | "vision_faces"
                    | "vision_crop"
                    | "vision_watermark"
            ) || (cfg!(feature = "local-vlm") && operation == "vision_local")
                || (cfg!(feature = "vision-providers") && operation == "vision_provider")
        }
        _ => false,
    }
}
fn input(policy: &RootPolicy, p: PathBuf) -> Result<PathBuf, CliError> {
    let p = policy.read(&p)?;
    if !p.is_file() {
        return Err(CliError::usage("vision input must be a file"));
    }
    Ok(p)
}
fn read<T: serde::de::DeserializeOwned>(
    policy: &RootPolicy,
    p: PathBuf,
    limit: u64,
) -> Result<T, CliError> {
    Ok(serde_json::from_slice(
        &models::read_bounded(&input(policy, p)?, limit).map_err(error)?,
    )?)
}
fn image(policy: &RootPolicy, p: PathBuf) -> Result<VisionImage, CliError> {
    VisionImage::load(&input(policy, p)?).map_err(error)
}
fn face(policy: &RootPolicy, p: PathBuf, image: &VisionImage) -> Result<FaceReport, CliError> {
    let mut r: FaceReport = read(policy, p, 1024 * 1024)?;
    r.validate(image).map_err(error)?;
    r.provenance.runtime = "replay".into();
    r.provenance.source_parity = false;
    Ok(r)
}
fn redaction(
    policy: &RootPolicy,
    p: Option<PathBuf>,
    image: &VisionImage,
    r: &FaceReport,
) -> Result<(), CliError> {
    if let Some(p) = p {
        crate::wave7_cmd::write_png(
            &policy.write(&p)?,
            &faces::blur_faces(image, r).map_err(error)?,
        )?;
    }
    Ok(())
}
pub(crate) fn call(
    policy: &RootPolicy,
    args: &serde_json::Map<String, Value>,
) -> Result<Value, CliError> {
    let operation: Operation = serde_json::from_value(Value::Object(args.clone()))?;
    match operation {
        Operation::Models { registry, cache } => {
            let r = Registry::load(&input(policy, registry)?).map_err(error)?;
            let c = policy.read(&cache)?;
            if !c.is_dir() {
                return Err(CliError::usage("cache must be a registered directory"));
            }
            for m in &r.models {
                for a in &m.artifacts {
                    let path = models::artifact_path(&c, a).map_err(error)?;
                    if path.exists() {
                        policy.read(&path)?;
                    }
                }
            }
            Ok(r.status(&c))
        }
        Operation::Pull { id } => Err(error(VisionError::Unavailable(format!(
            "{id}: MCP cannot authorize model downloads; use explicit CLI models pull"
        )))),
        Operation::Locate {
            image: p,
            phrase,
            observations,
            segment,
            overlay,
        } => {
            let i = image(policy, p)?;
            let r = LocateReport::replay(&input(policy, observations)?, &i, &phrase, segment)
                .map_err(error)?;
            crate::wave7_cmd::write_png(
                &policy.write(&overlay)?,
                &saccade_core::wave7::vision::overlay(&i, &r.detections).map_err(error)?,
            )?;
            Ok(serde_json::to_value(r)?)
        }
        Operation::Quality {
            image: p,
            reference,
            metric,
            observations,
        } => {
            let i = image(policy, p)?;
            let reference = reference.map(|p| image(policy, p)).transpose()?;
            let mut r: QualityReport = read(policy, observations, 1024 * 1024)?;
            r.validate(metric, &i, reference.as_ref()).map_err(error)?;
            for m in &mut r.named_metrics {
                m.provenance.runtime = "replay".into();
                m.provenance.source_parity = false;
            }
            Ok(serde_json::to_value(r)?)
        }
        Operation::Faces {
            image: p,
            observations,
            blur_faces,
        } => {
            let i = image(policy, p)?;
            let r = face(policy, observations, &i)?;
            redaction(policy, blur_faces, &i, &r)?;
            Ok(serde_json::to_value(r)?)
        }
        Operation::Crop {
            image: p,
            observations,
            crops,
            focal_point,
            blur_faces,
        } => {
            let i = image(policy, p)?;
            let r = face(policy, observations, &i)?;
            let crop = faces::crop_check(&i, &r, &crops, focal_point).map_err(error)?;
            redaction(policy, blur_faces, &i, &r)?;
            Ok(serde_json::to_value(crop)?)
        }
        Operation::Watermark {
            image: p,
            observations,
            expected_payload,
        } => {
            let i = image(policy, p)?;
            let r = if let Some(p) = observations {
                let mut r: WatermarkReport = read(policy, p, 1024 * 1024)?;
                r.validate(&i).map_err(error)?;
                for f in &mut r.findings {
                    if let Some(p) = &mut f.provenance {
                        p.runtime = "replay".into();
                        p.source_parity = false;
                    }
                    f.interpretation = format!("Explicit observation replay. {}", f.interpretation);
                }
                r
            } else {
                let c = expected_payload.map(|expected_payload| DwtConfig {
                    expected_payload,
                    quantization_step: 36.,
                    minimum_agreement: 0.9,
                });
                watermark::inspect(&i, None, c.as_ref()).map_err(error)?
            };
            Ok(serde_json::to_value(r)?)
        }
        #[cfg(feature = "local-vlm")]
        Operation::Local {
            request,
            response,
            endpoint,
            runtime_revision,
        } => {
            let r = read(policy, request, 24 * 1024 * 1024)?;
            let p = saccade_core::wave7::local_vlm::LocalVlm {
                endpoint,
                runtime_revision,
            };
            let mut report = p
                .decode(
                    &r,
                    &models::read_bounded(&input(policy, response)?, 1024 * 1024).map_err(error)?,
                )
                .map_err(error)?;
            report.provenance.runtime = "replay".into();
            Ok(serde_json::to_value(report)?)
        }
        #[cfg(feature = "vision-providers")]
        Operation::Provider {
            request,
            response,
            provider,
            coordinates,
        } => {
            let r = read(policy, request, 24 * 1024 * 1024)?;
            let a = saccade_core::wave7::providers::ProviderAdapter {
                provider,
                coordinates,
            };
            Ok(serde_json::to_value(
                a.decode(
                    &r,
                    &models::read_bounded(&input(policy, response)?, 1024 * 1024).map_err(error)?,
                )
                .map_err(error)?,
            )?)
        }
    }
}
fn variant(op: &str, properties: Value, required: &[&str]) -> Value {
    let mut p = properties.as_object().cloned().unwrap_or_default();
    p.insert("operation".into(), json!({"const":op}));
    let mut r = vec!["operation"];
    r.extend_from_slice(required);
    json!({"type":"object","properties":p,"required":r,"additionalProperties":false})
}
pub(crate) fn inspect_schema() -> Value {
    variant(
        "models_list",
        json!({"registry":{"type":"string"},"cache":{"type":"string"}}),
        &["registry", "cache"],
    )
}
pub(crate) fn measure_schemas() -> Vec<Value> {
    let mut v = vec![
        variant("models_pull", json!({"id":{"type":"string"}}), &["id"]),
        variant(
            "vision_locate",
            json!({"image":{"type":"string"},"phrase":{"type":"string","minLength":1,"maxLength":4096},"observations":{"type":"string"},"segment":{"type":"boolean"},"overlay":{"type":"string"}}),
            &["image", "phrase", "observations", "overlay"],
        ),
        variant(
            "vision_quality",
            json!({"image":{"type":"string"},"reference":{"type":["string","null"]},"metric":{"enum":["lpips_alex_v01","dists","musiq_technical"]},"observations":{"type":"string"}}),
            &["image", "metric", "observations"],
        ),
        variant(
            "vision_faces",
            json!({"image":{"type":"string"},"observations":{"type":"string"},"blur_faces":{"type":["string","null"]}}),
            &["image", "observations"],
        ),
        variant(
            "vision_crop",
            json!({"image":{"type":"string"},"observations":{"type":"string"},"crops":{"type":"array","minItems":1,"maxItems":32,"items":{"type":"object"}},"focal_point":{"type":["array","null"],"items":{"type":"number"},"minItems":2,"maxItems":2},"blur_faces":{"type":["string","null"]}}),
            &["image", "observations", "crops"],
        ),
        variant(
            "vision_watermark",
            json!({"image":{"type":"string"},"observations":{"type":["string","null"]},"expected_payload":{"type":["array","null"],"items":{"type":"integer","minimum":0,"maximum":255},"minItems":1,"maxItems":64}}),
            &["image"],
        ),
    ];
    if cfg!(feature = "local-vlm") {
        v.push(variant("vision_local",json!({"request":{"type":"string"},"response":{"type":"string"},"endpoint":{"type":"string"},"runtime_revision":{"type":"string"}}),&["request","response","endpoint","runtime_revision"]));
    }
    if cfg!(feature = "vision-providers") {
        v.push(variant("vision_provider",json!({"request":{"type":"string"},"response":{"type":"string"},"provider":{"enum":["claude","gpt"]},"coordinates":{"enum":["pixels","unit","thousand"]}}),&["request","response","provider","coordinates"]));
    }
    v
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn existing_tool_interface_handles_generated_face_and_crop_receipts() {
        let d = tempfile::tempdir().unwrap();
        let inputs = d.path().join("inputs");
        let output = d.path().join("output");
        std::fs::create_dir(&inputs).unwrap();
        std::fs::create_dir(&output).unwrap();
        let p = inputs.join("image.png");
        image::RgbImage::new(32, 24).save(&p).unwrap();
        let i = VisionImage::load(&p).unwrap();
        let r = FaceReport {
            schema: faces::FACES_SCHEMA.into(),
            image_sha256: i.sha256.clone(),
            image_size: i.size(),
            faces: vec![faces::Face {
                bbox: saccade_core::wave7::vision::Rect {
                    x: 2.,
                    y: 4.,
                    width: 8.,
                    height: 8.,
                },
                score: 0.9,
                landmarks: vec![],
            }],
            provenance: saccade_core::wave7::vision::Provenance::fixture("yunet-2026may"),
            limitations: faces::FACE_LIMIT.into(),
        };
        std::fs::write(inputs.join("faces.json"), serde_json::to_vec(&r).unwrap()).unwrap();
        let server = crate::mcp::Server::new(&[inputs], Some(&output), false, &[]).unwrap();
        let response=server.handle_message(&json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"saccade_measure","arguments":{"operation":"vision_crop","image":"image.png","observations":"faces.json","crops":[{"kind":"ratio","width":1.,"height":1.}]}}})).unwrap();
        assert_eq!(response["result"]["isError"], false);
        assert_eq!(
            response["result"]["structuredContent"]["schema"],
            faces::CROP_SCHEMA
        );
        assert_eq!(
            response["result"]["structuredContent"]["detection"]["provenance"]["runtime"],
            "replay"
        );
        let tools = server
            .handle_message(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}))
            .unwrap();
        assert!(
            tools["result"]["tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t["name"] == "saccade_measure"
                    && t["inputSchema"]["oneOf"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|v| v["properties"]["operation"]["const"] == "vision_crop"))
        );
    }
    #[test]
    fn operations_keep_containment_and_refuse_download_authority() {
        let d = tempfile::tempdir().unwrap();
        let input = d.path().join("inputs");
        let output = d.path().join("outputs");
        std::fs::create_dir(&input).unwrap();
        std::fs::create_dir(&output).unwrap();
        let policy = RootPolicy::new(&[input], Some(&output), false, &[]).unwrap();
        let a = json!({"operation":"vision_watermark","image":"../secret.png"});
        assert!(call(&policy, a.as_object().unwrap()).is_err());
        let a = json!({"operation":"models_pull","id":"yunet-2026may"});
        assert!(
            call(&policy, a.as_object().unwrap())
                .unwrap_err()
                .message
                .contains("cannot authorize")
        );
        let a = json!({"operation":"vision_watermark","image":"x.png","allow_download":true});
        assert!(call(&policy, a.as_object().unwrap()).is_err());
    }
}
