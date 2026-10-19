import { Chessboard } from "react-chessboard";
import type { Arrow } from "react-chessboard";
import type { CSSProperties } from "react";
import type { Classification } from "../api/generated/Classification";
import type { Side } from "../api/generated/Side";
import { ClassificationIcon } from "./ClassificationIcon";
import { boardPieces } from "../lib/boardPieces";

export interface BoardBadge {
  square: string;
  classification: Classification;
}

export interface BoardRetry {
  /** Only this side's pieces can be dragged. */
  side: Side;
  /** Returns whether the piece stays on the target square. */
  onDrop: (from: string, to: string) => boolean;
}

interface Props {
  fen: string;
  flipped: boolean;
  arrows: Arrow[];
  squareStyles: Record<string, CSSProperties>;
  badge?: BoardBadge | null;
  retry?: BoardRetry | null;
}

/** Position of a square's top-right corner, in % of the board, for the badge overlay. */
function corner(square: string, flipped: boolean): { left: string; top: string } {
  const file = square.charCodeAt(0) - 97;
  const rank = Number(square[1]);
  const col = flipped ? 7 - file : file;
  const row = flipped ? rank - 1 : 8 - rank;
  return { left: `${(col + 1) * 12.5}%`, top: `${row * 12.5}%` };
}

const NOTATION: CSSProperties = { fontSize: "clamp(10px, 2.05cqw, 17px)", fontWeight: 600, lineHeight: 1 };

/** Read-only board (draggable only in retry mode); positions and annotations come from the backend. */
export function BoardView({ fen, flipped, arrows, squareStyles, badge, retry }: Props) {
  return (
    <div className="board-wrap">
      <div className="board">
        <Chessboard
          options={{
            id: "review-board",
            pieces: boardPieces,
            position: fen,
            boardOrientation: flipped ? "black" : "white",
            allowDragging: !!retry,
            canDragPiece: ({ piece }) => !!retry && piece.pieceType.startsWith(retry.side === "white" ? "w" : "b"),
            onPieceDrop: ({ sourceSquare, targetSquare }) =>
              !!retry && !!targetSquare && targetSquare !== sourceSquare && retry.onDrop(sourceSquare, targetSquare),
            allowDrawingArrows: true,
            arrows,
            squareStyles,
            showNotation: true,
            animationDurationInMs: 180,
            darkSquareStyle: { backgroundColor: "var(--board-dark)" },
            lightSquareStyle: { backgroundColor: "var(--board-light)" },
            darkSquareNotationStyle: { color: "var(--board-light)" },
            lightSquareNotationStyle: { color: "var(--board-dark)" },
            alphaNotationStyle: { ...NOTATION, right: "5%", bottom: "4%" },
            numericNotationStyle: { ...NOTATION, left: "5%", top: "4%" },
            draggingPieceStyle: { transform: "scale(1.12)", filter: "drop-shadow(0 6px 6px rgba(0,0,0,.35))" },
            dropSquareStyle: { boxShadow: "inset 0 0 0 4px rgba(255,255,255,.65)" },
          }}
        />
      </div>
      {badge && (
        <div className="board-badge" style={corner(badge.square, flipped)} key={`${badge.square}-${badge.classification}-${fen}`}>
          <ClassificationIcon value={badge.classification} size={64} />
        </div>
      )}
    </div>
  );
}
