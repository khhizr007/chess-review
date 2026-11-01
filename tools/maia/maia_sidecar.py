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

Setup (not part of the build; the maia2 package downloads the model weights on first use, from
the location its authors publish, into --models-dir). maia2 0.10.0 pins torch 2.8.0, numpy 2.1.3
and pandas 2.2.3, which need Python 3.12 or 3.13; see README.md "Maia-2 (optional)":

    uv venv --python 3.12 ~/maia312
    uv pip install --python ~/maia312/bin/python torch==2.8.0 --index-url https://download.pytorch.org/whl/cpu
    uv pip install --python ~/maia312/bin/python maia2==0.10.0 --extra-index-url https://download.pytorch.org/whl/cpu --index-strategy unsafe-best-match
    MAIA_COMMAND="$HOME/maia312/bin/python tools/maia/maia_sidecar.py --type rapid"

The server asks only about moves from move 11 on: Maia-2 was trained without the first ten moves.

Diagnostics go to stderr; stdout carries only protocol lines. The maia2 package prints progress
with print(), so everything but the protocol is sent to stderr.
"""

from __future__ import annotations

import argparse
import json
import os
import sys

# The protocol's stream; sys.stdout itself is pointed at stderr in main().
PROTOCOL = sys.stdout


def log(*parts: object) -> None:
    print(*parts, file=sys.stderr, flush=True)


def reply(obj: dict) -> None:
    PROTOCOL.write(json.dumps(obj) + "\n")
    PROTOCOL.flush()


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("--type", default="rapid", choices=("rapid", "blitz"),
                   help="which Maia-2 model to load")
    p.add_argument("--device", default="cpu", choices=("cpu", "gpu"))
    p.add_argument("--models-dir", default=os.path.expanduser("~/.cache/maia2"),
                   help="where the weights are kept (downloaded there on first use)")
    p.add_argument("--threads", type=int, default=2,
                   help="CPU threads for inference (the engine needs the rest)")
    args = p.parse_args()
    sys.stdout = sys.stderr

    try:
        import torch  # noqa: PLC0415 (imported late: it is slow)
        from maia2 import inference, model  # noqa: PLC0415
    except ImportError as e:
        log(f"maia2 is not installed: {e}")
        return 1
    torch.set_num_threads(max(1, args.threads))
    log(f"loading Maia-2 ({args.type}, {args.device}) from {args.models_dir}")
    net = model.from_pretrained(type=args.type, device=args.device, save_root=args.models_dir)
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
