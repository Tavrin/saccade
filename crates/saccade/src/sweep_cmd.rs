//! Deterministic, driver-neutral page sweeps.
use crate::{agent::CliError, product_io as io};
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{collections::{BTreeMap, BTreeSet}, path::{Path, PathBuf}};

pub(crate) const MANIFEST_SCHEMA: &str = "saccade-sweep.v1";
pub(crate) const RECEIPT_SCHEMA: &str = "saccade-sweep-captures.v1";
pub(crate) const REPORT_SCHEMA: &str = "saccade-sweep-report.v1";
#[derive(Args)]
pub(crate) struct SweepArgs { #[command(subcommand)] operation: Operation }
#[derive(Subcommand)]
enum Operation {
    /// Sample a URL list or bounded sitemap tree into a driver-neutral manifest.
    Plan {
        #[arg(long, conflicts_with = "sitemap", required_unless_present = "sitemap")] urls: Option<PathBuf>,
        #[arg(long)] sitemap: Option<String>,
        #[arg(long)] before_origin: String,
        #[arg(long)] after_origin: String,
        #[arg(long, default_value_t = 3, value_parser = clap::value_parser!(u16).range(1..=100))] samples: u16,
        #[arg(long, default_value_t = 42)] seed: u64,
        /// Repeat WIDTHxHEIGHT. Default: 1280x720.
        #[arg(long)] viewport: Vec<String>,
        #[arg(long)] out: PathBuf,
        #[arg(long)] json: bool,
    },
    /// Compare every planned capture, including explicit failure receipts.
    Compare {
        manifest: PathBuf,
        #[arg(long)] captures: PathBuf,
        #[arg(long)] out: PathBuf,
        #[arg(long)] config: Option<PathBuf>,
        #[arg(long, value_parser = ["last-good"])] baseline: Option<String>,
        #[arg(long, requires = "baseline")] history_store: Option<PathBuf>,
        #[arg(long)] json: bool,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Manifest { pub(crate) schema: String, pub(crate) seed: u64, pub(crate) pages: Vec<Page> }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Page { pub(crate) id: String, pub(crate) group: String, pub(crate) before: String, pub(crate) after: String, pub(crate) viewport: [u32; 2] }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Captures { pub(crate) schema: String, pub(crate) manifest_sha256: String, pub(crate) receipts: Vec<Receipt> }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Receipt {
    pub(crate) id: String, pub(crate) side: String, pub(crate) status: String,
    pub(crate) path: Option<PathBuf>, pub(crate) sha256: Option<String>, pub(crate) final_url: Option<String>, pub(crate) timing_ms: u64,
    pub(crate) error: Option<String>, #[serde(default)] pub(crate) css_tokens: Vec<serde_json::Value>,
}
pub(crate) fn pattern(url: &url::Url) -> String {
    url.path().split('/').map(|part| if !part.is_empty() && (part.bytes().all(|b| b.is_ascii_digit()) || (part.len() >= 8 && part.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-'))) { ":id" } else { part }).collect::<Vec<_>>().join("/")
}
fn rewrite(text: &str, origin: &str) -> Result<String, CliError> {
    let mut source = io::url(text)?; let origin = io::url(origin)?;
    if origin.path() != "/" || origin.query().is_some() { return Err(CliError::usage("origins must have no path or query")); }
    source.set_scheme(origin.scheme()).map_err(|_| CliError::usage("invalid origin scheme"))?;
    source.set_host(origin.host_str()).map_err(|_| CliError::usage("invalid origin host"))?;
    source.set_port(origin.port()).map_err(|_| CliError::usage("invalid origin port"))?;
    Ok(source.to_string())
}
fn sitemap(source: &str, visited: &mut BTreeSet<String>, depth: usize, budget: &mut usize, urls: &mut Vec<String>) -> Result<(), CliError> {
    if depth > 4 || visited.len() >= 128 { return Err(CliError::new("input_budget", "sitemap depth/document bound exceeded")); }
    if !visited.insert(source.to_owned()) { return Err(CliError::new("config", "sitemap index cycle or repeated document")); }
    let bytes = if source.starts_with("https://") || source.starts_with("http://") { io::get(source, &[])?.bytes } else { io::read(Path::new(source))? };
    *budget = budget.saturating_add(bytes.len());
    if *budget > 32 * 1024 * 1024 { return Err(CliError::new("input_budget", "sitemap tree exceeds 32 MiB")); }
    let text = std::str::from_utf8(&bytes).map_err(|_| CliError::new("config", "sitemap must be UTF-8 XML"))?;
    let document = roxmltree::Document::parse(text).map_err(|_| CliError::new("config", "invalid sitemap XML (DTD disabled)"))?;
    let root = document.root_element();
    if !matches!(root.tag_name().name(), "sitemapindex" | "urlset") { return Err(CliError::usage("sitemap must be urlset or sitemapindex")); }
    for node in root.children().filter(|n| n.is_element()) {
        let location = node.children().find(|n| n.has_tag_name("loc")).and_then(|n| n.text()).ok_or_else(|| CliError::usage("sitemap entry lacks loc"))?.trim();
        if root.has_tag_name("sitemapindex") {
            let child = if source.starts_with("http") { io::url(source)?.join(location).map_err(|_| CliError::usage("invalid sitemap location"))?.to_string() }
                else if location.starts_with("http") { location.to_owned() }
                else { Path::new(source).parent().unwrap_or(Path::new(".")).join(location).to_string_lossy().into_owned() };
            sitemap(&child, visited, depth + 1, budget, urls)?;
        } else { io::url(location)?; urls.push(location.to_owned()); }
        if urls.len() > 100_000 { return Err(CliError::new("input_budget", "sitemap exceeds 100000 URLs")); }
    }
    Ok(())
}
pub(crate) fn plan(urls: Vec<String>, before: &str, after: &str, samples: usize, seed: u64, viewports: &[[u32; 2]]) -> Result<Manifest, CliError> {
    if urls.is_empty() || urls.len() > 100_000 || samples == 0 || samples > 100 || viewports.is_empty() || viewports.len() > 16 || viewports.iter().any(|v| v.iter().any(|n| *n < 8 || *n > 16384)) { return Err(CliError::usage("invalid sweep size, sample count or viewports")); }
    let mut groups: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for text in urls { let u = io::url(&text)?; groups.entry(pattern(&u)).or_default().insert(u.to_string()); }
    let mut pages = Vec::new();
    for (group, candidates) in groups {
        let mut ranked: Vec<_> = candidates.into_iter().map(|u| { let rank = format!("{:x}", Sha256::digest(format!("{seed}:{u}").as_bytes())); (rank, u) }).collect(); ranked.sort();
        for (_, u) in ranked.into_iter().take(samples) {
            for viewport in viewports {
                let before = rewrite(&u, before)?; let after = rewrite(&u, after)?;
                let id = format!("{:x}", Sha256::digest(format!("{u}:{}x{}", viewport[0], viewport[1]).as_bytes()));
                pages.push(Page { id, group: group.clone(), before, after, viewport: *viewport });
            }
        }
    }
    if pages.len() > 100_000 { return Err(CliError::new("input_budget", "planned captures exceed 100000 pairs")); }
    Ok(Manifest { schema: MANIFEST_SCHEMA.into(), seed, pages })
}
pub(crate) fn validate(manifest: &Manifest) -> Result<(), CliError> {
    if manifest.schema != MANIFEST_SCHEMA { return Err(CliError::new("version_skew", "unsupported sweep schema")); }
    if manifest.pages.is_empty() || manifest.pages.len() > 100_000 { return Err(CliError::usage("sweep requires 1..100000 pairs")); }
    let mut ids = BTreeSet::new();
    for page in &manifest.pages {
        if page.id.len() != 64 || !page.id.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()) || !ids.insert(&page.id) || page.viewport.iter().any(|n| *n < 8 || *n > 16384) { return Err(CliError::usage("invalid/duplicate page identity or viewport")); }
        io::url(&page.before)?; io::url(&page.after)?;
    }
    Ok(())
}
pub(crate) fn capture_file(root: &Path, receipt: &Receipt) -> Result<Vec<u8>, CliError> {
    let relative = receipt.path.as_ref().ok_or_else(|| CliError::new("capture_failure", "capture receipt lacks path"))?;
    if relative.is_absolute() || relative.components().any(|c| !matches!(c, std::path::Component::Normal(_))) { return Err(CliError::new("unsafe_path", "capture path must be relative")); }
    let root = root.canonicalize().map_err(|_| CliError::io("capture root unavailable"))?;
    let path = root.join(relative).canonicalize().map_err(|_| CliError::new("capture_failure", "capture file missing"))?;
    if !path.starts_with(root) { return Err(CliError::new("unsafe_path", "capture path escapes root")); }
    let bytes = io::read(&path)?;
    if receipt.sha256.as_deref() != Some(format!("{:x}", Sha256::digest(&bytes)).as_str()) { return Err(CliError::new("capture_failure", "capture hash differs from receipt")); }
    Ok(bytes)
}
pub(crate) fn compare(manifest_path: &Path, captures_path: &Path, out: &Path, cfg: &saccade_core::config::RunConfig, last_good: Option<&Path>) -> Result<serde_json::Value, CliError> {
    let manifest: Manifest = io::json(manifest_path)?; validate(&manifest)?;
    let captures: Captures = io::json(captures_path)?;
    if captures.schema != RECEIPT_SCHEMA || captures.manifest_sha256 != format!("{:x}", Sha256::digest(io::read(manifest_path)?)) { return Err(CliError::new("capture_failure", "capture manifest identity mismatch")); }
    let mut receipts = BTreeMap::new();
    for receipt in &captures.receipts {
        if !manifest.pages.iter().any(|p| p.id == receipt.id) || !matches!(receipt.side.as_str(), "before" | "after") || receipts.insert((receipt.id.as_str(), receipt.side.as_str()), receipt).is_some() { return Err(CliError::new("capture_failure", "extra/duplicate capture receipt")); }
    }
    let temp = tempfile::tempdir().map_err(|_| CliError::io("cannot create sweep pair workspace"))?;
    let baseline = temp.path().join("before"); let candidate = temp.path().join("after");
    std::fs::create_dir(&baseline).map_err(|_| CliError::io("cannot create baseline"))?;
    std::fs::create_dir(&candidate).map_err(|_| CliError::io("cannot create candidate"))?;
    let root = captures_path.parent().unwrap_or(Path::new(".")); let mut failures = Vec::new();
    for page in &manifest.pages {
        for (side, dir) in [("before", &baseline), ("after", &candidate)] {
            let filename = format!("{}.png", page.id);
            let data = if side == "before" && last_good.is_some() { io::read(&last_good.unwrap_or(Path::new(".")).join(&filename)) }
                else { receipts.get(&(page.id.as_str(), side)).filter(|r| r.status == "captured" && r.error.is_none()).ok_or_else(|| CliError::new("capture_failure", "missing or failed capture receipt")).and_then(|r| capture_file(root, r)) };
            match data {
                Ok(bytes) => { std::fs::write(dir.join(filename), bytes).map_err(|_| CliError::io("cannot stage sweep input"))?; }
                Err(error) => failures.push(json!({"id":page.id,"group":page.group,"viewport":page.viewport,"side":side,"code":error.code,"error":error.message})),
            }
        }
    }
    let mut config = cfg.clone(); config.fail_on_new = true; config.allow_empty = false;
    let report = saccade_core::run::run(&baseline, &candidate, &out.join("comparison"), &config)?;
    let mut groups: BTreeMap<String, Vec<serde_json::Value>> = BTreeMap::new();
    for page in &manifest.pages {
        let entry = report.entries.iter().find(|e| e.name == format!("{}.png", page.id));
        groups.entry(format!("{}@{}x{}", page.group, page.viewport[0], page.viewport[1])).or_default().push(json!({"id":page.id,"status":entry.map_or("error".into(), |e|serde_json::to_value(e.status).unwrap_or(json!("error"))),"value":entry.and_then(|e|e.value)}));
    }
    let value = json!({"schema":REPORT_SCHEMA,"verdict":if failures.is_empty() && !report.is_regression(){"pass"}else{"regression"},"groups":groups,"capture_failures":failures,"totals":report.totals,"report":"comparison/index.html"});
    io::write(&out.join("saccade-sweep-report.v1.json"), &value)?; Ok(value)
}
pub(crate) fn run(args: SweepArgs) -> Result<u8, CliError> {
    match args.operation {
        Operation::Plan { urls, sitemap: source, before_origin, after_origin, samples, seed, viewport, out, json } => {
            let urls = if let Some(path) = urls { String::from_utf8(io::read(&path)?).map_err(|_|CliError::usage("URL list must be UTF-8"))?.lines().map(str::trim).filter(|s| !s.is_empty() && !s.starts_with('#')).map(str::to_owned).collect() }
                else { let mut urls=Vec::new(); sitemap(source.as_deref().ok_or_else(||CliError::usage("missing sitemap"))?, &mut BTreeSet::new(), 0, &mut 0, &mut urls)?; urls };
            let viewports = if viewport.is_empty() { vec![[1280,720]] } else { viewport.iter().map(|v| { let (w,h)=v.split_once('x').ok_or_else(||CliError::usage("viewport must be WIDTHxHEIGHT"))?; Ok([w.parse().map_err(|_|CliError::usage("invalid viewport"))?, h.parse().map_err(|_|CliError::usage("invalid viewport"))?]) }).collect::<Result<Vec<_>,CliError>>()? };
            let manifest=plan(urls,&before_origin,&after_origin,samples as usize,seed,&viewports)?; io::write(&out,&manifest)?; io::emit(&manifest,json,"sweep manifest written")?; Ok(0)
        }
        Operation::Compare { manifest,captures,out,config,baseline,history_store,json } => {
            let last = if baseline.is_some() { Some(crate::last_good::resolve(history_store.as_deref().ok_or_else(||CliError::usage("last-good requires --history-store"))?)?) } else {None};
            let value=compare(&manifest,&captures,&out,&crate::load_config(config.as_deref())?,last.as_ref().map(|t|t.path()))?; io::emit(&value,json,"sweep report written")?; Ok(u8::from(value["verdict"]!="pass"))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn sampling_is_seeded_and_input_order_independent() {
        let urls=(0..20).map(|i|format!("https://example.com/items/{i}")).collect::<Vec<_>>();
        let a=plan(urls.clone(),"https://example.org","https://example.com",3,42,&[[320,240]]).expect("plan");
        let b=plan(urls.into_iter().rev().collect(),"https://example.org","https://example.com",3,42,&[[320,240]]).expect("plan");
        assert_eq!(serde_json::to_value(&a).expect("json"),serde_json::to_value(b).expect("json")); assert_eq!(a.pages.len(),3); assert!(a.pages[0].before.starts_with("https://example.org/items/")); assert_eq!(a.pages[0].group,"/items/:id");
    }
    #[test] fn sitemap_index_and_cycle_are_bounded() {
        let dir=tempfile::tempdir().expect("temp"); let index=dir.path().join("index.xml"); let child=dir.path().join("child.xml");
        std::fs::write(&index,"<sitemapindex><sitemap><loc>child.xml</loc></sitemap></sitemapindex>").expect("index");
        std::fs::write(&child,"<urlset><url><loc>https://example.com/a</loc></url></urlset>").expect("child"); let mut urls=Vec::new(); sitemap(index.to_str().expect("path"),&mut BTreeSet::new(),0,&mut 0,&mut urls).expect("parse"); assert_eq!(urls.len(),1);
        std::fs::write(&child,"<sitemapindex><sitemap><loc>index.xml</loc></sitemap></sitemapindex>").expect("cycle"); assert!(sitemap(index.to_str().expect("path"),&mut BTreeSet::new(),0,&mut 0,&mut Vec::new()).is_err());
    }
}
