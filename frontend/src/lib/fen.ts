// Display-only FEN helpers (no chess rules).

/** King square of the side to move (for check highlighting); display only. */
export function kingSquareToMove(fen: string): string | null {
  const [placement, turn] = fen.split(" ");
  if (!placement || !turn) return null;
  const king = turn === "w" ? "K" : "k";
  const ranks = placement.split("/");
  for (let r = 0; r < ranks.length; r++) {
    let file = 0;
    for (const ch of ranks[r] ?? "") {
      if (/\d/.test(ch)) {
        file += Number(ch);
      } else {
        if (ch === king) return `${"abcdefgh"[file]}${8 - r}`;
        file += 1;
      }
    }
  }
  return null;
}

/** Splits a UCI move ("e2e4", "e7e8q") into its squares. */
export function uciSquares(uci: string): { from: string; to: string } | null {
  if (!/^[a-h][1-8][a-h][1-8][qrbn]?$/.test(uci)) return null;
  return { from: uci.slice(0, 2), to: uci.slice(2, 4) };
}
