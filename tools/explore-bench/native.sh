#!/bin/bash
# Usage: native.sh <engine> <fens.txt> <depth>
# Server-like searches (Threads 1, Hash 64, MultiPV 3, cold per position); prints "ms cp|mate best" per FEN.
coproc SF { "$1"; }
send() { echo "$1" >&"${SF[1]}"; }
waitfor() { while read -r -u "${SF[0]}" line; do [[ "$line" == $1* ]] && { echo "$line"; return; }; LAST_INFO="$line"; [[ "$line" == info*" multipv 1 "*" pv "* && "$line" != *bound* ]] && BEST1="$line"; done; }
send uci; waitfor uciok >/dev/null
send "setoption name Threads value 1"; send "setoption name Hash value 64"; send "setoption name MultiPV value 3"
while read -r fen; do
  [ -z "$fen" ] && continue
  send ucinewgame; send "setoption name Clear Hash"; send isready; waitfor readyok >/dev/null
  BEST1=""; s=$(date +%s%N)
  send "position fen $fen"; send "go depth $3"; waitfor bestmove >/dev/null
  e=$(( ($(date +%s%N) - s) / 1000000 ))
  score=$(sed -E 's/.* score (cp|mate) (-?[0-9]+).*/\1 \2/' <<<"$BEST1"); pv=$(sed -E 's/.* pv ([a-h1-8qrbn]+).*/\1/' <<<"$BEST1")
  echo "$e $score $pv"
done < "$2"
send quit
