import { Chessboard } from "react-chessboard";
import type { Arrow } from "react-chessboard";
import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import type { Classification } from "../api/generated/Classification";
import type { Side } from "../api/generated/Side";
import type { MoveTarget, PromotionPiece } from "../lib/chess";
import { ClassificationIcon, EndMarkIcon, type EndMark } from "./ClassificationIcon";
import { Piece } from "./pieces";
import { boardPieces } from "../lib/boardPieces";

export interface BoardBadge {
  square: string;
  classification: Classification;
}

/** A king's badge at the end of the game. */
export interface BoardEndMark {
  square: string;
  mark: EndMark;
}

/** Moving pieces on the board (a retry, or exploring a sideline). */
export interface BoardMoves {
  /** Whether `from`→`to` is a legal move. The board refuses any other drop or click itself. */
  legal: (from: string, to: string) => boolean;
  /** Plays a legal move (a drop or a click on a target square); returns whether the piece stays there. */
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
  /** The kings' badges when the position shown ends the game. */
  endMarks?: BoardEndMark[];
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
/** The outline of the square under a held or dragged piece: white, 5% of the square (1cqw is 1% of the board). */
const HOVER_OUTLINE = "inset 0 0 0 0.625cqw rgba(255, 255, 255, 0.65)";
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
 * clicking the piece and then a square. As on Chess.com, any piece can be picked up, whichever side
 * is to move: it shows a growing highlight on its square, and, if it can move, dots on free squares
 * and rings on pieces it can take. A move the rules forbid flashes its square red and is not played.
 */
export function BoardView({ fen, flipped, arrows, squareStyles, badge, endMarks, moves, promotion }: Props) {
  // The selection and the right-click marks belong to one position: a move or a step clears them.
  const [picked, setPicked] = useState<{ fen: string; square: string } | null>(null);
  const selected = moves && !promotion && picked?.fen === fen ? picked.square : null;
  const targets = useMemo(() => {
    const map = new Map<string, MoveTarget>();
    if (selected && moves) for (const t of moves.targets(selected)) map.set(t.square, t);
    return map;
  }, [selected, moves]);
  const [marked, setMarked] = useState<{ fen: string; squares: string[] }>({ fen, squares: [] });
  const marks = marked.fen === fen ? marked.squares : [];
  // The square of a piece held down, outlined until the release (then the drag outlines its square).
  const [held, setHeld] = useState<string | null>(null);
  // The square of the last refused move; `n` replays the flash when the same square is refused again.
  const [refused, setRefused] = useState<{ square: string; n: number } | null>(null);
  const refuse = (square: string) => setRefused((r) => ({ square, n: (r?.n ?? 0) + 1 }));

  useEffect(() => {
    if (!held) return;
    const release = () => setHeld(null);
    window.addEventListener("mouseup", release);
    return () => window.removeEventListener("mouseup", release);
  }, [held]);

  // The press a click ends, when the press already acted (a mouse); touch only sends the click.
  const press = useRef<{ square: string; again: boolean } | null>(null);

  /**
   * Acts on a square as a press does: a target square of the picked-up piece plays the move, any
   * other piece is picked up, and an empty square puts the piece down, flashing red as a refused
   * move. The picked-up piece itself ("again") is put down only by the release, so it can still be
   * dragged.
   */
  const act = (square: string, pieceType: string | null): "move" | "pick" | "again" | "none" => {
    if (!moves || promotion) return "none";
    if (selected && targets.has(square)) {
      setPicked(null);
      moves.onDrop(selected, square);
      return "move";
    }
    if (square === selected) return "again";
    if (pieceType) {
      setPicked({ fen, square });
      return "pick";
    }
    if (selected) refuse(square);
    setPicked(null);
    return "none";
  };

  // A mouse acts on the press, as on Chess.com: the piece is picked up (and shows its moves) the
  // moment the button goes down, and a click on a target moves at once. The right button puts the
  // piece down; its click marks the square.
  const onSquareMouseDown = (square: string, pieceType: string | null, button: number) => {
    if (button === 2) {
      setPicked(null);
      press.current = null;
      return;
    }
    if (button !== 0) return;
    setMarked({ fen, squares: [] });
    const done = act(square, pieceType);
    press.current = { square, again: done === "again" };
    setHeld(done === "pick" || done === "again" ? square : null);
  };
  const onSquareClick = (square: string, pieceType: string | null) => {
    const pressed = press.current;
    press.current = null;
    if (pressed?.square === square) {
      // The release of a press that already acted: it only puts down a piece clicked twice.
      if (pressed.again) setPicked(null);
      return;
    }
    // A tap (touch), or a release elsewhere than the press.
    if (act(square, pieceType) === "again") setPicked(null);
  };
  const onSquareRightClick = (square: string) =>
    setMarked({ fen, squares: marks.includes(square) ? marks.filter((s) => s !== square) : [...marks, square] });

  return (
    <div className="board-wrap">
      <div className="board">
        <Chessboard
          options={{
            id: "review-board",
            pieces: boardPieces,
            position: fen,
            boardOrientation: flipped ? "black" : "white",
            // Any piece can be dragged, of either side; the drop decides.
            allowDragging: !!moves && !promotion,
            // A dragged piece shows its moves too.
            onPieceDrag: ({ square }) => {
              setHeld(null);
              if (square) setPicked({ fen, square });
            },
            // A refused drop flashes its square and goes back, still picked up. Off the board it just
            // goes back; back on its own square it is a click (it puts down a piece picked up before).
            onPieceDrop: ({ sourceSquare, targetSquare }) => {
              if (!moves || !targetSquare) return false;
              if (targetSquare === sourceSquare) {
                if (press.current?.again) setPicked(null);
                // The click that may follow this release is already handled.
                if (press.current) press.current = { square: sourceSquare, again: false };
                return false;
              }
              if (!moves.legal(sourceSquare, targetSquare)) {
                refuse(targetSquare);
                return false;
              }
              const kept = moves.onDrop(sourceSquare, targetSquare);
              if (kept) setPicked(null);
              return kept;
            },
            onSquareMouseDown: ({ piece, square }, e) => onSquareMouseDown(square, piece?.pieceType ?? null, e.button),
            onSquareClick: ({ piece, square }) => onSquareClick(square, piece?.pieceType ?? null),
            onSquareRightClick: ({ square }) => onSquareRightClick(square),
            // Each square: its highlight and mark, the selection and held outline under the piece,
            // then the move hint and the refused-move flash over it.
            squareRenderer: ({ square, children }) => {
              const target = targets.get(square);
              return (
                <div className={`sq ${target ? "sq--target" : ""}`} style={squareStyles[square]}>
                  {marks.includes(square) && <span className="sq-mark" aria-hidden="true" />}
                  {square === selected && (
                    <span className="sq-select" aria-hidden="true">
                      <span />
                    </span>
                  )}
                  {square === held && <span className="sq-held" style={{ boxShadow: HOVER_OUTLINE }} aria-hidden="true" />}
                  <div className="sq-piece">{children}</div>
                  {target && <span className={`sq-hint ${target.capture ? "sq-hint--capture" : ""}`} aria-hidden="true" />}
                  {refused?.square === square && (
                    <span key={refused.n} className="sq-refused" aria-hidden="true">
                      <span className="sq-refused__flash" onAnimationEnd={() => setRefused(null)} />
                      <span className="sq-refused__circle" />
                    </span>
                  )}
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
            dropSquareStyle: { boxShadow: HOVER_OUTLINE },
          }}
        />
      </div>
      {badge && (
        <div className="board-badge" style={corner(badge.square, flipped)} key={`${badge.square}-${badge.classification}-${fen}`}>
          <ClassificationIcon value={badge.classification} size={64} />
        </div>
      )}
      {endMarks?.map((m) => (
        <div className="board-badge" style={corner(m.square, flipped)} key={`${m.square}-${m.mark}-${fen}`}>
          <EndMarkIcon value={m.mark} size={64} />
        </div>
      ))}
      {promotion && <PromotionPicker promotion={promotion} flipped={flipped} />}
    </div>
  );
}
