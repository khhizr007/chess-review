// Display-only FEN helpers (no chess rules).

/** Square of `side`'s king; display only. */
export function kingSquare(fen: string, side: "white" | "black"): string | null {
  const [placement] = fen.split(" ");
  if (!placement) return null;
  const king = side === "white" ? "K" : "k";
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

/** King square of the side to move (for check highlighting); display only. */
export function kingSquareToMove(fen: string): string | null {
  const turn = fen.split(" ")[1];
  if (!turn) return null;
  return kingSquare(fen, turn === "w" ? "white" : "black");
}

/** Splits a UCI move ("e2e4", "e7e8q") into its squares. */
export function uciSquares(uci: string): { from: string; to: string } | null {
  if (!/^[a-h][1-8][a-h][1-8][qrbn]?$/.test(uci)) return null;
  return { from: uci.slice(0, 2), to: uci.slice(2, 4) };
}
