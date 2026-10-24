import { Chessground } from "chessground";
import type { Api } from "chessground/api";
import type { Key } from "chessground/types";
import "chessground/assets/chessground.base.css";
import { memo, useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import type { Classification } from "../api/generated/Classification";
import type { Side } from "../api/generated/Side";
import type { MoveTarget, PromotionPiece } from "../lib/chess";
import { ClassificationIcon, EndMarkIcon, type EndMark } from "./ClassificationIcon";
import { Piece } from "./pieces";
import { installBoardPieces } from "../lib/boardPieces";

installBoardPieces();

/** An arrow on the board, from the review or drawn by the reader with the right button. */
export interface Arrow {
  startSquare: string;
  endSquare: string;
  color: string;
}

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

const FILES = "abcdefgh";
const NO_MARKS: string[] = [];
const NO_ARROWS: Arrow[] = [];

/** Position of a square's top-left corner, in squares from the board's top-left corner. */
function topLeft(square: string, flipped: boolean): { col: number; row: number } {
  const file = square.charCodeAt(0) - 97;
  const rank = Number(square[1]);
  return { col: flipped ? 7 - file : file, row: flipped ? rank - 1 : 8 - rank };
}

/** The square at a column and row from the board's top-left corner. */
function squareAt(col: number, row: number, flipped: boolean): string {
  return `${FILES[flipped ? 7 - col : col]}${flipped ? row + 1 : 8 - row}`;
}

/** Position of a square's top-right corner, in % of the board, for the badge overlay. */
function corner(square: string, flipped: boolean): { left: string; top: string } {
  const { col, row } = topLeft(square, flipped);
  return { left: `${(col + 1) * 12.5}%`, top: `${row * 12.5}%` };
}

const NOTATION: CSSProperties = { fontSize: "clamp(10px, 2.05cqw, 17px)", fontWeight: 600, lineHeight: 1 };
/** A mouse or pen rather than a finger. */
const FINE_POINTER = typeof window !== "undefined" && !!window.matchMedia?.("(pointer: fine)").matches;
const PROMOTION_PIECES: PromotionPiece[] = ["q", "n", "r", "b"];
const PROMOTION_NAMES: Record<PromotionPiece, string> = { q: "Queen", n: "Knight", r: "Rook", b: "Bishop" };
/** Arrows the reader draws: react-chessboard's colours, by the modifier key held. */
const DRAWN_ARROW = { default: "#ffaa00", shift: "#4caf50", ctrl: "#f44336", alt: "#9c27b0", meta: "#fbbf24" };

function drawnArrowColor(e: MouseEvent): string {
  if (e.altKey) return DRAWN_ARROW.alt;
  if (e.shiftKey) return DRAWN_ARROW.shift;
  if (e.ctrlKey) return DRAWN_ARROW.ctrl;
  if (e.metaKey) return DRAWN_ARROW.meta;
  return DRAWN_ARROW.default;
}

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
 * The squares under the pieces: their colours, the review's highlights, the right-click marks and the
 * coordinates. They change with the position, never on a press.
 */
const Squares = memo(function Squares({
  flipped,
  squareStyles,
  marks,
}: {
  flipped: boolean;
  squareStyles: Record<string, CSSProperties>;
  marks: string[];
}) {
  const squares = [];
  for (let row = 0; row < 8; row++) {
    for (let col = 0; col < 8; col++) {
      const square = squareAt(col, row, flipped);
      const dark = (square.charCodeAt(0) - 97 + Number(square[1])) % 2 === 1;
      const ink = { color: dark ? "var(--board-light)" : "var(--board-dark)" };
      squares.push(
        <div key={square} className={`board-square board-square--${dark ? "dark" : "light"}`} data-square={square}>
          {row === 7 && (
            <span className="board-coord" style={{ ...NOTATION, ...ink, right: "5%", bottom: "4%" }}>
              {square[0]}
            </span>
          )}
          {col === 0 && (
            <span className="board-coord" style={{ ...NOTATION, ...ink, left: "5%", top: "4%" }}>
              {square[1]}
            </span>
          )}
          <div className="sq" style={squareStyles[square]}>
            {marks.includes(square) && <span className="sq-mark" aria-hidden="true" />}
          </div>
        </div>,
      );
    }
  }
  return <div className="board-squares">{squares}</div>;
});

/** The arrows: react-chessboard's drawing (a fifth of a square wide, knights' arrows bent), which the board had before. */
const Arrows = memo(function Arrows({ arrows, flipped, drawing }: { arrows: Arrow[]; flipped: boolean; drawing: Arrow | null }) {
  const size = 2048;
  const sq = size / 8;
  const all = drawing ? [...arrows, drawing] : arrows;
  const centre = (square: string) => {
    const { col, row } = topLeft(square, flipped);
    return { x: col * sq + sq / 2, y: row * sq + sq / 2 };
  };
  return (
    <svg className="board-arrows" viewBox={`0 0 ${size} ${size}`} aria-hidden="true">
      {all.map((arrow, i) => {
        const active = arrow === drawing;
        const from = centre(arrow.startSquare);
        const to = centre(arrow.endSquare);
        // Arrows into the same square stop shorter, so their heads do not overlap.
        const shared = !active && all.some((a) => a.startSquare !== arrow.startSquare && a.endSquare === arrow.endSquare);
        const reduce = sq / (shared ? 4 : 8);
        const dx = to.x - from.x;
        const dy = to.y - from.y;
        const r = Math.hypot(dx, dy);
        let d: string;
        if (Math.abs(r - Math.hypot(1, 2) * sq) < 1) {
          // A knight's move: along the longer leg, then one square across.
          const vertical = Math.abs(dx) < Math.abs(dy);
          const turn = vertical ? { x: from.x, y: to.y } : { x: to.x, y: from.y };
          const end = { x: turn.x + ((to.x - turn.x) * (sq - reduce)) / sq, y: turn.y + ((to.y - turn.y) * (sq - reduce)) / sq };
          d = `M${from.x},${from.y} L${turn.x},${turn.y} L${end.x},${end.y}`;
        } else {
          d = `M${from.x},${from.y} L${from.x + (dx * (r - reduce)) / r},${from.y + (dy * (r - reduce)) / r}`;
        }
        const id = `board-arrowhead-${i}`;
        return (
          <g key={`${arrow.startSquare}${arrow.endSquare}${active ? "-active" : ""}`}>
            <marker id={id} markerWidth="2" markerHeight="2.5" refX="1.25" refY="1.25" orient="auto">
              <polygon points="0.3 0, 2 1.25, 0.3 2.5" fill={arrow.color} />
            </marker>
            <path
              d={d}
              fill="none"
              stroke={arrow.color}
              opacity={active ? 0.5 : 0.65}
              strokeWidth={(active ? 0.9 : 1) * (sq / 5)}
              markerEnd={`url(#${id})`}
            />
          </g>
        );
      })}
    </svg>
  );
});

/**
 * What a picked-up piece shows, drawn straight into chessground's board so that a press costs no
 * React render: the selection circle and the held outline under the pieces, the move hints and the
 * refused-move flash over them.
 */
class Layers {
  private under = document.createElement("div");
  private over = document.createElement("div");
  private held = document.createElement("span");
  private refused: HTMLElement | null = null;
  private selection: HTMLElement[] = [];
  flipped = false;
  /** The square the selection shows. */
  selected: string | null = null;

  constructor() {
    this.under.className = "board-layer board-layer--under";
    this.over.className = "board-layer board-layer--over";
    this.held.className = "sq-held";
    this.held.hidden = true;
    this.under.appendChild(this.held);
  }

  /** Puts the layers in chessground's board element (again after it rebuilt its DOM). */
  attach(board: HTMLElement) {
    board.append(this.under, this.over);
  }

  private place(el: HTMLElement, square: string) {
    const { col, row } = topLeft(square, this.flipped);
    el.style.left = `${col * 12.5}%`;
    el.style.top = `${row * 12.5}%`;
    el.dataset.square = square;
  }

  private span(className: string, square: string, parent: HTMLElement) {
    const el = document.createElement("span");
    el.className = className;
    this.place(el, square);
    parent.appendChild(el);
    return el;
  }

  /** Shows a picked-up piece (a new circle, so it grows again) and its moves; null clears them. */
  select(square: string | null, targets: MoveTarget[]) {
    if (square === this.selected) return;
    for (const el of this.selection) el.remove();
    this.selection = [];
    this.selected = square;
    if (!square) return;
    const circle = this.span("sq-select", square, this.under);
    circle.appendChild(document.createElement("span"));
    this.selection.push(circle);
    for (const t of targets) this.selection.push(this.span(`sq-hint${t.capture ? " sq-hint--capture" : ""}`, t.square, this.over));
  }

  /** The outline of the square under a held piece, or none. */
  hold(square: string | null) {
    if (square) this.place(this.held, square);
    this.held.hidden = !square;
  }

  /** Flashes a square red, as a refused move. */
  refuse(square: string) {
    this.refused?.remove();
    const el = this.span("sq-refused", square, this.over);
    const flash = document.createElement("span");
    flash.className = "sq-refused__flash";
    const circle = document.createElement("span");
    circle.className = "sq-refused__circle";
    el.append(flash, circle);
    flash.addEventListener("animationend", () => {
      el.remove();
      if (this.refused === el) this.refused = null;
    });
    this.refused = el;
  }

  /** Turns the layers with the board, which puts the picked-up piece down. */
  reorient(flipped: boolean) {
    this.flipped = flipped;
    this.select(null, []);
    this.hold(null);
  }
}

/** The pointer position of a mouse or touch event (none for a touchend). */
function eventPosition(e: MouseEvent | TouchEvent): [number, number] | undefined {
  if ("clientX" in e) return [e.clientX, e.clientY];
  const touch = e.targetTouches?.[0];
  return touch ? [touch.clientX, touch.clientY] : undefined;
}

/**
 * The board. Positions and annotations come from the backend, except in sidelines the reader plays
 * (docs/explore-mode.md §5.1); pieces can be moved only when `moves` is given, by dragging or by
 * clicking the piece and then a square. As on Chess.com, any piece can be picked up, whichever side
 * is to move: it shows a growing highlight on its square, and, if it can move, dots on free squares
 * and rings on pieces it can take. A move the rules forbid flashes its square red and is not played.
 *
 * The pieces are chessground's (Lichess's board, docs/explore-mode.md §5.2): it moves them in the DOM
 * itself, so picking a piece up renders nothing in React.
 */
export function BoardView({ fen, flipped, arrows, squareStyles, badge, endMarks, moves, promotion }: Props) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const boardRef = useRef<HTMLDivElement>(null);
  const cg = useRef<Api | null>(null);
  const layers = useRef<Layers | null>(null);
  // The latest props, for chessground's callbacks and the board's own listeners.
  const props = useRef({ fen, flipped, moves, promotion });
  useLayoutEffect(() => {
    props.current = { fen, flipped, moves, promotion };
  });
  const movable = !!moves && !promotion;

  // The right-click marks and the arrows the reader draws belong to one position: a move or a step clears them.
  const [marked, setMarked] = useState<{ fen: string; squares: string[] }>({ fen, squares: [] });
  const marks = marked.fen === fen ? marked.squares : NO_MARKS;
  const [drawn, setDrawn] = useState<{ fen: string; arrows: Arrow[] }>({ fen, arrows: [] });
  const drawnArrows = drawn.fen === fen ? drawn.arrows : NO_ARROWS;
  const [drawing, setDrawing] = useState<Arrow | null>(null);
  const annotations = useRef({ marks, drawnArrows });
  useLayoutEffect(() => {
    annotations.current = { marks, drawnArrows };
  });

  useLayoutEffect(() => {
    const wrap = wrapRef.current!;
    const board = boardRef.current!;
    const own = new Layers();
    own.flipped = props.current.flipped;
    layers.current = own;
    const { fen: startFen, flipped: startFlipped } = props.current;
    const api = Chessground(wrap, {
      fen: startFen,
      orientation: startFlipped ? "black" : "white",
      coordinates: false,
      // Synthetic events too (the tests); nothing here needs to tell a person from a script.
      trustAllEvents: true,
      disableContextMenu: true,
      addDimensionsCssVarsTo: board,
      highlight: { lastMove: false, check: false },
      animation: { enabled: true, duration: 180 },
      // Any piece can be picked up, of either side; `dests` holds only the picked-up piece's moves.
      movable: { free: false, color: undefined, showDests: false, rookCastle: true, events: { after: (orig, dest) => played(orig, dest) } },
      premovable: { enabled: false },
      // With a mouse the piece lifts on the press, centred under the pointer wherever the square
      // was pressed, as on Lichess. Touch waits for a move, so a tap stays a tap.
      draggable: { enabled: true, distance: FINE_POINTER ? 0 : 1, showGhost: true },
      selectable: { enabled: true },
      drawable: { enabled: false, visible: false },
      events: { insert: (elements) => own.attach(elements.board) },
    });
    cg.current = api;
    const s = api.state;

    /** Chessground played a legal move; the review decides whether it stands. */
    const played = (orig: Key, dest: Key) => {
      const { moves: current, fen: shown } = props.current;
      if (current?.onDrop(orig, dest)) return;
      // Taken back (a promotion to choose, a wrong retry): the piece snaps back.
      api.set({ animation: { enabled: false } });
      api.set({ fen: shown });
      api.set({ animation: { enabled: true } });
    };

    /** Shows what chessground has picked up, and gives it the picked-up piece's moves. */
    const sync = () => {
      const selected = s.selected ?? null;
      if (selected !== own.selected) {
        const current = props.current.moves;
        const targets = selected && current ? current.targets(selected) : [];
        own.select(selected, targets);
        if (selected && current) {
          const dests = targets.map((t) => t.square as Key);
          // A king can also be dropped on its own rook, which castles.
          if (s.pieces.get(selected)?.role === "king") {
            for (const [key, p] of s.pieces) {
              if (p.role === "rook" && p.color === s.pieces.get(selected)!.color && !dests.includes(key) && current.legal(selected, key)) {
                dests.push(key);
              }
            }
          }
          s.movable.dests = new Map([[selected, dests]]);
        } else s.movable.dests = undefined;
      }
    };
    const canMove = (orig: string, dest: string) => !!s.movable.dests?.get(orig as Key)?.includes(dest as Key);
    const keyAt = (pos: [number, number] | undefined) => (pos ? (api.getKeyAtDomPos(pos) ?? null) : null);

    // The press: chessground picks up, moves or puts down on its own (on cg-board); these run just
    // before it (capture) and just after it (bubble) to add Chess.com's rules.
    let rightFrom: string | null = null;
    const onPressBefore = (e: MouseEvent | TouchEvent) => {
      // The board may have moved on the page since chessground last measured it.
      s.dom.bounds.clear();
      const square = keyAt(eventPosition(e));
      if (!square) return;
      if ("button" in e && e.button === 2) {
        rightFrom = square;
        return;
      }
      if ("button" in e && e.button !== 0) return;
      const { marks: m, drawnArrows: a } = annotations.current;
      if (m.length) setMarked({ fen: props.current.fen, squares: [] });
      if (a.length) setDrawn({ fen: props.current.fen, arrows: [] });
      // An empty square the picked-up piece cannot reach puts it down, flashing red.
      const selected = s.selected;
      if (selected && square !== selected && !s.pieces.has(square as Key) && !canMove(selected, square)) own.refuse(square);
    };
    const onPressAfter = (e: MouseEvent | TouchEvent) => {
      // The right button puts the picked-up piece down.
      if ("button" in e && e.button === 2) api.selectSquare(null);
      sync();
      const held = s.draggable.current;
      own.hold(held && !held.newPiece ? held.orig : null);
    };

    // The release. Chessground plays a legal drop itself; the rest is Chess.com's: a refused drop
    // flashes and the piece goes back still picked up, a drop off the board just goes back, and a
    // drop on its own square counts as a click (it puts down a piece picked up before this press).
    let keep: string | null | undefined;
    const onReleaseBefore = (e: MouseEvent | TouchEvent) => {
      keep = undefined;
      const cur = s.draggable.current;
      if (!cur) return;
      const dest = keyAt(eventPosition(e) ?? cur.pos);
      if (dest && cur.started && dest !== cur.orig) {
        if (canMove(cur.orig, dest)) return;
        own.refuse(dest);
        keep = cur.orig;
      } else if (!dest) keep = cur.orig;
      else keep = cur.previouslySelected === cur.orig ? null : cur.orig;
    };
    const onReleaseAfter = (e: MouseEvent | TouchEvent) => {
      if ("button" in e && e.button === 2) {
        if (!rightFrom) return;
        const to = keyAt(eventPosition(e));
        if (to) {
          const from = rightFrom;
          const shown = props.current.fen;
          if (from === to) {
            const m = annotations.current.marks;
            setMarked({ fen: shown, squares: m.includes(to) ? m.filter((q) => q !== to) : [...m, to] });
          } else {
            const color = drawnArrowColor(e as MouseEvent);
            const a = annotations.current.drawnArrows;
            const same = (x: Arrow) => x.startSquare === from && x.endSquare === to;
            setDrawn({ fen: shown, arrows: a.some(same) ? a.filter((x) => !same(x)) : [...a, { startSquare: from, endSquare: to, color }] });
          }
        }
        rightFrom = null;
        setDrawing(null);
        return;
      }
      if (keep !== undefined) {
        if (keep) api.selectSquare(keep as Key);
        else api.selectSquare(null);
        keep = undefined;
      }
      own.hold(null);
      sync();
    };
    // While a piece is held, its outline follows the square under it; a right-drag draws an arrow.
    const onMove = (e: MouseEvent | TouchEvent) => {
      if (rightFrom) {
        const to = keyAt(eventPosition(e));
        const from = rightFrom;
        setDrawing((d) => {
          if (!to || to === from) return null;
          if (d?.endSquare === to) return d;
          return { startSquare: from, endSquare: to, color: drawnArrowColor(e as MouseEvent) };
        });
        return;
      }
      const cur = s.draggable.current;
      if (!cur?.started) return;
      own.hold(keyAt(eventPosition(e)));
    };

    board.addEventListener("mousedown", onPressBefore, true);
    board.addEventListener("touchstart", onPressBefore, { capture: true, passive: true });
    board.addEventListener("mousedown", onPressAfter);
    board.addEventListener("touchstart", onPressAfter, { passive: true });
    window.addEventListener("mouseup", onReleaseBefore, true);
    window.addEventListener("touchend", onReleaseBefore, true);
    window.addEventListener("mouseup", onReleaseAfter);
    window.addEventListener("touchend", onReleaseAfter);
    window.addEventListener("mousemove", onMove, { passive: true });
    window.addEventListener("touchmove", onMove, { passive: true });
    return () => {
      board.removeEventListener("mousedown", onPressBefore, true);
      board.removeEventListener("touchstart", onPressBefore, true);
      board.removeEventListener("mousedown", onPressAfter);
      board.removeEventListener("touchstart", onPressAfter);
      window.removeEventListener("mouseup", onReleaseBefore, true);
      window.removeEventListener("touchend", onReleaseBefore, true);
      window.removeEventListener("mouseup", onReleaseAfter);
      window.removeEventListener("touchend", onReleaseAfter);
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("touchmove", onMove);
      api.destroy();
      cg.current = null;
    };
  }, []);

  // A new position glides the pieces there and puts down a picked-up piece.
  useLayoutEffect(() => {
    const api = cg.current!;
    api.cancelMove();
    api.set({ fen });
    layers.current!.select(null, []);
    layers.current!.hold(null);
  }, [fen]);

  useLayoutEffect(() => {
    const api = cg.current!;
    api.set({ orientation: flipped ? "black" : "white" });
    layers.current!.reorient(flipped);
  }, [flipped]);

  useLayoutEffect(() => {
    const api = cg.current!;
    if (!movable) {
      api.cancelMove();
      layers.current!.select(null, []);
      layers.current!.hold(null);
    }
    api.set({ movable: { color: movable ? "both" : undefined } });
  }, [movable]);

  const shownArrows = drawnArrows.length
    ? [...arrows, ...drawnArrows.filter((d) => !arrows.some((a) => a.startSquare === d.startSquare && a.endSquare === d.endSquare))]
    : arrows;

  return (
    <div className="board-wrap">
      <div className={`board ${movable ? "board--movable" : ""}`} ref={boardRef}>
        <Squares flipped={flipped} squareStyles={squareStyles} marks={marks} />
        <div className="cg-wrap" ref={wrapRef} />
        <Arrows arrows={shownArrows} flipped={flipped} drawing={drawing} />
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
