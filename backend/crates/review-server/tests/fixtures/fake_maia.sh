#!/bin/sh
# A stand-in for tools/maia/maia_sidecar.py in tests: it answers every query
# with the odds 0.5 for the first move and 0.25 for the second.
echo '{"ready": true, "name": "fake Maia"}'
while read -r line; do
  n=$(printf '%s' "$line" | grep -o '"fen"' | wc -l)
  out=""
  i=0
  while [ "$i" -lt "$n" ]; do
    if [ -n "$out" ]; then out="$out,"; fi
    out="$out[0.5,0.25]"
    i=$((i + 1))
  done
  echo "{\"odds\": [$out]}"
done
