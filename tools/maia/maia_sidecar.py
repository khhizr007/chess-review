#!/usr/bin/env python3
"""Maia-2 sidecar for the review server: P(move | rating) for the coach.

Maia-2 (Tang et al., NeurIPS 2024; https://github.com/CSSLab/maia2, MIT licence) predicts the
moves human players of a given rating make. The review server starts this script when
`MAIA_COMMAND` is set (see `backend/crates/review-server/src/human.rs`) and asks, per reviewed
move, how likely the played move and the engine's best move are at the mover's rating. The coach
uses that to say "hard to find at this level" and "looks natural, but".

Protocol: one JSON object per line.

    stdout <- {"ready": true, "name": "Maia-2 rapid"}           (once, after the model loads)
    stdin  -> {"queries": [{"fen": "...", "moves": ["e2e4"], "elo_self": 1200, "elo_oppo": 1250}]}
    stdout <- {"odds": [[0.41]]}                                 (or {"error": "..."})

Setup (not part of the build; the model weights are downloaded by the maia2 package on first
use, from the location its authors publish):

    python3 -m venv ~/maia && ~/maia/bin/pip install maia2
    MAIA_COMMAND="$HOME/maia/bin/python3 tools/maia/maia_sidecar.py --type rapid"

Diagnostics go to stderr; stdout carries only protocol lines.
"""

from __future__ import annotations

import argparse
import json
import sys


def log(*parts: object) -> None:
    print(*parts, file=sys.stderr, flush=True)


def reply(obj: dict) -> None:
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("--type", default="rapid", choices=("rapid", "blitz"),
                   help="which Maia-2 model to load")
    p.add_argument("--device", default="cpu", choices=("cpu", "gpu"))
    args = p.parse_args()

    try:
        from maia2 import inference, model  # noqa: PLC0415 (imported late: it is slow)
    except ImportError as e:
        log(f"maia2 is not installed: {e}")
        return 1
    log(f"loading Maia-2 ({args.type}, {args.device})")
    net = model.from_pretrained(type=args.type, device=args.device)
    prepared = inference.prepare()
    reply({"ready": True, "name": f"Maia-2 {args.type}"})

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            queries = json.loads(line)["queries"]
            odds = []
            for q in queries:
                probs, _win = inference.inference_each(
                    net, prepared, q["fen"], int(q["elo_self"]), int(q["elo_oppo"]))
                odds.append([float(probs.get(m, 0.0)) for m in q["moves"]])
            reply({"odds": odds})
        except Exception as e:  # noqa: BLE001 (any failure is reported to the server)
            log(f"query failed: {e!r}")
            reply({"error": repr(e)})
    return 0


if __name__ == "__main__":
    sys.exit(main())
