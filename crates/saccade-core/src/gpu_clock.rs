//! Optional GPU clock and power-state evidence, with configurable telemetry mapping.
#![allow(missing_docs)]

use serde::{Deserialize, Serialize};
use std::{io::Read, path::Path};

pub mod map;

pub const FILE: &str = "gpu_clock.json";

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClockRange {
    pub min: f64,
    pub median: f64,
    pub max: f64,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClockWindow {
    pub name: String,
    pub core_mhz: ClockRange,
    pub memory_mhz: Option<ClockRange>,
    pub sample_count: u64,
    pub expected_frames: u64,
    pub observed_frames: u64,
    pub query_failures: u64,
    #[serde(default)]
    pub throttle_reasons: Vec<String>,
    pub stabilized: bool,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GpuClock {
    pub schema: String,
    pub device_id: String,
    /// Explicit producer power state, e.g. `ac-performance`. Unknown is absent.
    pub power_state: Option<String>,
    pub windows: Vec<ClockWindow>,
}

impl GpuClock {
    pub fn read(dir: &Path) -> Result<Option<Self>, String> {
        Self::read_mapped(dir, None)
    }

    /// Read a bounded telemetry sidecar using an explicit map or native/legacy dispatch.
    pub fn read_mapped(
        dir: &Path,
        mapping: Option<&map::ClockMap>,
    ) -> Result<Option<Self>, String> {
        let path = dir.join(FILE);
        if !path.exists() {
            return Ok(None);
        }
        // Preserve regular-file symlink inputs while bounding allocation before parsing.
        let meta = std::fs::metadata(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if !meta.is_file() || meta.len() > 16 << 20 {
            return Err(format!(
                "{}: GPU clock sidecar must be a regular file of at most 16 MiB",
                path.display()
            ));
        }
        let mut bytes = Vec::new();
        std::fs::File::open(&path)
            .map_err(|e| format!("{}: {e}", path.display()))?
            .take((16 << 20) + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if bytes.len() > 16 << 20 {
            return Err(format!(
                "{}: GPU clock sidecar exceeds 16 MiB",
                path.display()
            ));
        }
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        let model = if let Some(mapping) = mapping {
            mapping.apply(&value)?
        } else {
            match value
                .get("schema")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
            {
                "saccade-gpu-clock.v1" => serde_json::from_value::<Self>(value)
                    .map_err(|e| format!("{}: {e}", path.display()))?,
                "moss.gpu-clock.v2" => {
                    eprintln!(
                        "warning: legacy GPU clock schema is deprecated; supply --gpu-clock-map FILE (or gpu_clock_map in config); removed in 0.3.0"
                    );
                    legacy_map()?.apply(&value)?
                }
                schema => {
                    return Err(format!(
                        "{}: unsupported GPU clock schema {schema:?}; supply saccade-gpu-clock.v1 or --gpu-clock-map FILE",
                        path.display()
                    ));
                }
            }
        };
        Ok(Some(model))
    }

    pub fn qualification_reasons(&self) -> Vec<String> {
        let mut reasons = Vec::new();
        if self.device_id.is_empty() {
            reasons.push("GPU clock device identity is empty".into());
        }
        if self.power_state.as_deref().is_none_or(|s| {
            matches!(
                s.trim().to_ascii_lowercase().as_str(),
                "" | "unknown" | "unspecified" | "none" | "n/a" | "unavailable"
            )
        }) {
            reasons.push("GPU power state is absent or unknown".into());
        }
        if self.windows.is_empty() {
            reasons.push("GPU clock has no sample windows".into());
        }
        for w in &self.windows {
            if w.name.is_empty() {
                reasons.push("GPU clock window name is empty".into());
            }
            if w.sample_count == 0
                || w.expected_frames == 0
                || w.observed_frames != w.expected_frames
            {
                reasons.push(format!(
                    "GPU clock {}: incomplete sample window ({} of {} frames, {} samples)",
                    w.name, w.observed_frames, w.expected_frames, w.sample_count
                ));
            }
            if w.query_failures > 0 {
                reasons.push(format!(
                    "GPU clock {}: {} query failures",
                    w.name, w.query_failures
                ));
            }
            if !w.stabilized {
                reasons.push(format!(
                    "GPU clock {}: warm-to-boost stability is unqualified",
                    w.name
                ));
            }
            if !w.throttle_reasons.is_empty() {
                reasons.push(format!(
                    "GPU clock {}: throttling evidence {}",
                    w.name,
                    w.throttle_reasons.join(", ")
                ));
            }
            for (kind, range) in [
                ("core", Some(&w.core_mhz)),
                ("memory", w.memory_mhz.as_ref()),
            ] {
                if let Some(r) = range {
                    if !r.min.is_finite()
                        || !r.median.is_finite()
                        || !r.max.is_finite()
                        || r.min <= 0.0
                        || r.min > r.median
                        || r.median > r.max
                    {
                        reasons.push(format!("GPU clock {}: invalid {kind} MHz range", w.name));
                    } else if (r.max - r.min) / r.median > 0.05 {
                        reasons.push(format!("GPU clock {}: {kind} MHz range {:.0}..{:.0} exceeds 5% of median {:.0}", w.name, r.min, r.max, r.median));
                    }
                }
            }
        }
        reasons
    }
}

/// Reject present but incomplete, unqualified, or unequal clock evidence.
pub fn compare(dirs: &[&Path]) -> (Vec<Option<GpuClock>>, Vec<String>) {
    compare_required(dirs, true)
}

pub fn compare_required(dirs: &[&Path], required: bool) -> (Vec<Option<GpuClock>>, Vec<String>) {
    compare_mapped(dirs, required, None)
}

/// Qualify and compare runs using the same optional producer telemetry map.
pub fn compare_mapped(
    dirs: &[&Path],
    required: bool,
    map_path: Option<&Path>,
) -> (Vec<Option<GpuClock>>, Vec<String>) {
    let mapping = match map_path.map(map::ClockMap::read).transpose() {
        Ok(mapping) => mapping,
        Err(reason) => return (vec![None; dirs.len()], vec![reason]),
    };
    let mut clocks = Vec::new();
    let mut reasons = Vec::new();
    for dir in dirs {
        match GpuClock::read_mapped(dir, mapping.as_ref()) {
            Ok(clock) => clocks.push(clock),
            Err(reason) => {
                reasons.push(reason);
                clocks.push(None);
            }
        }
    }
    if clocks.iter().all(Option::is_none) && reasons.is_empty() && !required {
        return (clocks, reasons);
    }
    for (i, clock) in clocks.iter().enumerate() {
        match clock {
            Some(c) => reasons.extend(
                c.qualification_reasons()
                    .into_iter()
                    .map(|r| format!("GPU clock run {i}: {r}")),
            ),
            None => reasons.push(format!("GPU clock run {i}: {FILE} is absent")),
        }
    }
    if let Some(first) = clocks.first().and_then(Option::as_ref) {
        for (i, other) in clocks.iter().enumerate().skip(1) {
            if let Some(other) = other {
                if first.device_id != other.device_id {
                    reasons.push(format!(
                        "GPU clock run {i}: device differs: {:?} vs {:?}",
                        first.device_id, other.device_id
                    ));
                }
                if first.power_state != other.power_state {
                    reasons.push(format!(
                        "GPU clock run {i}: power_state differs: {:?} vs {:?}",
                        first.power_state, other.power_state
                    ));
                }
                for window in &first.windows {
                    match other.windows.iter().find(|w| w.name == window.name) {
                        Some(w) => {
                            if window.core_mhz.median != w.core_mhz.median {
                                reasons.push(format!(
                                    "GPU clock run {i}: {} core median MHz differs: {:.0} vs {:.0}",
                                    window.name, window.core_mhz.median, w.core_mhz.median
                                ));
                            }
                            if window.memory_mhz.as_ref().map(|r| r.median)
                                != w.memory_mhz.as_ref().map(|r| r.median)
                            {
                                reasons.push(format!(
                                    "GPU clock run {i}: {} memory median MHz differs",
                                    window.name
                                ));
                            }
                        }
                        None => reasons.push(format!(
                            "GPU clock run {i}: window {:?} is absent",
                            window.name
                        )),
                    }
                }
                if first.windows.len() != other.windows.len() {
                    reasons.push(format!("GPU clock run {i}: window sets differ"));
                }
            }
        }
    }
    (clocks, reasons)
}

// Temporary protocol compatibility only; remove with the 0.3.0 release.
fn legacy_map() -> Result<map::ClockMap, String> {
    let mut mapping: map::ClockMap =
        toml::from_str(include_str!("gpu_clock/legacy-map.toml")).map_err(|e| e.to_string())?;
    if let Some(source) = mapping.windows.fields.get_mut("throttle_reasons") {
        source.prefix = Some("Moss throttle mask ".into());
    }
    Ok(mapping)
}

#[cfg(all(test, feature = "graphics"))]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn absent_clocks_and_unknown_power_state_cannot_qualify() {
        let temp = tempfile::tempdir().unwrap();
        let dirs = [temp.path().join("base"), temp.path().join("arm")];
        for dir in &dirs {
            std::fs::create_dir_all(dir).unwrap();
        }
        let (_, reasons) = compare(&[&dirs[0], &dirs[1]]);
        assert!(
            reasons
                .iter()
                .any(|r| r.contains("gpu_clock.json is absent"))
        );
        assert!(compare_required(&[&dirs[0], &dirs[1]], false).1.is_empty());
        let clock = json!({"schema":"saccade-gpu-clock.v1","device_id":"gpu-1","power_state":"unknown","windows":[{"name":"frame","core_mhz":{"min":1000.0,"median":1000.0,"max":1000.0},"memory_mhz":null,"sample_count":20,"expected_frames":4,"observed_frames":4,"query_failures":0,"throttle_reasons":[],"stabilized":true}]});
        for dir in &dirs {
            std::fs::write(dir.join(FILE), clock.to_string()).unwrap();
        }
        let (_, reasons) = compare(&[&dirs[0], &dirs[1]]);
        assert!(
            reasons
                .iter()
                .any(|r| r.contains("power state is absent or unknown"))
        );
    }

    #[test]
    fn clock_mismatch_and_unqualified_state_reject_performance_pair() {
        let temp = tempfile::tempdir().unwrap();
        let dirs = [temp.path().join("base"), temp.path().join("arm")];
        for (i, dir) in dirs.iter().enumerate() {
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(dir.join("saccade-perf.json"), json!({"schema":"saccade-perf.v1","unit":"ms","frame":{"value":10.0,"samples":4,"stat":"p50"},"terms":[],"counters":{}}).to_string()).unwrap();
            let mhz = if i == 0 { 1000.0 } else { 900.0 };
            std::fs::write(dir.join(FILE), json!({"schema":"saccade-gpu-clock.v1","device_id":"gpu-1","power_state":"ac-performance","windows":[{"name":"frame","core_mhz":{"min":mhz,"median":mhz,"max":mhz},"memory_mhz":null,"sample_count":20,"expected_frames":4,"observed_frames":4,"query_failures":0,"throttle_reasons":[],"stabilized":true}]}).to_string()).unwrap();
        }
        let (diff, errors) =
            crate::perf::pair(&dirs[0], &dirs[1], &crate::perf::PerfOptions::default()).unwrap();
        assert!(errors.is_empty());
        let diff = diff.unwrap();
        assert_eq!(diff.comparability, crate::perf::Comparability::Rejected);
        assert!(
            diff.qualification_reasons
                .iter()
                .any(|r| r.contains("core median MHz differs"))
        );
        let mut arm: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dirs[1].join(FILE)).unwrap()).unwrap();
        arm["windows"][0]["stabilized"] = false.into();
        std::fs::write(dirs[1].join(FILE), arm.to_string()).unwrap();
        let (_, reasons) = compare(&[&dirs[0], &dirs[1]]);
        assert!(
            reasons
                .iter()
                .any(|r| r.contains("warm-to-boost stability is unqualified"))
        );
        for dir in &dirs {
            std::fs::remove_file(dir.join(FILE)).unwrap();
        }
        let (diff, errors) =
            crate::perf::pair(&dirs[0], &dirs[1], &crate::perf::PerfOptions::default()).unwrap();
        assert!(errors.is_empty());
        let diff = diff.unwrap();
        assert_eq!(diff.comparability, crate::perf::Comparability::Rejected);
        assert!(
            diff.qualification_reasons
                .iter()
                .any(|r| r.contains("gpu_clock.json is absent"))
        );
    }
}
