#!/usr/bin/env bash
# Installs a local sound pack into frontend/public/sounds/ (gitignored, never committed).
#
# The source is a folder of the reference web client's captured sounds, laid out as
#   <src>/default/{move-self,capture,castle,move-check,promote}.mp3   (the board's sound theme)
#   <src>/assets-ds/{brilliant,correct,incorrect,training-result-*}-<hash>.mp3
# and defaults to ~/work/chesscom/sounds/web. Those files must never be committed (THIRD_PARTY.md).
#
# Usage (from Windows, the source being in WSL):  wsl bash tools/sounds/install-sound-pack.sh [src]
set -euo pipefail

src="${1:-$HOME/work/chesscom/sounds/web}"
out="$(cd "$(dirname "$0")/../.." && pwd)/frontend/public/sounds"
mkdir -p "$out"

# One source file for a pattern, or nothing.
pick() { compgen -G "$src/$1" | head -n 1 || true; }

copied=0
while read -r name pattern; do
  file="$(pick "$pattern")"
  if [[ -z "$file" ]]; then
    echo "missing: $name ($pattern), the synth plays it" >&2
    continue
  fi
  cp "$file" "$out/$name.mp3"
  echo "$name.mp3  <-  ${file#"$src"/}"
  copied=$((copied + 1))
done <<'EOF'
move default/move-self.mp3
capture default/capture.mp3
castle default/castle.mp3
check default/move-check.mp3
promote default/promote.mp3
brilliant assets-ds/brilliant-*.mp3
correct assets-ds/correct-*.mp3
incorrect assets-ds/incorrect-*.mp3
result_good assets-ds/training-result-good-*.mp3
result_ok assets-ds/training-result-ok-*.mp3
result_bad assets-ds/training-result-bad-*.mp3
EOF

echo "$copied of 11 sounds installed in $out"
