import type { Classification } from "../api/generated/Classification";
import { ClassificationIcon } from "./ClassificationIcon";
import { Piece } from "./pieces";

const FILES = "abcdefgh";

interface Props {
  fen: string;
  /** Black at the bottom. */
  flipped?: boolean;
  /** Squares of the last move, shown as on the review board. */
  lastMove?: { from: string; to: string } | null;
  /** A classification badge on a square, as the review board pins one to the moved piece. */
  badge?: { square: string; value: Classification } | null;
  label: string;
}

/** Pieces by square from a FEN's placement field; display only. */
function placement(fen: string): Map<string, string> {
  const squares = new Map<string, string>();
  const ranks = (fen.split(" ")[0] ?? "").split("/");
  ranks.forEach((row, r) => {
    let file = 0;
    for (const ch of row) {
      if (/\d/.test(ch)) file += Number(ch);
      else {
        squares.set(`${FILES[file]}${8 - r}`, ch);
        file += 1;
      }
    }
  });
  return squares;
}

/** A static, non-interactive board: the landing page's diagram. No engine, no chessground. */
export function MiniBoard({ fen, flipped = false, lastMove, badge, label }: Props) {
  const pieces = placement(fen);
  const rows = flipped ? [1, 2, 3, 4, 5, 6, 7, 8] : [8, 7, 6, 5, 4, 3, 2, 1];
  const cols = flipped ? [...FILES].reverse() : [...FILES];

  return (
    <div className="mini-board" role="img" aria-label={label}>
      {rows.map((rank, r) =>
        cols.map((file, f) => {
          const sq = `${file}${rank}`;
          // a1 is dark: file index + rank is odd on dark squares.
          const dark = (FILES.indexOf(file) + rank) % 2 === 1;
          const piece = pieces.get(sq);
          const lit = lastMove && (lastMove.from === sq || lastMove.to === sq);
          return (
            <div key={sq} className={`mini-board__sq ${dark ? "is-dark" : "is-light"} ${lit ? "is-lit" : ""} ${badge?.square === sq ? "has-badge" : ""}`}>
              {f === 0 && <span className="mini-board__rank">{rank}</span>}
              {r === 7 && <span className="mini-board__file">{file}</span>}
              {piece && <Piece kind={piece} colour={piece === piece.toUpperCase() ? "w" : "b"} />}
              {badge?.square === sq && (
                <span className="mini-board__badge">
                  <ClassificationIcon value={badge.value} size={30} />
                </span>
              )}
            </div>
          );
        }),
      )}
    </div>
  );
}
