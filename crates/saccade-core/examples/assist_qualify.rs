//! The old paid runner is disabled: local price constants cannot establish an invoice ceiling.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err("paid qualification disabled: verified billing-cap evidence unavailable; use scripts/qualify-wave4.sh --dry-run".into())
}
