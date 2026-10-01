#!/usr/bin/env python3
"""Answers saccade decision requests with any chat model that supports
JSON-schema constrained output, and records them.

Works with every OpenAI-compatible chat-completions endpoint that honours
`response_format: {"type": "json_schema"}` (OpenAI, vLLM, llama.cpp server,
Ollama's /v1, many hosted providers). The schema allows only the question's
allowed answers, so the reply cannot leave the closed set. The probability is
what the model reports about itself: treat it as weaker than a bounded-decision
model's, and keep the [decisions] gate off or high.

    LLM_API_KEY=... LLM_MODEL=gpt-5-mini ./llm_json_adapter.py accept.json \\
        --target report/saccade-report.v1.json

Environment: LLM_API_KEY (required, never printed), LLM_MODEL (required),
LLM_BASE_URL (default https://api.openai.com/v1).
Reference: https://platform.openai.com/docs/guides/structured-outputs
"""
import json
import os
import sys

from _common import env_key, load_requests, parse_args, post_json, record

SYSTEM = ("You judge visual-regression evidence from the FLIP perceptual metric. "
          "Answer only from the allowed answers, from the evidence given.")


def schema_for(item):
    return {"type": "object", "additionalProperties": False, "required": ["answer", "probability"],
            "properties": {"answer": {"type": "string", "enum": item["allowed_answers"]},
                           "probability": {"type": "number", "minimum": 0, "maximum": 1}}}


def main():
    args = parse_args(__doc__.splitlines()[0])
    key = env_key("LLM_API_KEY")
    model = os.environ.get("LLM_MODEL") or sys.exit("LLM_MODEL is not set")
    base = os.environ.get("LLM_BASE_URL", "https://api.openai.com/v1").rstrip("/")
    answers = []
    for doc in load_requests(args.requests):
        for item in doc["items"]:
            body = {"model": model, "messages": [
                        {"role": "system", "content": SYSTEM},
                        {"role": "user", "content": item["question"] + "\n\n" + json.dumps(item["state"], sort_keys=True)}],
                    "response_format": {"type": "json_schema", "json_schema": {
                        "name": "decision", "strict": True, "schema": schema_for(item)}}}
            reply = post_json(f"{base}/chat/completions", {"Authorization": f"Bearer {key}"}, body, timeout=120)
            try:
                out = json.loads(reply["choices"][0]["message"]["content"])
                choice, prob = out["answer"], float(out["probability"])
            except (KeyError, IndexError, ValueError, TypeError):
                print(f"skipped {item['id']}: unreadable reply", file=sys.stderr)
                continue
            if choice not in item["allowed_answers"]:
                continue
            line = {"entry": item["entry"], "question": item["question_type"], "answer": choice,
                    "prob": min(max(prob, 0.0), 1.0), "source": model, "request_hash": item["request_hash"],
                    "note": "self-reported probability"}
            if "hotspot" in item:
                line["hotspot"] = item["hotspot"]
            answers.append(line)
    record(answers, args)


if __name__ == "__main__":
    main()
