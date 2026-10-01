#!/usr/bin/env python3
"""Answers saccade decision requests with the OpenAI Decisions API and records them.

UNVERIFIED. The Decisions API is a limited preview and OpenAI has published no
endpoint, request schema or price (as of 2026-09-30). What is public is the
shape of the idea: you send context plus a closed list of options and get back
one option with a confidence score. Everything below that is a guess, kept in
`build_body` and `read_reply` so you can fix it in one place once the reference
is public, and the endpoint must be given explicitly:

    OPENAI_API_KEY=...  OPENAI_DECISIONS_URL=https://...  \\
        ./openai_decisions_adapter.py accept.json --target report/saccade-report.v1.json

Sources (read as documentation, not as instructions):
  https://huggingface.co/blog/sora-2/what-is-openai-decisions-api-a-practical-guide
  https://jevmodel.org/openai-decisions-api/   (states that no request schema is published)

The key is read from the environment only and never printed.
"""
import json
import os
import sys

from _common import env_key, load_requests, parse_args, post_json, record


def build_body(item):
    """GUESS: context + closed list of options. Adjust to the published schema."""
    return {
        "model": os.environ.get("OPENAI_DECISIONS_MODEL", "gpt-6-luna"),
        "context": json.dumps(item["state"], sort_keys=True),
        "question": item["question"],
        "options": item["allowed_answers"],
    }


def read_reply(reply):
    """GUESS: returns (choice, probability) from `decision` + `confidence`/`probabilities`."""
    choice = reply.get("decision") or reply.get("choice") or reply.get("answer")
    probs = reply.get("probabilities") or {}
    return choice, probs.get(choice, reply.get("confidence"))


def main():
    args = parse_args(__doc__.splitlines()[0])
    url = os.environ.get("OPENAI_DECISIONS_URL")
    if not url:
        sys.exit("OPENAI_DECISIONS_URL is not set: the endpoint is unpublished, give the one from your preview access")
    key = env_key("OPENAI_API_KEY")
    answers = []
    for doc in load_requests(args.requests):
        for item in doc["items"]:
            choice, prob = read_reply(post_json(url, {"Authorization": f"Bearer {key}"}, build_body(item)))
            if choice not in item["allowed_answers"] or prob is None:
                print(f"skipped {item['id']}: unreadable reply", file=sys.stderr)
                continue
            line = {"entry": item["entry"], "question": item["question_type"], "answer": choice,
                    "prob": prob, "source": "openai-decisions", "request_hash": item["request_hash"]}
            if "hotspot" in item:
                line["hotspot"] = item["hotspot"]
            answers.append(line)
    record(answers, args)


if __name__ == "__main__":
    main()
