//! Accessibility success criteria and candidate information-loss policy.
pub use saccade_core::safety::thresholds::Threshold;
const THRESHOLDS: &[(&str, f64, &str, bool)] = &[
    (
        "cvd_distinct_delta_e",
        5.0,
        "HEURISTIC: original CIEDE2000 >= 5 is distinct; not a universal perceptual threshold",
        true,
    ),
    (
        "cvd_jnd_delta_e",
        2.0,
        "HEURISTIC: simulated CIEDE2000 < 2 is a candidate information loss; viewing/task dependent",
        true,
    ),
    (
        "contrast_aa_text",
        4.5,
        "WCAG 2.x SC 1.4.3: normal text AA >= 4.5:1",
        false,
    ),
    (
        "contrast_aa_large",
        3.0,
        "WCAG SC 1.4.3: large text AA >= 3:1 (18pt, or 14pt bold; declared by user)",
        false,
    ),
    (
        "contrast_aaa_text",
        7.0,
        "WCAG SC 1.4.6: normal text AAA >= 7:1",
        false,
    ),
    (
        "contrast_aaa_large",
        4.5,
        "WCAG SC 1.4.6: large text AAA >= 4.5:1",
        false,
    ),
    (
        "contrast_ui",
        3.0,
        "WCAG 2.1/2.2 SC 1.4.11: essential UI/graphical boundaries >= 3:1; applicability requires human judgement",
        false,
    ),
];
pub(crate) fn value(name: &str) -> f64 {
    THRESHOLDS.iter().find(|t| t.0 == name).map_or(0., |t| t.1)
}
pub(crate) fn table() -> Vec<Threshold> {
    let mut table = saccade_core::safety::thresholds::table();
    table.extend(
        THRESHOLDS
            .iter()
            .map(|&(name, value, citation, verify)| Threshold {
                name: name.into(),
                value,
                citation: citation.into(),
                verify,
            }),
    );
    table
}
