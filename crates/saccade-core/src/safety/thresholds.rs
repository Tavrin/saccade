//! Published criteria and explicit assumptions: the single numeric review table.

use serde::{Deserialize, Serialize};

/// One criterion, including its provenance and offline verification status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Threshold {
    /// Stable name used by the implementation.
    pub name: String,
    /// Numeric value; units are in the citation.
    pub value: f64,
    /// Publication, definition and interpretation.
    pub citation: String,
    /// True for an assumption or a value requiring human source verification.
    pub verify: bool,
}

/// Central table. Published criteria are edition-pinned; this is not certification.
pub const THRESHOLDS: &[(&str, f64, &str, bool)] = &[
    (
        "broadcast_red_wcag_surrogate",
        1.0,
        "ASSUMPTION enabled: BT.1702 saturated-red detection uses the numeric WCAG red definition below; verify against the broadcast edition's wording",
        true,
    ),
    (
        "luminance_change",
        0.1,
        "WCAG 2.x, SC 2.3.1, general flash definition: change >= 0.10 relative luminance",
        false,
    ),
    (
        "darker_luminance",
        0.8,
        "WCAG general flash: darker state < 0.80 relative luminance",
        false,
    ),
    (
        "broadcast_peak_cd_m2",
        200.0,
        "ASSUMPTION: SDR peak white 200 cd/m2; BT.1702-2 Annex 1 requires absolute luminance, which PNG cannot establish",
        true,
    ),
    (
        "broadcast_change_cd_m2",
        20.0,
        "ITU-R BT.1702-2 Annex 1 flashing guidance: change >= 20 cd/m2; verify edition wording",
        true,
    ),
    (
        "broadcast_darker_cd_m2",
        160.0,
        "ITU-R BT.1702-2 Annex 1 flashing guidance: darker state < 160 cd/m2; verify edition wording",
        true,
    ),
    (
        "red_saturation",
        0.8,
        "WCAG general flash and red flash thresholds: R/(R+G+B) >= 0.8; linear RGB as in relative luminance",
        false,
    ),
    (
        "red_excursion",
        20.0,
        "Legacy WCAG 2.0/2.1 red flash working definition (W3C WCAG21-ca): change in max(0,R-G-B)*320 > 20 on BOTH transitions; WCAG 2.2 instead uses CIE 1976 UCS > 0.2; verify required edition",
        true,
    ),
    (
        "red_scale",
        320.0,
        "Legacy WCAG 2.0/2.1 working definition (W3C WCAG21-ca): (R-G-B)*320, negatives set to zero; not the WCAG 2.2 UCS definition",
        true,
    ),
    (
        "max_flashes",
        3.0,
        "WCAG SC 2.3.1 and BT.1702 flashing guidance: no more than three flashes in any one-second period",
        false,
    ),
    (
        "window_seconds",
        1.0,
        "WCAG SC 2.3.1: any one-second period, evaluated at every frame",
        false,
    ),
    (
        "broadcast_area_fraction",
        0.25,
        "ITU-R BT.1702-2/-3 Annex 1: combined concurrent flashing area > one quarter of screen; verify delivery requirements",
        true,
    ),
    (
        "wcag_area_sr",
        0.006,
        "WCAG general flash and red flash thresholds: combined concurrent area <= 0.006 steradians within any 10-degree visual field",
        false,
    ),
    (
        "wcag_field_degrees",
        10.0,
        "WCAG flash thresholds: any 10-degree visual field; implementation uses conservative circumscribing square",
        false,
    ),
    (
        "pattern_static_area_fraction",
        0.4,
        "ITU-R BT.1702-3 Annex 1 Attachment 1 (informative): stationary pattern area > 40%; verify delivery requirements",
        true,
    ),
    (
        "pattern_changing_area_fraction",
        0.25,
        "ITU-R BT.1702-3 Annex 1 Attachment 1 (informative): changing pattern area > 25%; verify delivery requirements",
        true,
    ),
    (
        "pattern_static_pairs",
        5.0,
        "ITU-R BT.1702-3 Annex 1 Attachment 1: > five stationary light/dark pairs; smooth one-direction flow exempt (not inferred here)",
        true,
    ),
    (
        "pattern_changing_pairs",
        5.0,
        "ITU-R BT.1702-3 Annex 1 Attachment 1: > five pairs when changing direction, oscillating, flashing or reversing contrast",
        true,
    ),
    (
        "periodicity_correlation",
        0.7,
        "DETECTOR HEURISTIC: normalized autocorrelation at a nonzero period; not a standards constant",
        true,
    ),
    (
        "warn_area_fraction",
        0.8,
        "PRE-CHECK POLICY: warning at 80% of the area limit when rate exceeds three; not a compliance threshold",
        true,
    ),
    (
        "warn_flashes",
        3.0,
        "PRE-CHECK POLICY: three flashes/s warns; exactly two passes as required by the synthetic acceptance fixture",
        true,
    ),
];

/// Read a named value. Only internal literal names are used.
pub(crate) fn value(name: &str) -> f64 {
    THRESHOLDS.iter().find(|t| t.0 == name).map_or(0.0, |t| t.1)
}

/// Serializable copy, embedded in both report formats for human review.
pub fn table() -> Vec<Threshold> {
    THRESHOLDS
        .iter()
        .map(|&(name, value, citation, verify)| Threshold {
            name: name.into(),
            value,
            citation: citation.into(),
            verify,
        })
        .collect()
}
