import { Chessboard } from "react-chessboard";
import type { Arrow } from "react-chessboard";
import type { CSSProperties } from "react";

interface Props {
  fen: string;
  flipped: boolean;
  arrows: Arrow[];
  squareStyles: Record<string, CSSProperties>;
}

/** Read-only board; all positions and annotations come from the backend. */
export function BoardView({ fen, flipped, arrows, squareStyles }: Props) {
  return (
    <div className="board">
      <Chessboard
        options={{
          id: "review-board",
          position: fen,
          boardOrientation: flipped ? "black" : "white",
          allowDragging: false,
          allowDrawingArrows: true,
          arrows,
          squareStyles,
          showNotation: true,
          animationDurationInMs: 150,
          darkSquareStyle: { backgroundColor: "var(--board-dark)" },
          lightSquareStyle: { backgroundColor: "var(--board-light)" },
        }}
      />
    </div>
  );
}
