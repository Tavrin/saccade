//! Generic summary notifications; credentials and endpoint stay in user configuration.
use crate::{agent::CliError, product_io as io};
use clap::{Args,ValueEnum};
use serde_json::{Value,json};
use std::path::{Path,PathBuf};
pub(crate) const SUMMARY_SCHEMA:&str="saccade-notification.v1";
pub(crate) const RESULT_SCHEMA:&str="saccade-notify-result.v1";
#[derive(Clone,Copy,ValueEnum)]pub(crate) enum Template{Generic,Slack,Teams}
#[derive(Args)]pub(crate) struct NotifyArgs{
    /// Full report, sweep report or design report, read locally.
    report:PathBuf,
    #[arg(long,value_enum,default_value="generic")]template:Template,
    /// Display link; defaults to the report path. Never used as the webhook endpoint.
    #[arg(long)]report_link:Option<String>,
    #[arg(long)]json:bool,
}
pub(crate) fn summary(report:&Value,path:&Path,link:Option<&str>)->Result<Value,CliError>{
    let schema=report["schema"].as_str().ok_or_else(||CliError::usage("notification report needs a schema"))?;
    if !matches!(schema,"saccade-report.v1"|"saccade-result.v2"|"saccade-sweep-report.v1"|"saccade-design-report.v1"){return Err(CliError::new("version_skew","unsupported notification report"));}
    let mut counts=report["totals"].clone();if counts.is_null(){counts=report["counts"].clone();}
    if counts.is_null(){let entries=report["frames"].as_array().cloned().unwrap_or_default();let failures=report["capture_failures"].as_array().map_or(0,Vec::len);counts=json!({"total":entries.len(),"pass":entries.iter().filter(|e|e["status"]=="pass").count(),"fail":entries.iter().filter(|e|e["status"]=="fail").count(),"capture_failures":failures});}
    if let Some(failures)=report["capture_failures"].as_array(){counts["capture_failures"]=json!(failures.len());}
    let mut entries=report["entries"].as_array().cloned().unwrap_or_default();
    entries.retain(|e|e["status"]!="pass");entries.sort_by(|a,b|b["value"].as_f64().unwrap_or(-1.0).total_cmp(&a["value"].as_f64().unwrap_or(-1.0)));
    let worst=entries.iter().take(5).map(|e|json!({"name":e["name"],"status":e["status"],"value":e["value"],"threshold":e["threshold"]})).collect::<Vec<_>>();
    let worst=if worst.is_empty(){report["failing"].as_array().map(|a|a.iter().take(5).map(|e|json!({"name":e["entry"],"value":e["value"],"threshold":e["threshold"]})).collect()).unwrap_or(worst)}else{worst};
    let report_link=link.map(str::to_owned).unwrap_or_else(||path.to_string_lossy().into_owned());
    if report_link.len()>2048 || report_link.chars().any(char::is_control){return Err(CliError::usage("notification report link is too long or contains controls"));}
    Ok(json!({"schema":SUMMARY_SCHEMA,"counts":counts,"worst":worst,"report":report_link,"verdict":report["verdict"].as_str().unwrap_or("measured_report")}))
}
pub(crate) fn payload(summary:&Value,template:Template)->Value{
    let text=format!("Saccade {}: {}. Report: {}",summary["verdict"].as_str().unwrap_or("summary"),summary["counts"],summary["report"].as_str().unwrap_or("unavailable"));
    match template{Template::Generic=>summary.clone(),Template::Slack=>json!({"text":text,"blocks":[{"type":"section","text":{"type":"plain_text","text":text}}]}),Template::Teams=>json!({"type":"message","attachments":[{"contentType":"application/vnd.microsoft.card.adaptive","content":{"type":"AdaptiveCard","version":"1.2","body":[{"type":"TextBlock","text":text,"wrap":true}]}}]})}
}
pub(crate) fn send(summary:&Value,template:Template,url:&str,token:Option<&str>)->Result<Value,CliError>{
    let body=serde_json::to_vec(&payload(summary,template))?;if body.len()>16384{return Err(CliError::new("output_budget","notification exceeds 16 KiB"));}
    let auth=token.map(|t|format!("Bearer {t}"));let mut headers=vec![("Content-Type","application/json")];if let Some(auth)=&auth{headers.push(("Authorization",auth));}
    let response=io::request("POST",url,&headers,Some(&body))?;
    if !(200..300).contains(&response.status){return Err(CliError::new("webhook_status",format!("webhook HTTP {} (endpoint and credentials redacted)",response.status)));}
    Ok(json!({"schema":RESULT_SCHEMA,"delivered":true,"status":response.status,"summary":summary}))
}
pub(crate) fn run(args:NotifyArgs)->Result<u8,CliError>{
    let report:Value=io::json(&args.report)?;let summary=summary(&report,&args.report,args.report_link.as_deref())?;let values=io::provider_env("webhook")?;
    let url=values.get("WEBHOOK_URL").filter(|s|!s.is_empty()).ok_or_else(||CliError::new("credentials","WEBHOOK_URL missing in user webhook.env"))?;
    if io::url(url)?.scheme()!="https"{return Err(CliError::usage("configured webhook endpoint must be HTTPS"));}
    let result=send(&summary,args.template,url,values.get("WEBHOOK_TOKEN").map(String::as_str))?;io::emit(&result,args.json,"notification delivered")?;Ok(0)
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn templates_keep_plain_data_and_do_not_include_credentials(){let value=json!({"schema":SUMMARY_SCHEMA,"counts":{"fail":2},"worst":[],"report":"https://example.com/report","verdict":"regression"});assert_eq!(payload(&value,Template::Generic)["schema"],SUMMARY_SCHEMA);assert_eq!(payload(&value,Template::Slack)["blocks"][0]["text"]["type"],"plain_text");assert_eq!(payload(&value,Template::Teams)["attachments"][0]["contentType"],"application/vnd.microsoft.card.adaptive");}
    #[test]#[ignore="heavy: notifier-http"]fn posts_generic_summary_to_local_fixture(){use std::io::{Read,Write};let listener=std::net::TcpListener::bind("127.0.0.1:0").expect("bind");let url=format!("http://{}",listener.local_addr().expect("address"));let server=std::thread::spawn(move||{let(mut stream,_)=listener.accept().expect("accept");stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).expect("timeout");let mut bytes=Vec::new();let mut buffer=[0u8;4096];loop{let n=stream.read(&mut buffer).expect("read");bytes.extend_from_slice(&buffer[..n]);if let Some(end)=bytes.windows(4).position(|w|w==b"\r\n\r\n"){let headers=String::from_utf8_lossy(&bytes[..end]);let len=headers.lines().find_map(|l|l.to_ascii_lowercase().strip_prefix("content-length:").and_then(|v|v.trim().parse::<usize>().ok())).expect("length");if bytes.len()>=end+4+len{let value:Value=serde_json::from_slice(&bytes[end+4..end+4+len]).expect("body");assert_eq!(value["schema"],SUMMARY_SCHEMA);break;}}}stream.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").expect("response");});let summary=json!({"schema":SUMMARY_SCHEMA,"counts":{"pass":1},"worst":[],"report":"report/index.html","verdict":"pass"});let result=send(&summary,Template::Generic,&url,Some("fixture-token")).expect("send");assert_eq!(result["delivered"],true);assert!(!result.to_string().contains("fixture-token"));server.join().expect("server");}
}
