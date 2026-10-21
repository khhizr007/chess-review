import { Chessboard } from "react-chessboard";
import type { Arrow } from "react-chessboard";
import { useMemo, useRef, useState, type CSSProperties } from "react";
import type { Classification } from "../api/generated/Classification";
import type { Side } from "../api/generated/Side";
import type { MoveTarget, PromotionPiece } from "../lib/chess";
import { ClassificationIcon } from "./ClassificationIcon";
import { Piece } from "./pieces";
import { boardPieces } from "../lib/boardPieces";

export interface BoardBadge {
  square: string;
  classification: Classification;
}

/** Moving pieces on the board (a retry, or exploring a sideline). */
export interface BoardMoves {
  /** Whether the piece on `square` (of `side`) can be picked up. */
  canDrag: (square: string, side: Side) => boolean;
  /** Plays a move (a drop or a click on a target square); returns whether the piece stays there. */
  onDrop: (from: string, to: string) => boolean;
  /** The squares the piece on `square` can move to: the move hints, and where a click moves it. */
  targets: (square: string) => MoveTarget[];
}

/** A pawn waiting on its promotion square for the reader to choose a piece. */
export interface BoardPromotion {
  square: string;
  side: Side;
  /** The chosen piece, or null to take the move back. */
  onPick: (piece: PromotionPiece | null) => void;
}

interface Props {
  fen: string;
  flipped: boolean;
  arrows: Arrow[];
  squareStyles: Record<string, CSSProperties>;
  badge?: BoardBadge | null;
  moves?: BoardMoves | null;
  promotion?: BoardPromotion | null;
}

/** Position of a square's top-left corner, in % of the board. */
function topLeft(square: string, flipped: boolean): { col: number; row: number } {
  const file = square.charCodeAt(0) - 97;
  const rank = Number(square[1]);
  return { col: flipped ? 7 - file : file, row: flipped ? rank - 1 : 8 - rank };
}

/** Position of a square's top-right corner, in % of the board, for the badge overlay. */
function corner(square: string, flipped: boolean): { left: string; top: string } {
  const { col, row } = topLeft(square, flipped);
  return { left: `${(col + 1) * 12.5}%`, top: `${row * 12.5}%` };
}

const NOTATION: CSSProperties = { fontSize: "clamp(10px, 2.05cqw, 17px)", fontWeight: 600, lineHeight: 1 };
const PROMOTION_PIECES: PromotionPiece[] = ["q", "n", "r", "b"];
const PROMOTION_NAMES: Record<PromotionPiece, string> = { q: "Queen", n: "Knight", r: "Rook", b: "Bishop" };

/** The four promotion pieces, stacked from the promotion square towards the middle of the board. */
function PromotionPicker({ promotion, flipped }: { promotion: BoardPromotion; flipped: boolean }) {
  const { col, row } = topLeft(promotion.square, flipped);
  const down = row === 0;
  const colour = promotion.side === "white" ? "w" : "b";
  return (
    <div className="promotion" role="dialog" aria-label="Choose a promotion piece" onClick={() => promotion.onPick(null)}>
      {PROMOTION_PIECES.map((piece, i) => (
        <button
          key={piece}
          type="button"
          className="promotion__piece"
          style={{ left: `${col * 12.5}%`, top: `${(down ? row + i : row - i) * 12.5}%` }}
          aria-label={PROMOTION_NAMES[piece]}
          title={PROMOTION_NAMES[piece]}
          autoFocus={i === 0}
          onClick={(e) => {
            e.stopPropagation();
            promotion.onPick(piece);
          }}
        >
          <Piece kind={piece} colour={colour} />
        </button>
      ))}
    </div>
  );
}

/**
 * The board. Positions and annotations come from the backend, except in sidelines the reader plays
 * (docs/explore-mode.md §5.1); pieces can be moved only when `moves` is given, by dragging or by
 * clicking the piece and then a square. A picked-up piece shows where it can go, as on Chess.com: a
 * growing highlight on its square, dots on free squares and rings on pieces it can take.
 */
export function BoardView({ fen, flipped, arrows, squareStyles, badge, moves, promotion }: Props) {
  // The selection belongs to one position: a new position (a move, a step) clears it.
  const [picked, setPicked] = useState<{ fen: string; square: string } | null>(null);
  const selected = moves && !promotion && picked?.fen === fen ? picked.square : null;
  const targets = useMemo(() => {
    const map = new Map<string, MoveTarget>();
    if (selected && moves) for (const t of moves.targets(selected)) map.set(t.square, t);
    return map;
  }, [selected, moves]);

  const sideOf = (pieceType: string) => (pieceType.startsWith("w") ? "white" : "black");
  // The press a click ends, when the press already acted (a mouse); touch only sends the click.
  const press = useRef<{ square: string; deselect: boolean } | null>(null);

  /**
   * Acts on a square as a press does: a target square of the picked-up piece plays the move, a
   * movable piece is picked up, anything else puts the piece down. The picked-up piece itself is
   * put down only by a click (the release), so it can still be dragged.
   */
  const act = (square: string, pieceType: string | null): { deselect: boolean } => {
    if (!moves || promotion) return { deselect: false };
    if (selected && targets.has(square)) {
      setPicked(null);
      moves.onDrop(selected, square);
      return { deselect: false };
    }
    if (square === selected) return { deselect: true };
    const movable = pieceType && moves.canDrag(square, sideOf(pieceType));
    setPicked(movable ? { fen, square } : null);
    return { deselect: false };
  };

  // A mouse acts on the press, as on Chess.com: the piece is picked up (and shows its moves) the
  // moment the button goes down, and a click on a target moves at once.
  const onSquareMouseDown = (square: string, pieceType: string | null, button: number) => {
    if (button !== 0) return;
    press.current = { square, ...act(square, pieceType) };
  };
  const onSquareClick = (square: string, pieceType: string | null) => {
    const pressed = press.current;
    press.current = null;
    if (pressed?.square === square) {
      // The release of a press that already acted: it only puts down a piece clicked twice.
      if (pressed.deselect) setPicked(null);
      return;
    }
    // A tap (touch), or a release elsewhere than the press.
    const { deselect } = act(square, pieceType);
    if (deselect) setPicked(null);
  };

  return (
    <div className="board-wrap">
      <div className="board">
        <Chessboard
          options={{
            id: "review-board",
            pieces: boardPieces,
            position: fen,
            boardOrientation: flipped ? "black" : "white",
            allowDragging: !!moves && !promotion,
            canDragPiece: ({ piece, square }) =>
              !!moves && !!square && moves.canDrag(square, piece.pieceType.startsWith("w") ? "white" : "black"),
            // A dragged piece shows its moves too; a drop back on its square leaves it picked up.
            onPieceDrag: ({ square }) => {
              if (square) setPicked({ fen, square });
            },
            onPieceDrop: ({ sourceSquare, targetSquare }) => {
              if (!moves || !targetSquare || targetSquare === sourceSquare) return false;
              const kept = moves.onDrop(sourceSquare, targetSquare);
              if (kept) setPicked(null);
              return kept;
            },
            onSquareMouseDown: ({ piece, square }, e) => onSquareMouseDown(square, piece?.pieceType ?? null, e.button),
            onSquareClick: ({ piece, square }) => onSquareClick(square, piece?.pieceType ?? null),
            // Each square: its highlight, the selection under the piece, then the move hint over it.
            squareRenderer: ({ square, children }) => {
              const target = targets.get(square);
              return (
                <div className={`sq ${target ? "sq--target" : ""}`} style={squareStyles[square]}>
                  {square === selected && (
                    <span className="sq-select" aria-hidden="true">
                      <span />
                    </span>
                  )}
                  <div className="sq-piece">{children}</div>
                  {target && <span className={`sq-hint ${target.capture ? "sq-hint--capture" : ""}`} aria-hidden="true" />}
                </div>
              );
            },
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
      {promotion && <PromotionPicker promotion={promotion} flipped={flipped} />}
    </div>
  );
}
