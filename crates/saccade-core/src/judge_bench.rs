//! Model benchmark on the same human-labelled evidence, in both image orders.
use crate::judge::{self, JudgeQuestion, Order, Provider};
use crate::judge_provider::{AskRequest, Backend};
use crate::judge_stats::{Scored, ece};
use crate::labels::Labels;
use crate::review::Profile;
use crate::{Error, Result};
use serde_json::{Value, json};
use std::path::Path;

/// Benchmark schema.
pub const SCHEMA: &str = "saccade-judge-bench.v1";

/// Sort measured models by accuracy, then latency. Incomplete coverage cannot
/// outrank a model that answered the entire evaluation set.
pub fn chain_order(rows: &[Value]) -> Vec<String> {
    let mut ranked: Vec<_> = rows
        .iter()
        .filter(|r| {
            r["provider"] == "gemini"
                && r["accuracy"].as_f64().is_some()
                && r["coverage"].as_f64() == Some(1.0)
        })
        .collect();
    ranked.sort_by(|a, b| {
        b["accuracy"]
            .as_f64()
            .unwrap_or(0.0)
            .total_cmp(&a["accuracy"].as_f64().unwrap_or(0.0))
            .then(
                a["latency_ms"]
                    .as_u64()
                    .unwrap_or(u64::MAX)
                    .cmp(&b["latency_ms"].as_u64().unwrap_or(u64::MAX)),
            )
            .then(a["model"].as_str().cmp(&b["model"].as_str()))
    });
    ranked
        .iter()
        .filter_map(|r| r["model"].as_str().map(str::to_owned))
        .collect()
}

/// Benchmark all selected labels across each pinned Gemini model and Jev.
/// Batch eight labelled items per request; retry and HTTP caps belong to the transport.
pub fn run(
    document: &Path,
    labels: &Labels,
    models: &[String],
    questions: &[String],
    backend: &dyn Backend,
    budget: usize,
) -> Result<Value> {
    for q in questions {
        if JudgeQuestion::parse(q).is_none() {
            return Err(Error::Config(format!("unknown benchmark question {q}")));
        }
    }
    let selected: Vec<_> = labels
        .items
        .iter()
        .filter(|l| questions.is_empty() || questions.contains(&l.question))
        .collect();
    if selected.is_empty() {
        return Err(Error::Config(
            "no labelled items match the selected questions".into(),
        ));
    }
    let items: Vec<_> = selected
        .iter()
        .map(|l| crate::labels::replay(l, document))
        .collect::<Result<_>>()?;
    // Validate every crop before spending even the first Jev call.
    if !models.is_empty()
        && items
            .iter()
            .any(|i| judge::strips_for(i, Order::Ab).is_empty())
    {
        return Err(Error::Config(
            "benchmark vision items need verified image pairs and crops".into(),
        ));
    }
    let mut rows = Vec::new();
    let mut used = 0;
    for (provider, model) in std::iter::once((Provider::Jev, "jev-latest".to_owned()))
        .chain(models.iter().cloned().map(|m| (Provider::Gemini, m)))
    {
        let profile = Profile {
            gemini_models: vec![model.clone()],
            ..Profile::default()
        };
        let spec = profile.spec(provider)?;
        let mut predictions = Vec::new();
        let mut scores = Vec::new();
        let mut latency = 0;
        let mut correct = 0;
        let mut answered = 0;
        let mut first_choices = 0;
        let mut pair_observations = 0;
        let mut flips = 0;
        let mut order_answers: std::collections::BTreeMap<String, String> =
            std::collections::BTreeMap::new();
        for order in [Order::Ab, Order::Ba] {
            for start in (0..items.len()).step_by(8) {
                if used >= budget {
                    continue;
                }
                let end = (start + 8).min(items.len());
                let prepared: Vec<_> = items[start..end]
                    .iter()
                    .map(|item| {
                        // Preference labels have no reference identity; Jev sees the encoded state.
                        let images = if provider == Provider::Gemini {
                            judge::strips_for(item, order)
                        } else {
                            Vec::new()
                        };
                        let wire = item.question.wire_answers();
                        let mut presented = item.clone();
                        if provider == Provider::Gemini
                            && item.question == JudgeQuestion::Preference
                        {
                            presented.states = [json!({}), json!({})];
                        }
                        let (prompt, state) =
                            judge::build_prompt(&spec, &presented, order, &wire, images.len());
                        (wire, prompt, state, images)
                    })
                    .collect();
                if provider == Provider::Gemini && prepared.iter().any(|p| p.3.is_empty()) {
                    return Err(Error::Config(
                        "benchmark vision items need verified image pairs and crops".into(),
                    ));
                }
                let requests: Vec<_> = items[start..end]
                    .iter()
                    .zip(&prepared)
                    .map(|(i, (wire, prompt, state, images))| AskRequest {
                        spec: &spec,
                        question: i.question.as_str(),
                        question_text: i.question.text(),
                        kind: i.question.kind().as_str(),
                        wire_answers: wire,
                        state,
                        prompt,
                        images,
                    })
                    .collect();
                let started = std::time::Instant::now();
                let result = backend.ask_many(&requests);
                let elapsed_ms = started.elapsed().as_millis() as u64;
                used += 1;
                if let Some(c) = backend.http_counts() {
                    used = c.iter().sum();
                }
                latency += result
                    .iter()
                    .filter_map(|r| r.result.as_ref().ok().map(|r| r.latency_ms))
                    .max()
                    .unwrap_or(0)
                    .max(elapsed_ms);
                for (offset, i) in items[start..end].iter().enumerate() {
                    let truth = &selected[start + offset].answer;
                    let Some(out) = result.get(offset) else {
                        continue;
                    };
                    let answer = out.result.as_ref().ok();
                    let stable = answer.map(|r| {
                        if i.question == JudgeQuestion::Preference {
                            match (r.answer.as_str(), order) {
                                ("P1", Order::Ab) | ("P2", Order::Ba) => "a",
                                ("P2", Order::Ab) | ("P1", Order::Ba) => "b",
                                _ => r.answer.as_str(),
                            }
                        } else {
                            r.answer.as_str()
                        }
                    });
                    let valid = stable.is_some_and(|s| !crate::judge_stats::is_abstain(s));
                    let is_correct = valid && stable == Some(truth.as_str());
                    correct += usize::from(is_correct);
                    answered += usize::from(valid);
                    if let Some(r) = answer {
                        scores.push(Scored {
                            conf: r.prob.unwrap_or(0.0),
                            correct: is_correct,
                        });
                        if i.question == JudgeQuestion::Preference {
                            first_choices += usize::from(r.answer == "P1");
                            pair_observations += 1;
                            if order == Order::Ba
                                && order_answers
                                    .get(&i.id)
                                    .is_some_and(|a| Some(a.as_str()) != stable)
                            {
                                flips += 1;
                            }
                            if order == Order::Ab {
                                order_answers.insert(i.id.clone(), stable.unwrap_or("").to_owned());
                            }
                        }
                    }
                    predictions.push(json!({"entry":i.entry,"question":i.question.as_str(),"evidence_hash":selected[start+offset].evidence_hash,
                        "order":if order==Order::Ab {"ab"}else{"ba"}, "answer":stable, "truth":truth,"correct":is_correct,
                        "prob":answer.and_then(|r|r.prob),"confidence":answer.and_then(|r|r.confidence),
                        "answered_model":answer.map(|r|&r.model),"latency_ms":answer.map(|r|r.latency_ms),
                        "usage":answer.map(|r|&r.usage),"error":out.result.as_ref().err(),"attempts":out.attempts}));
                }
            }
        }
        let expected = selected.len() * 2;
        rows.push(json!({"provider":provider.as_str(),"model":model,"items":selected.len(),"predictions":predictions,
            "accuracy":(answered>0).then(||correct as f64/answered as f64),"coverage":answered as f64/expected as f64,
            "ece":ece(&scores),"latency_ms":latency,"estimated_cost_usd":null,
            "position_bias":{"p1_share":(pair_observations>0).then(||first_choices as f64/pair_observations as f64),
                "order_flips":flips,"paired_items":selected.iter().filter(|l|l.question=="preference").count()}}));
    }
    let suggested = chain_order(&rows);
    Ok(
        json!({"schema":SCHEMA,"labels":crate::paths::portable(document),"items":selected.len(),
        "models":rows,"suggested_chain":suggested,"calls_used":used,"http_attempts":backend.http_counts(),
        "budget_calls":budget,"estimated_cost_usd":null,
        "limits":"A small public set is a smoke test, not evidence of production accuracy. Accuracy is conditional on committed answers; incomplete models are excluded from chain suggestions."}),
    )
}

/// Canonical manifest evaluator with resumable isolated budgets.
pub mod evaluator;
