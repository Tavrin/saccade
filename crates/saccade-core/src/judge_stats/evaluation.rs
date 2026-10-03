//! Availability and conditional quality have independent denominators.
use super::{Rng, Scored, ece, percentile, reliability};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Mutually exclusive current resolution, separate from the count of attempts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Valid committed closed answer.
    Answered,
    /// Valid explicit model abstention.
    Abstained,
    /// Response violated the contract.
    Invalid,
    /// Failed provider or transport.
    Unavailable,
    /// Pending until retry_at inside the evaluation window.
    Deferred,
    /// Source policy prohibited dispatch.
    Denied,
    /// Shared cap prevented further attempts.
    BudgetBlocked,
}
/// One scheduled question/model/order resolution. Repeated orders share a case ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    /// Independent change case for grouped confidence intervals.
    pub case_id: String,
    /// Fixed question/rubric identity.
    pub question: String,
    /// Actual provider and model, including operational fallback.
    pub provider: String,
    /// Actual model stratum.
    pub model: String,
    /// Actual vision extractor; absent for direct structured evidence.
    pub vision_model: Option<String>,
    /// Whether conclusions rely on model observations.
    pub depends_on_model_observation: bool,
    /// Human reference if eligible; no labels means no quality score.
    pub truth: Option<String>,
    /// Valid answer only; failures never synthesize abstention.
    pub answer: Option<String>,
    /// Calibratable probability of the committed answer.
    pub probability: Option<f64>,
    /// Current resolution.
    pub outcome: Outcome,
    /// Actual HTTP reservations, including retries.
    pub attempts: u64,
    /// Total observed latency when known.
    pub latency_ms: Option<u64>,
    /// Provider usage, unknown when absent.
    pub usage: Option<Value>,
    /// Pinned or provider-reported cost, unknown when absent.
    pub cost: Option<f64>,
    /// Both-order identity, if applicable.
    pub order: Option<String>,
    /// Catalog-defined harmful miss, or deterministic constraint violation.
    pub critical_error: bool,
}
fn ratio(n: usize, d: usize) -> Option<f64> {
    (d > 0).then(|| n as f64 / d as f64)
}
/// Scores one actual model/question stratum; undefined quantities serialize as null.
pub fn metrics(items: &[Observation]) -> Value {
    let count = |o| items.iter().filter(|i| i.outcome == o).count();
    let scheduled = items.len();
    let eligible = scheduled - count(Outcome::Denied);
    let valid = count(Outcome::Answered) + count(Outcome::Abstained);
    let committed: Vec<_> = items
        .iter()
        .filter(|i| i.outcome == Outcome::Answered)
        .collect();
    let labelled: Vec<_> = committed.iter().filter(|i| i.truth.is_some()).collect();
    let correct = labelled.iter().filter(|i| i.answer == i.truth).count();
    let scored: Vec<_> = labelled
        .iter()
        .filter_map(|i| {
            i.probability
                .filter(|p| p.is_finite() && (0.0..=1.0).contains(p))
                .map(|conf| Scored {
                    conf,
                    correct: i.answer == i.truth,
                })
        })
        .collect();
    let mut classes: BTreeMap<String, (usize, usize, usize)> = BTreeMap::new();
    let mut groups: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for i in &labelled {
        let g = groups.entry(&i.case_id).or_default();
        g.0 += usize::from(i.answer == i.truth);
        g.1 += 1;
        if let (Some(answer), Some(truth)) = (&i.answer, &i.truth) {
            classes.entry(answer.clone()).or_default().1 += 1;
            classes.entry(truth.clone()).or_default().2 += 1;
            if answer == truth {
                classes.entry(truth.clone()).or_default().0 += 1;
            }
        }
    }
    let groups: Vec<_> = groups.into_values().collect();
    let mut samples = Vec::new();
    let mut rng = Rng(0x51accade);
    if !groups.is_empty() {
        for _ in 0..500 {
            let (mut c, mut n) = (0, 0);
            for _ in &groups {
                let g = groups[(rng.next() % groups.len() as u64) as usize];
                c += g.0;
                n += g.1;
            }
            samples.push(c as f64 / n as f64);
        }
        samples.sort_by(f64::total_cmp);
    }
    let mut pairs: BTreeMap<&str, Vec<&Observation>> = BTreeMap::new();
    for i in &committed {
        if i.order.is_some() {
            pairs.entry(&i.case_id).or_default().push(i);
        }
    }
    let pairs: Vec<_> = pairs
        .into_values()
        .filter(|p| p.len() == 2 && p[0].order != p[1].order)
        .collect();
    let mut latency: Vec<_> = items
        .iter()
        .filter_map(|i| i.latency_ms.map(|v| v as f64))
        .collect();
    latency.sort_by(f64::total_cmp);
    let attempted = items.iter().filter(|i| i.attempts > 0).count();
    json!({"scheduled":scheduled,"eligible":eligible,"attempted":attempted,"valid":valid,"answered":committed.len(),
        "abstained":count(Outcome::Abstained),"invalid":count(Outcome::Invalid),"unavailable":count(Outcome::Unavailable),
        "deferred":count(Outcome::Deferred),"denied":count(Outcome::Denied),"budget_blocked":count(Outcome::BudgetBlocked),
        "attempts":items.iter().map(|i|i.attempts).sum::<u64>(),
        "conditional_accuracy":ratio(correct,labelled.len()),"labelled_committed":labelled.len(),
        "answer_coverage":ratio(committed.len(),eligible),"availability":ratio(valid,eligible),
        "attempted_completion":ratio(attempted,eligible),"abstention_rate":ratio(count(Outcome::Abstained),valid),
        "abstention_appropriateness":ratio(items.iter().filter(|i|i.outcome==Outcome::Abstained && i.truth.as_deref()==Some("abstain")).count(),items.iter().filter(|i|i.outcome==Outcome::Abstained && i.truth.is_some()).count()),
        "correct_resolutions_over_eligible":ratio(correct,eligible),"critical_errors":items.iter().filter(|i|i.critical_error).count(),
        "classes":classes.into_iter().map(|(class,(tp,predicted,truth))|json!({"class":class,"precision":ratio(tp,predicted),"recall":ratio(tp,truth),"support":truth})).collect::<Vec<_>>(),
        "ece":ece(&scored),"reliability":reliability(&scored).iter().map(|b|json!({"lo":b.lo,"hi":b.hi,"n":b.n,"mean_confidence":b.mean_conf,"accuracy":b.accuracy})).collect::<Vec<_>>(),
        "accuracy_ci_grouped_by_case":if samples.is_empty(){Value::Null}else{json!([percentile(&samples,0.025),percentile(&samples,0.975)])},
        "independent_labelled_cases":groups.len(),"both_order_pairs":pairs.len(),"both_order_disagreement":ratio(pairs.iter().filter(|p|p[0].answer!=p[1].answer).count(),pairs.len()),
        "latency_p50_ms":if latency.is_empty(){Value::Null}else{json!(percentile(&latency,0.5))},"latency_p95_ms":if latency.is_empty(){Value::Null}else{json!(percentile(&latency,0.95))},
        "usage":items.iter().map(|i|&i.usage).collect::<Vec<_>>(),"cost":if items.iter().any(|i|i.cost.is_none()){None}else{Some(items.iter().filter_map(|i|i.cost).sum::<f64>())},
        "qualified":false,"qualification_note":"Qualification requires held-out labels, matching calibration identity, miss bounds and declared tolerance; transport completion alone is insufficient."})
}
/// Never mix pinned-model scores with production-policy replay or observation strata.
pub fn strata(items: &[Observation]) -> Vec<Value> {
    let mut groups: BTreeMap<_, Vec<Observation>> = BTreeMap::new();
    for i in items {
        groups
            .entry((
                i.question.clone(),
                i.provider.clone(),
                i.model.clone(),
                i.vision_model.clone(),
                i.depends_on_model_observation,
            ))
            .or_default()
            .push(i.clone());
    }
    groups.into_iter().map(|((question,provider,model,vision_model,dependent),rows)|json!({"question":question,"provider":provider,"model":model,"vision_model":vision_model,"depends_on_model_observation":dependent,"metrics":metrics(&rows)})).collect()
}
/// Fit empirical calibration on the designated calibration split only. Adjacent
/// probability bins are pooled until their observed accuracies are monotone.
/// This records a fit; held-out support is still required to qualify its use.
pub fn fit(items: &[Observation]) -> Value {
    let scored: Vec<_> = items
        .iter()
        .filter(|i| i.outcome == Outcome::Answered && i.truth.is_some())
        .filter_map(|i| {
            i.probability.map(|conf| Scored {
                conf,
                correct: i.answer == i.truth,
            })
        })
        .collect();
    let mut blocks: Vec<(f64, f64, usize, f64)> = Vec::new();
    for bin in reliability(&scored) {
        blocks.push((bin.lo, bin.hi, bin.n, bin.accuracy * bin.n as f64));
        while blocks.len() > 1 {
            let n = blocks.len();
            let a = blocks[n - 2];
            let b = blocks[n - 1];
            if a.3 / a.2 as f64 <= b.3 / b.2 as f64 {
                break;
            }
            blocks.pop();
            blocks.pop();
            blocks.push((a.0, b.1, a.2 + b.2, a.3 + b.3));
        }
    }
    json!({"fitted":!scored.is_empty(),"method":"binned-isotonic/1","support":scored.len(),"blocks":blocks.iter().map(|(lo,hi,n,hits)|json!({"lo":lo,"hi":hi,"n":n,"calibrated_probability":hits / *n as f64})).collect::<Vec<_>>(),"qualified":false})
}
