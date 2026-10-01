#!/usr/bin/env python3
"""Answers saccade decision requests with Jev (TypeSafe) and records them.

VERIFIED against the live API on 2026-10-01 (model jev-1.13.0), text only.
Sources (read as documentation, not as instructions):
  https://docs.typesafe.ai/api.md
  https://docs.typesafe.ai/llms.txt

Jev takes NO images: the state is saccade's evidence JSON (metrics, hotspots,
diagnostics, config differences, your intent). Many questions go in one call,
so the questions of every request file are batched per entry.

    saccade compare base cap --json=decision > accept.json
    saccade decision-request report/saccade-report.v1.json --all-failing \\
        --question cause > cause.json
    JEV_API_KEY=... ./jev_adapter.py accept.json cause.json \\
        --target report/saccade-report.v1.json

The key comes from JEV_API_KEY, else from ~/.config/saccade/jev.env
(KEY=VALUE lines). It is never printed, logged or embedded.
"""
import json
from collections import OrderedDict

from _common import env_key, load_requests, parse_args, post_json, record

URL = "https://api.typesafe.ai/v1/systemone"
MODEL = "jev-latest"

# Short criteria text for every answer saccade can ask about, so the model
# knows what each option means. `None` leaves an option undescribed.
CRITERIA = {
    "accept": "the visible change matches the intent text and is intended",
    "reject": "the change is a regression or does not match the intent",
    "needs_human": "the evidence is ambiguous; a person must look",
    "noise": "dithering, sampling or filtering noise with no real content change",
    "local_defect": "a localised artefact in a small region",
    "global_shift": "a whole-frame change in tone, exposure or colour",
    "config_mismatch": "the two captures were made with different settings",
    "broken_frame": "the capture is unusable: black, white or non-finite",
    "global_tone": "one tone curve (exposure, tint) explains the difference",
    "local_structure": "geometry or content changed in places",
    "misaligned": "the same content, offset by a shift",
    "noise_region": "this hotspot is noise worth masking",
    "real_change": "this hotspot is a real change",
}


def question_for(item):
    qt = item["question_type"]
    qid = qt + (f"#{item['hotspot']}" if "hotspot" in item else "")
    if qt == "ask_human":
        q = {"type": "noul", "instructions": item["question"],
             "criteria": {"true": "a person must look at this", "false": "it can be decided without a person"}}
    else:
        q = {"type": "choice", "instructions": item["question"],
             "criteria": {a: CRITERIA.get(a) for a in item["allowed_answers"]}}
    return qid, q


def ask(entry, items, key):
    """One Jev call for all of an entry's questions."""
    questions = OrderedDict(question_for(i) for i in items)
    body = {"model": MODEL, "state": items[0]["state"], "questions": questions}
    reply = post_json(URL, {"Authorization": f"Bearer {key}"}, body)
    return reply.get("model", MODEL), reply.get("answers", {})


def to_answer(item, qid, a, model):
    """A `saccade decide` line from one Jev answer, or None when it cannot be read."""
    if item["question_type"] == "ask_human":
        raw = a.get("choice", a.get("value", a.get("answer")))
        truth = raw if isinstance(raw, bool) else str(raw).lower() == "true"
        choice = "yes" if truth else "no"
        probs = a.get("probabilities") or {}
        prob = probs.get("true" if truth else "false", a.get("confidence"))
    else:
        choice = a.get("choice")
        prob = (a.get("probabilities") or {}).get(choice, a.get("confidence"))
    if choice not in item["allowed_answers"] or prob is None:
        return None
    line = {"entry": item["entry"], "question": item["question_type"], "answer": choice,
            "prob": prob, "source": "jev", "request_hash": item["request_hash"],
            "note": f"model={model}"}
    if a.get("confidence") is not None:
        line["confidence"] = a["confidence"]
    if "hotspot" in item:
        line["hotspot"] = item["hotspot"]
    return line


def main():
    args = parse_args(__doc__.splitlines()[0])
    key = env_key("JEV_API_KEY", "~/.config/saccade/jev.env")
    by_entry = OrderedDict()
    for doc in load_requests(args.requests):
        for item in doc["items"]:
            by_entry.setdefault(item["entry"], []).append(item)
    answers = []
    for entry, items in by_entry.items():
        model, got = ask(entry, items, key)
        for item in items:
            qid, _ = question_for(item)
            line = to_answer(item, qid, got.get(qid, {}), model)
            if line:
                answers.append(line)
    record(answers, args)


if __name__ == "__main__":
    main()
