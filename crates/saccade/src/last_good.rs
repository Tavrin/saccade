//! Resolve passed history runs without granting human approval authority.
use crate::agent::CliError;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{io::Read, path::{Path, PathBuf}};
fn read(path:&Path)->Result<Vec<u8>,CliError>{
    let file=std::fs::File::open(path).map_err(|_|CliError::io("last-good input unavailable"))?; let mut data=Vec::new();
    file.take(64*1024*1024+1).read_to_end(&mut data).map_err(|_|CliError::io("last-good input unreadable"))?;
    if data.len()>64*1024*1024{return Err(CliError::new("input_budget","last-good input exceeds 64 MiB"));} Ok(data)
}
pub(crate) fn record_origin(store:&Path, hash:&str, report:&Path)->Result<(),CliError>{
    let origin=store.join("objects").join(format!("{hash}.origin.json"));
    if origin.exists(){return Ok(());}
    let report=report.canonicalize().map_err(|_|CliError::io("history source unavailable"))?;
    let mut file=tempfile::NamedTempFile::new_in(store.join("objects")).map_err(|_|CliError::io("history origin unavailable"))?;
    serde_json::to_writer(&mut file,&json!({"report":report}))?;
    match file.persist_noclobber(origin){Ok(_)=>Ok(()),Err(e) if e.error.kind()==std::io::ErrorKind::AlreadyExists=>Ok(()),Err(_)=>Err(CliError::io("history origin cannot be written"))}
}
pub(crate) fn resolve(store:&Path)->Result<tempfile::TempDir,CliError>{
    let text=String::from_utf8(read(&store.join("index.jsonl"))?).map_err(|_|CliError::io("history index not UTF-8"))?;
    let mut rows=Vec::new();
    for line in text.lines(){let row:Value=serde_json::from_str(line)?;if row["schema"]!="saccade-history.v1"{return Err(CliError::new("version_skew","unsupported history schema"));} rows.push(row);}
    rows.sort_by_key(|r|(r["generated_at_unix"].as_u64().unwrap_or(0),r["report_sha256"].as_str().unwrap_or("").to_owned()));
    for row in rows.into_iter().rev(){
        let hash=row["report_sha256"].as_str().ok_or_else(||CliError::io("history hash missing"))?;
        if hash.len()!=64 || !hash.bytes().all(|b|b.is_ascii_hexdigit() && !b.is_ascii_uppercase()){return Err(CliError::io("history hash invalid"));}
        let data=read(&store.join("objects").join(format!("{hash}.json")))?;
        if format!("{:x}",Sha256::digest(&data))!=hash{return Err(CliError::io("history content hash mismatch"));}
        let report:saccade_core::Report=crate::parse_contract(&data,"saccade-report.v1")?;
        if report.entries.is_empty() || report.is_regression() || report.entries.iter().any(|e|e.status!=saccade_core::report::Status::Pass || e.capture_validity.status==saccade_core::meta::Validity::Invalid){continue;}
        let origin=store.join("objects").join(format!("{hash}.origin.json"));
        let base=if origin.exists(){let value:Value=serde_json::from_slice(&read(&origin)?)?;let source=PathBuf::from(value["report"].as_str().ok_or_else(||CliError::io("history origin missing"))?);source.parent().ok_or_else(||CliError::io("history origin invalid"))?.to_path_buf()}else{PathBuf::new()};
        let capture=PathBuf::from(report.capture_dir.as_deref().ok_or_else(||CliError::new("last_good_unavailable","history run has no capture directory"))?);
        if capture.is_relative() && base.as_os_str().is_empty(){return Err(CliError::new("last_good_unavailable","legacy relative history has no source origin; re-record the report with this build"));}
        let root=if capture.is_absolute(){capture}else{base.join(capture)};
        let temp=tempfile::tempdir().map_err(|_|CliError::io("cannot create last-good snapshot"))?;
        for entry in report.entries{
            let name=Path::new(&entry.name);
            if name.is_absolute() || name.components().any(|c|!matches!(c,std::path::Component::Normal(_))){return Err(CliError::new("unsafe_path","last-good entry path escapes root"));}
            let data=read(&root.join(name))?;
            if entry.capture_sha256.as_deref()!=Some(format!("{:x}",Sha256::digest(&data)).as_str()){return Err(CliError::new("last_good_unavailable","last-good capture no longer matches accepted content"));}
            let target=temp.path().join(name);if let Some(parent)=target.parent(){std::fs::create_dir_all(parent).map_err(|_|CliError::io("cannot stage last-good"))?;}std::fs::write(target,data).map_err(|_|CliError::io("cannot stage last-good"))?;
        }
        return Ok(temp);
    }
    Err(CliError::new("last_good_unavailable","history contains no complete passing run"))
}
