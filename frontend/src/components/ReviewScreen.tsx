import { useCallback, useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import type { Arrow } from "react-chessboard";
import type { ArrowKind } from "../api/generated/ArrowKind";
import type { Eval } from "../api/generated/Eval";
import type { MoveRef } from "../api/generated/MoveRef";
import type { ReviewResult } from "../api/generated/ReviewResult";
import type { Sentence } from "../api/generated/Sentence";
import type { Side } from "../api/generated/Side";
import { useLiveEngine } from "../hooks/useLiveEngine";
import { useSidelineJudge } from "../hooks/useSidelineJudge";
import { canMoveFrom, ending, needsPromotion, playMove, type Ending, type PlayedMove, type PromotionPiece } from "../lib/chess";
import { LEARNABLE, coachArrows, coachMessage, coachMood, playerName } from "../lib/coach";
import { play, stepSideline, type BoardState, type Sideline } from "../lib/explore";
import { kingSquareToMove, uciSquares } from "../lib/fen";
import { CLASSIFICATIONS, formatEvalPill, moveNumberLabel } from "../lib/format";
import { materialFromFen } from "../lib/material";
import { expectedPointsFor, hintLadder, hintStep, judgeRetry, type RetryAnswer } from "../lib/retry";
import { FILTERS, fenAt, nextPlyAt, plyAt, stepFiltered, type MoveFilter } from "../lib/review";
import { moveSound, navigationSounds, playSounds, retrySound, sanFlags, sidelineSounds } from "../lib/sound";
import { BoardView, type BoardBadge, type BoardMoves, type BoardPromotion } from "./BoardView";
import { ClassificationIcon } from "./ClassificationIcon";
import {
  CoachActions,
  CoachBubble,
  CoachSentence,
  EvalPill,
  LiveLines,
  MoveDetails,
  MoveVerdict,
  ShowLineMoves,
  type CoachAction,
  type FragmentMarking,
  type FragmentRef,
  type LinePreview,
} from "./Coach";
import { EvalBar } from "./EvalBar";
import { Icons } from "./icons";
import { LogoMark } from "./Logo";
import { EvalGraph } from "./EvalGraph";
import { MoveList } from "./MoveList";
import { San } from "./PieceIcon";
import { PlayerStrip } from "./PlayerStrip";
import { ReportView } from "./ReportView";

interface Props {
  review: ReviewResult;
  /** The review's id on the server (sideline moves are judged against it). */
  reviewId: string;
  onNewGame: () => void;
}

type Mode = "report" | "review";

interface Retry {
  /** Ply whose position is being retried. */
  ply: number;
  status: "trying" | "wrong" | "correct" | "revealed";
  /** SAN of the last wrong attempt when the engine lines know it. */
  attempt: string | null;
  /** Rungs of the hint ladder shown so far. */
  hint: number;
  /** The accepted move once solved: the best move, or one close enough to it. */
  answer: RetryAnswer | null;
}

/** A marked sentence fragment, tied to the move whose sentence it is. */
type Mark = FragmentRef & { ply: number };

const BEST_ARROW = "rgba(129, 182, 76, 0.9)";
const SUGGEST_ARROW = "rgba(92, 139, 176, 0.85)";
const PREVIEW_TINT = "rgba(255, 196, 64, 0.5)";
const HINT_SQUARE: CSSProperties = { boxShadow: "inset 0 0 0 4px rgba(255, 196, 64, 0.95)", background: "rgba(255, 196, 64, 0.25)" };
const MARK_SQUARE: CSSProperties = { boxShadow: "inset 0 0 0 4px rgba(250, 65, 45, 0.8)", background: "rgba(250, 65, 45, 0.2)" };
const NO_SENTENCES: Sentence[] = [];
const CHECK_GLOW: CSSProperties = {
  backgroundImage: "radial-gradient(circle, rgba(235,40,40,.9) 0%, rgba(235,40,40,.4) 45%, transparent 72%)",
};
const ENDINGS: Record<Ending, string> = {
  checkmate: "Checkmate.",
  stalemate: "Stalemate: a draw.",
  insufficient_material: "Neither side can mate: a draw.",
  fifty_moves: "Fifty moves without a capture or pawn move: either side can claim a draw.",
};
/** Pause between moves when a coach line plays itself. */
const LINE_STEP_MS = 1400;
const COACH_ARROW: Record<ArrowKind, string> = {
  move: "rgba(255, 164, 89, 0.9)",
  attack: "rgba(250, 65, 45, 0.85)",
  defence: SUGGEST_ARROW,
  best: BEST_ARROW,
};

/** Input types that take no text, so the board keys still apply while one has focus. */
const KEYLESS_INPUTS = new Set(["checkbox", "radio", "button", "submit", "reset"]);

/**
 * Whether a key press belongs to a text field rather than the board. A focused switch or menu (after a
 * click on "Engine arrow" or "Step through") must not swallow ← →, or the arrows stop working.
 */
function isTyping(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable || target.tagName === "TEXTAREA") return true;
  return target instanceof HTMLInputElement && !KEYLESS_INPUTS.has(target.type);
}

function loadFlag(key: string, fallback: boolean): boolean {
  try {
    const v = localStorage.getItem(key);
    return v == null ? fallback : v === "1";
  } catch {
    return fallback;
  }
}

function saveFlag(key: string, value: boolean) {
  try {
    localStorage.setItem(key, value ? "1" : "0");
  } catch {
    // Storage may be unavailable (private mode); the setting just won't persist.
  }
}

function tint(color: string, pct: number): CSSProperties {
  return { background: `color-mix(in srgb, ${color} ${pct}%, transparent)` };
}

export function ReviewScreen({ review, reviewId, onNewGame }: Props) {
  const total = review.plies.length;
  const [mode, setMode] = useState<Mode>("report");
  const [ply, setPly] = useState(0);
  const [flipped, setFlipped] = useState(false);
  const [filter, setFilter] = useState<MoveFilter>("all");
  const [preview, setPreview] = useState<LinePreview | null>(null);
  const [retry, setRetry] = useState<Retry | null>(null);
  const [showSuggestion, setShowSuggestion] = useState(() => loadFlag("review.engineArrow", false));
  const [explain, setExplain] = useState(() => loadFlag("review.explain", false));
  const [soundOn, setSoundOn] = useState(() => loadFlag("review.sound", true));
  const [playing, setPlaying] = useState(false);
  const [linePlaying, setLinePlaying] = useState(false);
  const [hovered, setHovered] = useState<Mark | null>(null);
  const [pinned, setPinned] = useState<Mark | null>(null);
  const [sideline, setSideline] = useState<Sideline | null>(null);
  /** A pawn move waiting for its promotion piece. */
  const [promotion, setPromotion] = useState<{ from: string; to: string } | null>(null);

  // Every game navigation leaves the sideline.
  const go = useCallback(
    (target: number) => {
      setPreview(null);
      setLinePlaying(false);
      setRetry(null);
      setPinned(null);
      setHovered(null);
      setSideline(null);
      setPromotion(null);
      setPly(Math.max(0, Math.min(total, target)));
    },
    [total],
  );

  const current = plyAt(review, ply);
  const next = nextPlyAt(review, ply);
  // The sideline move shown, when the board is inside the sideline rather than at its branch.
  const shownMove = sideline && sideline.cursor > 0 ? sideline.moves[sideline.cursor - 1] : undefined;
  const shownEnd = shownMove ? ending(shownMove.fen) : null;
  // A finished game needs no engine; a claimable fifty-move draw is still played on.
  const shownOver = !!shownEnd && shownEnd !== "fifty_moves";
  // The position the shown sideline move was played from: the branch (a game position) or the
  // sideline move before it.
  const shownFrom =
    sideline && shownMove
      ? sideline.cursor > 1
        ? sideline.moves[sideline.cursor - 2]!.fen
        : fenAt(review, sideline.fromPly)
      : null;
  // The browser engine searches sideline positions only, and the position each sideline move was
  // played from, so the move can be judged on one engine's lines (docs/explore-mode.md §5.4–5.5).
  // The coach's threat searches, as the review made them (its config), let the move be explained too.
  const coachSearches = useMemo(
    () =>
      shownMove
        ? {
            playedUci: shownMove.uci,
            depth: review.config.coach.threat_search_depth,
            best: review.config.coach.best_threat_search,
          }
        : null,
    [shownMove, review.config.coach.threat_search_depth, review.config.coach.best_threat_search],
  );
  const live = useLiveEngine(
    shownMove && !shownOver ? shownMove.fen : null,
    shownFrom,
    review,
    review.game.chess960,
    coachSearches,
  );
  const liveLines = live.analysis?.lines ?? [];
  const judgements = useSidelineJudge(reviewId, sideline, live, shownOver);
  const shownJudged = sideline && shownMove ? judgements[sideline.cursor - 1] : undefined;
  const shownClass = shownJudged?.judgement.classification ?? null;
  const shownMover: Side = shownFrom?.split(" ")[1] === "b" ? "black" : "white";
  // The coach's note for the shown sideline move (docs/explore-mode.md §5.5).
  const shownSentences = shownJudged?.judgement.coach.sentences ?? NO_SENTENCES;
  const liveBest = liveLines[0]?.moves[0];

  const showLine = preview?.source === "coach" ? current?.coach?.show_lines[preview.line] : undefined;
  const previewLine: MoveRef[] | undefined = useMemo(() => {
    if (!preview) return undefined;
    if (preview.source === "coach") return showLine?.moves;
    return (preview.source === "alternatives" ? current?.lines : next?.lines)?.[preview.line]?.moves;
  }, [preview, showLine, current, next]);
  const previewMove = preview && preview.move >= 0 ? previewLine?.[preview.move] : undefined;
  const previewing = !!preview && !!previewLine && (!!previewMove || !!showLine);

  const openShowLine = (index: number) => {
    setPlaying(false);
    setRetry(null);
    setPreview({ source: "coach", line: index, move: -1 });
    setLinePlaying(true);
  };
  const closePreview = () => {
    setPreview(null);
    setLinePlaying(false);
  };

  // Sentence fragments marked on the board (only those of the move shown).
  const ofCurrent = (m: Mark | null): FragmentRef | null => (m && current && m.ply === current.ply ? m : null);
  const marking: FragmentMarking | undefined = current
    ? {
        active: ofCurrent(hovered) ?? ofCurrent(pinned),
        pinned: ofCurrent(pinned),
        onHover: (ref) => setHovered(ref ? { ...ref, ply: current.ply } : null),
        onToggle: (ref) =>
          setPinned((p) => (p && p.ply === current.ply && p.sentence === ref.sentence && p.fragment === ref.fragment ? null : { ...ref, ply: current.ply })),
      }
    : undefined;
  const markedFragment = marking?.active
    ? current?.coach?.sentences[marking.active.sentence]?.fragments[marking.active.fragment]
    : undefined;

  const step = useCallback(
    (dir: 1 | -1) => {
      if (filter === "all") return go(ply + dir);
      const target = stepFiltered(review, ply, dir, filter);
      if (target !== null) go(target);
    },
    [filter, go, ply, review],
  );

  /** Moves the board to sideline position `cursor`, with the sideline's sounds. */
  const showSideline = useCallback(
    (cursor: number) => {
      if (!sideline || cursor === sideline.cursor) return;
      if (soundOn) playSounds(sidelineSounds(sideline.moves, sideline.cursor, cursor));
      setPromotion(null);
      setSideline({ ...sideline, cursor });
    },
    [sideline, soundOn],
  );

  /**
   * ← / → and the nav buttons. While a sideline is open they walk it; back from its branch position
   * leaves it for the game, and forward past its end does nothing.
   */
  const stepBoard = useCallback(
    (dir: 1 | -1) => {
      if (sideline) {
        const next = stepSideline(sideline, dir);
        if (next) return showSideline(next.cursor);
        if (dir === 1) return;
      }
      step(dir);
    },
    [sideline, showSideline, step],
  );

  const advance = useCallback(() => {
    if (ply >= total) {
      setMode("report");
      return;
    }
    step(1);
  }, [ply, total, step]);

  // Autoplay through the game.
  useEffect(() => {
    if (!playing) return;
    if (ply >= total) {
      setPlaying(false);
      return;
    }
    const t = setTimeout(() => go(ply + 1), 1000);
    return () => clearTimeout(t);
  }, [playing, ply, total, go]);

  const stepPreview = useCallback(
    (dir: 1 | -1) => {
      if (!preview || !previewLine) return;
      const m = preview.move + dir;
      // A coach line starts at its start position (-1) and stays open until hidden.
      const first = preview.source === "coach" ? -1 : 0;
      if (m < first) {
        if (preview.source !== "coach") setPreview(null);
      } else if (m < previewLine.length) setPreview({ ...preview, move: m });
    },
    [preview, previewLine],
  );

  // A coach line plays itself move by move until paused or finished.
  const lineMove = preview?.source === "coach" ? preview.move : null;
  const lineLength = showLine?.moves.length ?? 0;
  useEffect(() => {
    if (!linePlaying || lineMove === null) return;
    if (lineMove + 1 >= lineLength) {
      setLinePlaying(false);
      return;
    }
    const t = setTimeout(() => setPreview((p) => (p && p.source === "coach" ? { ...p, move: p.move + 1 } : p)), lineMove < 0 ? 700 : LINE_STEP_MS);
    return () => clearTimeout(t);
  }, [linePlaying, lineMove, lineLength]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (isTyping(e.target) || e.altKey || e.ctrlKey || e.metaKey) return;
      if (preview && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
        setLinePlaying(false);
        stepPreview(e.key === "ArrowRight" ? 1 : -1);
      } else if (e.key === "ArrowLeft") {
        setMode("review");
        stepBoard(-1);
      } else if (e.key === "ArrowRight") {
        setMode("review");
        stepBoard(1);
      } else if (e.key === "Home" || e.key === "End") {
        setMode("review");
        go(e.key === "Home" ? 0 : total);
      } else if (e.key === "f") setFlipped((f) => !f);
      else if (e.key === "Escape") {
        // A promotion choice is taken back first, then the sideline is left for its branch position.
        if (promotion) setPromotion(null);
        else if (sideline) go(sideline.fromPly);
        else {
          setPreview(null);
          setLinePlaying(false);
          setRetry(null);
          setPinned(null);
        }
      } else return;
      e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [go, stepBoard, stepPreview, preview, promotion, sideline, total]);

  // Board sounds when the game moves (rules in docs/sound.md).
  const lastPly = useRef(ply);
  useEffect(() => {
    const from = lastPly.current;
    lastPly.current = ply;
    // Older servers leave player_side out when no side was chosen.
    if (soundOn) playSounds(navigationSounds(review.plies, from, ply, review.settings.player_side ?? null));
  }, [ply, review, soundOn]);

  const lastPreview = useRef(previewMove);
  useEffect(() => {
    const before = lastPreview.current;
    lastPreview.current = previewMove;
    if (soundOn && previewMove && previewMove !== before) playSounds([moveSound(sanFlags(previewMove.san))]);
  }, [previewMove, soundOn]);

  const canRetry = !!current?.best_move && current.show_best_move && !current.played_is_best;
  const activeRetry = retry && current && retry.ply === current.ply && current.best_move ? retry : null;
  const bestMove = current?.best_move ?? null;
  const retrySolved = activeRetry && (activeRetry.status === "correct" || activeRetry.status === "revealed");

  const showBest = () => {
    if (!current) return;
    const i = current.lines.findIndex((l) => l.moves[0]?.uci === current.best_move?.uci);
    setRetry(null);
    setPreview(i >= 0 ? { source: "alternatives", line: i, move: 0 } : null);
  };

  const ladder = useMemo(() => (current ? hintLadder(current) : []), [current]);

  const onRetryDrop = (from: string, to: string): boolean => {
    if (!current?.best_move || !activeRetry) return false;
    const answer = judgeRetry(current, review.rating_used, from, to);
    if (answer) {
      if (soundOn) playSounds([moveSound(sanFlags(answer.move.san)), retrySound(answer.best ? "best" : "excellent")]);
      setRetry({ ...activeRetry, status: "correct", attempt: null, answer });
      return true;
    }
    // Not observed in the reference client, which sounds only the finished retry.
    if (soundOn) playSounds(["incorrect"]);
    const uci = `${from}${to}`;
    const known = current.lines.find((l) => l.moves[0]?.uci.startsWith(uci))?.moves[0]?.san ?? null;
    setRetry({ ...activeRetry, status: "wrong", attempt: known ?? (current.uci.startsWith(uci) ? current.san : null) });
    return false;
  };

  /* ---------------------------------------------------------------- board */

  // The move a solved retry shows: the one found, or the best move when revealed.
  const solvedMove = retrySolved ? (activeRetry?.answer?.move ?? bestMove) : null;

  const displayFen = activeRetry
    ? (solvedMove?.fen ?? current?.fen_before ?? fenAt(review, ply))
    : (shownMove?.fen ?? previewMove?.fen ?? (showLine && previewing ? showLine.start_fen : fenAt(review, ply)));
  const toMove: Side = displayFen.split(" ")[1] === "b" ? "black" : "white";

  /**
   * Plays moves from the shown position, one after another: the game's own move follows the game,
   * any other one explores. The last move played sounds.
   */
  const playMoves = (moves: PlayedMove[]) => {
    const last = moves.at(-1);
    if (!last) return;
    setPlaying(false);
    setMode("review");
    const after = moves.reduce<BoardState>((s, m) => play(s, review.plies, m), { ply, sideline });
    if (!after.sideline) {
      go(after.ply);
      return;
    }
    setPinned(null);
    setHovered(null);
    setSideline(after.sideline);
    if (soundOn) playSounds([moveSound(last.flags)]);
  };

  /** Plays a dragged move from the shown position; false leaves the piece where it was. */
  const exploreMove = (from: string, to: string, piece?: PromotionPiece): boolean => {
    const move = playMove(displayFen, from, to, piece, review.game.chess960);
    if (move) playMoves([move]);
    return !!move;
  };

  const onExploreDrop = (from: string, to: string): boolean => {
    if (needsPromotion(displayFen, from, to)) {
      setPromotion({ from, to });
      return false;
    }
    return exploreMove(from, to);
  };

  const { arrows, squareStyles, badge } = useMemo(() => {
    const arrows: Arrow[] = [];
    const styles: Record<string, CSSProperties> = {};
    let badge: BoardBadge | null = null;

    if (activeRetry) {
      const sq = solvedMove ? uciSquares(solvedMove.uci) : null;
      if (sq) {
        const best = solvedMove?.uci === bestMove?.uci;
        const color = CLASSIFICATIONS[best ? "best" : "excellent"].color;
        styles[sq.from] = tint(color, 45);
        styles[sq.to] = tint(color, 60);
        badge = { square: sq.to, classification: best ? "best" : "excellent" };
      } else {
        // Hint ladder: the squares of every area rung shown so far.
        for (const rung of ladder.slice(0, activeRetry.hint)) {
          if (rung.kind === "area") for (const s of rung.squares) styles[s] = HINT_SQUARE;
        }
      }
      return { arrows, squareStyles: styles, badge };
    }

    if (shownMove) {
      // Coloured by its classification once judged, as game moves are.
      const color = shownClass ? CLASSIFICATIONS[shownClass].color : PREVIEW_TINT;
      styles[shownMove.from] = tint(color, shownClass ? 42 : 45);
      styles[shownMove.to] = tint(color, shownClass ? 58 : 65);
      if (shownClass) badge = { square: shownMove.to, classification: shownClass };
      const king = shownMove.flags.check ? kingSquareToMove(shownMove.fen) : null;
      if (king) styles[king] = { ...styles[king], ...CHECK_GLOW };
      // The engine arrow shows the live engine's top move.
      if (showSuggestion && liveBest) arrows.push({ startSquare: liveBest.from, endSquare: liveBest.to, color: SUGGEST_ARROW });
      // What the coach's main sentence is about, as for game moves.
      for (const a of shownSentences[0]?.fragments.find((f) => f.arrows.length > 0)?.arrows ?? []) {
        if (!arrows.some((b) => b.startSquare === a.from && b.endSquare === a.to)) {
          arrows.push({ startSquare: a.from, endSquare: a.to, color: COACH_ARROW[a.kind] });
        }
      }
      return { arrows, squareStyles: styles, badge };
    }

    if (showLine && !previewMove) return { arrows, squareStyles: styles, badge };

    if (previewMove) {
      const sq = uciSquares(previewMove.uci);
      if (sq) {
        const isBest = preview?.source === "alternatives" && preview.move === 0 && previewMove.uci === current?.best_move?.uci;
        const color = isBest ? CLASSIFICATIONS.best.color : PREVIEW_TINT;
        styles[sq.from] = tint(color, 45);
        styles[sq.to] = tint(color, 65);
        if (isBest) badge = { square: sq.to, classification: "best" };
      }
      return { arrows, squareStyles: styles, badge };
    }

    if (current) {
      const color = current.classification ? CLASSIFICATIONS[current.classification].color : "var(--c-best)";
      if (current.from) styles[current.from] = tint(color, 42);
      styles[current.to] = tint(color, 58);
      if (current.classification) badge = { square: current.to, classification: current.classification };
      if (markedFragment) {
        // A fragment the reader is pointing at replaces the default arrows with its own.
        for (const a of markedFragment.arrows) {
          if (!arrows.some((b) => b.startSquare === a.from && b.endSquare === a.to)) {
            arrows.push({ startSquare: a.from, endSquare: a.to, color: COACH_ARROW[a.kind] });
          }
        }
        for (const s of markedFragment.squares) styles[s] = { ...styles[s], ...MARK_SQUARE };
        return { arrows, squareStyles: styles, badge };
      }
      if (current.show_best_move && current.best_move && !current.played_is_best) {
        const sq = uciSquares(current.best_move.uci);
        if (sq) arrows.push({ startSquare: sq.from, endSquare: sq.to, color: BEST_ARROW });
      }
      // What the coach's main sentence is about: the reply or idea and its targets.
      for (const a of coachArrows(current)) {
        if (!arrows.some((b) => b.startSquare === a.from && b.endSquare === a.to)) {
          arrows.push({ startSquare: a.from, endSquare: a.to, color: COACH_ARROW[a.kind] });
        }
      }
      if (current.flags.check) {
        const king = kingSquareToMove(current.fen_after);
        if (king) styles[king] = { ...styles[king], ...CHECK_GLOW };
      }
    }
    if (showSuggestion && next?.lines[0]?.moves[0]) {
      const sq = uciSquares(next.lines[0].moves[0].uci);
      if (sq && !arrows.some((a) => a.startSquare === sq.from && a.endSquare === sq.to)) {
        arrows.push({ startSquare: sq.from, endSquare: sq.to, color: SUGGEST_ARROW });
      }
    }
    return { arrows, squareStyles: styles, badge };
  }, [activeRetry, solvedMove, bestMove, ladder, shownMove, shownClass, shownSentences, liveBest, showLine, previewMove, preview, current, markedFragment, next, showSuggestion]);

  // Retry takes the mover's pieces; otherwise any legal move explores, except while a line plays.
  const boardMoves: BoardMoves | null = activeRetry
    ? current && !retrySolved
      ? { canDrag: (_square, side) => side === current.mover, onDrop: onRetryDrop }
      : null
    : preview
      ? null
      : { canDrag: (square) => canMoveFrom(displayFen, square), onDrop: onExploreDrop };
  const boardPromotion: BoardPromotion | null = promotion
    ? {
        square: promotion.to,
        side: toMove,
        onPick: (piece) => {
          setPromotion(null);
          if (piece) exploreMove(promotion.from, promotion.to, piece);
        },
      }
    : null;

  /** The bar in a sideline: the game's result when it is over, else the live engine's top line. */
  const sidelineBar: { eval: Eval; ep: number } | null = !shownMove
    ? null
    : shownOver
      ? shownEnd === "checkmate"
        ? { eval: { kind: "mate", winner: toMove === "white" ? "black" : "white", moves: 0 }, ep: toMove === "white" ? 0 : 1 }
        : { eval: { kind: "cp", cp: 0 }, ep: 0.5 }
      : liveLines[0]
        ? { eval: liveLines[0].eval, ep: expectedPointsFor(liveLines[0].eval, "white", review.rating_used) }
        : null;

  const material = materialFromFen(displayFen);
  const bottom: Side = flipped ? "black" : "white";
  const top: Side = flipped ? "white" : "black";
  const point = review.graph[ply];
  const strip = (side: Side) => (
    <PlayerStrip
      side={side}
      name={playerName(review, side)}
      elo={side === "white" ? review.white.elo : review.black.elo}
      material={material[side]}
      toMove={toMove === side}
    />
  );

  /* ---------------------------------------------------------------- coach */

  const msg = current ? coachMessage(review, current, next) : null;
  const learnable = !!current?.classification && LEARNABLE.has(current.classification);

  const coach = (() => {
    // Retry: find the best move yourself.
    if (activeRetry && current && bestMove) {
      const name = playerName(review, current.mover);
      const answer = activeRetry.status === "correct" ? activeRetry.answer : null;
      // An accepted alternative is judged by its own engine line.
      const solvedEval =
        answer && !answer.best ? (current.lines.find((l) => l.moves[0]?.uci === answer.move.uci)?.eval ?? null) : current.eval_best;
      const step = hintStep(activeRetry.hint, ladder);
      const reveal = () => setRetry({ ...activeRetry, status: "revealed" });
      const actions: CoachAction[] = retrySolved
        ? [
            { label: "Back", icon: Icons.back, onClick: () => setRetry(null) },
            { label: "Next", icon: Icons.next, onClick: advance, primary: true },
          ]
        : [
            {
              label: step.label,
              icon: step.reveal ? Icons.eye : Icons.bulb,
              onClick: step.reveal ? reveal : () => setRetry({ ...activeRetry, hint: activeRetry.hint + 1 }),
              pressed: activeRetry.hint > 0 && !step.reveal,
              hint: step.reveal ? "Show the answer" : "Get a hint without giving the move away",
            },
            ...(step.reveal ? [] : [{ label: "Show", icon: Icons.eye, onClick: reveal, hint: "Show the answer" }]),
            { label: "Cancel", onClick: () => setRetry(null) },
          ];
      const mood = activeRetry.status === "correct" ? "happy" : activeRetry.status === "wrong" ? "concerned" : "neutral";
      return (
        <>
          <CoachBubble mood={mood}>
            <div className="coach__head">
              {retrySolved ? (
                <ClassificationIcon value={answer && !answer.best ? "excellent" : "best"} size={26} />
              ) : (
                <span className="coach__glyph">{Icons.target}</span>
              )}
              <h2 className="coach__title">
                {answer && !answer.best ? (
                  <>
                    Good! <San san={answer.move.san} side={current.mover} /> works too
                  </>
                ) : answer ? (
                  <>
                    Correct! <San san={bestMove.san} side={current.mover} /> is best
                  </>
                ) : activeRetry.status === "revealed" ? (
                  <>
                    The best move was <San san={bestMove.san} side={current.mover} />
                  </>
                ) : (
                  <>Find the best move for {name}</>
                )}
              </h2>
              {retrySolved && solvedEval && <EvalPill value={solvedEval} />}
            </div>
            <div className="coach__body">
              {activeRetry.status === "trying" && (
                <p>
                  Instead of {moveNumberLabel(current.move_number, current.mover)} {current.san}, drag a{" "}
                  {current.mover === "white" ? "white" : "black"} piece to play the move you think is best.
                </p>
              )}
              {activeRetry.status === "wrong" && (
                <p className="retry-wrong">
                  {activeRetry.attempt ? `${activeRetry.attempt} isn't the best move here.` : "That's not the best move."} Try again
                  {activeRetry.hint > 0 ? "." : ", or ask for a hint."}
                </p>
              )}
              {!retrySolved &&
                ladder.slice(0, activeRetry.hint).map((rung) => (
                  <p key={rung.kind} className="retry-hint">
                    {Icons.bulb}
                    {rung.text}
                  </p>
                ))}
              {answer && !answer.best && (
                <p>
                  It is close enough to the engine's top move, <San san={bestMove.san} side={current.mover} />.
                </p>
              )}
              {retrySolved && solvedEval && (
                <p>
                  It keeps the evaluation at {formatEvalPill(solvedEval)} instead of {formatEvalPill(current.eval_after)}.
                </p>
              )}
            </div>
          </CoachBubble>
          <CoachActions actions={actions} />
        </>
      );
    }

    // Exploring a sideline the reader played on the board.
    if (sideline && shownMove) {
      const first = sideline.moves[0]!;
      const replaced = review.plies[sideline.fromPly];
      const branchMover: Side = fenAt(review, sideline.fromPly).split(" ")[1] === "b" ? "black" : "white";
      return (
        <>
          <CoachBubble>
            <div className="coach__head">
              <span className="coach__glyph">{Icons.eye}</span>
              <h2 className="coach__title">
                {replaced ? (
                  <>
                    Exploring <San san={first.san} side={branchMover} /> instead of <San san={replaced.san} side={replaced.mover} />
                  </>
                ) : (
                  "Exploring after the game"
                )}
              </h2>
              {sidelineBar && <EvalPill value={sidelineBar.eval} />}
            </div>
            <div className="coach__body">
              {shownClass ? (
                <p className="explore-verdict">
                  <ClassificationIcon value={shownClass} size={20} />
                  <span>
                    <San san={shownMove.san} side={shownMover} /> is {CLASSIFICATIONS[shownClass].phrase}.
                    {/* The coach's sentences name the better move themselves, as for game moves. */}
                    {shownSentences.length === 0 && shownJudged?.judgement.show_best_move && shownJudged.judgement.best_move && (
                      <>
                        {" "}
                        <San san={shownJudged.judgement.best_move.san} side={shownMover} /> was best.
                      </>
                    )}
                  </span>
                </p>
              ) : (
                !shownJudged && live.status !== "error" && <p className="coach__note">Judging the move…</p>
              )}
              {shownSentences.map((s, i) => (
                <CoachSentence key={i} index={i} sentence={s} />
              ))}
              {shownEnd && (
                <p className="coach__end">
                  {shownEnd === "checkmate" ? `Checkmate. ${toMove === "white" ? "Black" : "White"} wins.` : ENDINGS[shownEnd]}
                </p>
              )}
              <p>
                {shownOver ? "" : "Drag a piece to keep exploring. "}
                Step through your moves with ← →, or press Esc to go back to the game.
              </p>
            </div>
          </CoachBubble>
          <CoachActions actions={[{ label: "Back to game", icon: Icons.back, onClick: () => go(sideline.fromPly), primary: true, hint: "Back to the game (Esc)" }]} />
        </>
      );
    }

    // Replaying one of the coach's lines, with its narration.
    if (preview?.source === "coach" && showLine && previewing && current) {
      const atStart = preview.move < 0;
      const last = preview.move + 1 >= showLine.moves.length;
      const title = showLine.label.replace(/^Show /, "");
      const actions: CoachAction[] = [
        { label: "Prev", icon: Icons.prev, onClick: () => (setLinePlaying(false), stepPreview(-1)), hint: "Previous move (←)" },
        linePlaying
          ? { label: "Pause", icon: Icons.pause, onClick: () => setLinePlaying(false) }
          : {
              label: last ? "Replay" : "Play",
              icon: last ? Icons.retry : Icons.play,
              onClick: () => {
                if (last) setPreview({ ...preview, move: -1 });
                setLinePlaying(true);
              },
            },
        { label: "Next", icon: Icons.forward, onClick: () => (setLinePlaying(false), stepPreview(1)), hint: "Next move (→)" },
        { label: "Hide", icon: Icons.close, onClick: closePreview, primary: true, hint: "Back to the game (Esc)" },
      ];
      return (
        <>
          <CoachBubble>
            <div className="coach__head">
              <span className="coach__glyph">{Icons.eye}</span>
              <h2 className="coach__title">{title.charAt(0).toUpperCase() + title.slice(1)}</h2>
              <span className="show-line__count">
                {preview.move + 1}/{showLine.moves.length}
              </span>
            </div>
            <div className="coach__body">
              <p className="show-line__narration" aria-live="polite">
                {atStart ? "Here is the position. Watch the line, or step through it with ← →." : showLine.narration[preview.move]}
              </p>
              <ShowLineMoves
                line={showLine}
                move={preview.move}
                onSelect={(m) => {
                  setLinePlaying(false);
                  setPreview({ ...preview, move: m });
                }}
              />
            </div>
          </CoachBubble>
          <CoachActions actions={actions} />
        </>
      );
    }

    // Previewing an engine line.
    if (preview && preview.source !== "coach" && previewLine && previewMove && current) {
      const line = current.lines[preview.line];
      const fromBest = preview.source === "alternatives" && previewLine[0]?.uci === current.best_move?.uci;
      return (
        <>
          <CoachBubble>
            <div className="coach__head">
              {fromBest ? <ClassificationIcon value="best" size={26} /> : <span className="coach__glyph">{Icons.eye}</span>}
              <h2 className="coach__title">
                {fromBest ? (
                  <>
                    <San san={previewLine[0]!.san} side={current.mover} /> is the best move
                  </>
                ) : (
                  "Engine line"
                )}
              </h2>
              {line && <EvalPill value={line.eval} />}
            </div>
            <div className="coach__body">
              <p>
                Showing move {preview.move + 1} of {previewLine.length} in the engine's line. Use ← → to step through it.
              </p>
            </div>
          </CoachBubble>
          <CoachActions
            actions={[
              { label: "Back to game", icon: Icons.back, onClick: () => setPreview(null) },
              ...(preview.move + 1 < previewLine.length
                ? [{ label: "Continue line", icon: Icons.next, onClick: () => stepPreview(1), primary: true }]
                : [{ label: "Next move", icon: Icons.next, onClick: advance, primary: true }]),
            ]}
          />
        </>
      );
    }

    // Starting position.
    if (!current) {
      return (
        <>
          <CoachBubble>
            <div className="coach__head">
              <h2 className="coach__title">Let's go through the game</h2>
            </div>
            <div className="coach__body">
              <p>Press Next or → to step through every move. I'll mark the moves worth learning from.</p>
              {review.game.opening && <p>Opening: {review.game.opening.name}.</p>}
            </div>
          </CoachBubble>
          <CoachActions actions={[{ label: "Start", icon: Icons.next, onClick: advance, primary: true }]} />
        </>
      );
    }

    // The played move.
    if (!msg) return null;
    const atEnd = ply >= total;
    const actions: CoachAction[] = [
      { label: "Explain", icon: Icons.bulb, onClick: () => setExplain((v) => (saveFlag("review.explain", !v), !v)), pressed: explain },
    ];
    if (canRetry) {
      actions.push({ label: "Best", icon: Icons.star, onClick: showBest, hint: "Show the best move" });
      // Retry is offered where there is something to learn, not for near-best moves.
      if (learnable) {
        actions.push({
          label: "Retry",
          icon: Icons.retry,
          onClick: () => {
            closePreview();
            setRetry({ ply: current.ply, status: "trying", attempt: null, hint: 0, answer: null });
          },
          hint: "Try to find the best move yourself",
        });
      }
    }
    actions.push(
      atEnd
        ? { label: "Report", icon: Icons.report, onClick: () => setMode("report"), primary: true }
        : { label: "Next", icon: Icons.next, onClick: advance, primary: true },
    );
    return (
      <>
        <CoachBubble mood={coachMood(current.classification)}>
          <MoveVerdict ply={current} message={msg} marking={marking} onShowLine={openShowLine} />
          {atEnd && (
            <div className="coach__body">
              <p className="coach__end">
                End of the game{review.game.result && review.game.result !== "*" ? ` (${review.game.result})` : ""}.
                {review.game.termination ? ` ${review.game.termination}.` : ""}
              </p>
            </div>
          )}
        </CoachBubble>
        <CoachActions actions={actions} />
      </>
    );
  })();

  /* ---------------------------------------------------------------- layout */

  return (
    <div className="review">
      <section className="board-col" aria-label="Board">
        {strip(top)}
        <div className="board-row">
          {shownMove ? (
            <EvalBar whiteExpectedPoints={sidelineBar?.ep ?? null} evaluation={sidelineBar?.eval ?? null} flipped={flipped} />
          ) : (
            point && <EvalBar whiteExpectedPoints={point.white_expected_points} evaluation={point.eval} flipped={flipped} />
          )}
          <BoardView
            fen={displayFen}
            flipped={flipped}
            arrows={arrows}
            squareStyles={squareStyles}
            badge={badge}
            moves={boardMoves}
            promotion={boardPromotion}
          />
        </div>
        {strip(bottom)}
      </section>

      <aside className="panel" aria-label="Game review">
        <header className="panel__header">
          {mode === "review" ? (
            <button type="button" className="icon-btn" onClick={() => setMode("report")} aria-label="Back to report" title="Back to report">
              {Icons.back}
            </button>
          ) : (
            <span className="icon-btn-spacer" />
          )}
          <h1 className="panel__title">
            <LogoMark size={26} />
            Game Review
          </h1>
          <button
            type="button"
            className="icon-btn"
            onClick={() => setFlipped((f) => !f)}
            aria-pressed={flipped}
            aria-label="Flip board"
            title="Flip board (F)"
          >
            <svg width="20" height="20" viewBox="0 0 24 24" aria-hidden="true">
              <path fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" d="M7 4v15M3.5 15.5L7 19l3.5-3.5M17 20V5M13.5 8.5L17 5l3.5 3.5" />
            </svg>
          </button>
        </header>

        {mode === "report" ? (
          <ReportView
            review={review}
            current={ply}
            onSelect={go}
            onReviewAt={(p) => {
              go(p);
              setMode("review");
            }}
            onStart={() => {
              setMode("review");
              if (ply === 0 && total > 0) go(1);
            }}
            onNewGame={onNewGame}
          />
        ) : (
          <>
            <div className="review-pane">
              <div className="review-pane__coach" aria-live="polite">
                {coach}
              </div>
              {explain && current && msg && !activeRetry && !shownMove && (
                <div className="review-pane__details">
                  <MoveDetails ply={current} facts={msg.facts} preview={preview} onPreview={setPreview} />
                </div>
              )}
              {shownMove && !shownOver && (
                <div className="review-pane__details">
                  <LiveLines
                    fen={shownMove.fen}
                    engine={live}
                    onPlay={(line, move) => playMoves(liveLines[line]?.moves.slice(0, move + 1) ?? [])}
                  />
                </div>
              )}
              <MoveList
                plies={review.plies}
                current={ply}
                filter={filter}
                onSelect={go}
                sideline={
                  sideline && {
                    line: sideline,
                    branchFen: fenAt(review, sideline.fromPly),
                    onSelect: showSideline,
                    classes: judgements.map((j) => j?.judgement.classification ?? null),
                  }
                }
              />
              <div className="review-pane__graph">
                <EvalGraph
                  points={review.graph}
                  plies={review.plies}
                  current={ply}
                  phaseStarts={review.phase_starts}
                  onSelect={go}
                  height={64}
                />
              </div>
            </div>

            <nav className="nav" aria-label="Move navigation">
              <button type="button" className="nav__btn" onClick={() => go(0)} aria-label="First move" title="First move (Home)">
                <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 5h2.5v14H6zM19 5.5v13L9.5 12z" /></svg>
              </button>
              <button type="button" className="nav__btn" onClick={() => stepBoard(-1)} aria-label="Previous move" title="Previous (←)">
                <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M15.5 4.5L8 12l7.5 7.5" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" /></svg>
              </button>
              <button
                type="button"
                className="nav__btn"
                onClick={() => (ply >= total ? (go(0), setPlaying(true)) : setPlaying((p) => !p))}
                aria-label={playing ? "Pause" : "Play"}
                title={playing ? "Pause" : "Play through the game"}
              >
                {playing ? (
                  <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 5h3.5v14H7zM13.5 5H17v14h-3.5z" /></svg>
                ) : (
                  <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7.5 4.5v15L19.5 12z" /></svg>
                )}
              </button>
              <button type="button" className="nav__btn" onClick={() => stepBoard(1)} aria-label="Next move" title="Next (→)">
                <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8.5 4.5L16 12l-7.5 7.5" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" /></svg>
              </button>
              <button type="button" className="nav__btn" onClick={() => go(total)} aria-label="Last move" title="Last move (End)">
                <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 5.5v13l9.5-6.5zM15.5 5H18v14h-2.5z" /></svg>
              </button>
            </nav>

            <div className="panel__tools">
              <label className="tool">
                <span className="tool__label">Step through</span>
                <select aria-label="Step through" value={filter} onChange={(e) => setFilter(e.target.value as MoveFilter)}>
                  {FILTERS.map((f) => (
                    <option key={f.id} value={f.id}>
                      {f.label}
                    </option>
                  ))}
                </select>
              </label>
              <label className="tool tool--toggle" title="Show the engine's top move from the current position">
                <input
                  type="checkbox"
                  checked={showSuggestion}
                  onChange={(e) => {
                    setShowSuggestion(e.target.checked);
                    saveFlag("review.engineArrow", e.target.checked);
                  }}
                />
                <span className="switch" aria-hidden="true" />
                Engine arrow
              </label>
              <label className="tool tool--toggle" title="Play move and review sounds">
                <input
                  type="checkbox"
                  checked={soundOn}
                  onChange={(e) => {
                    setSoundOn(e.target.checked);
                    saveFlag("review.sound", e.target.checked);
                  }}
                />
                <span className="switch" aria-hidden="true" />
                Sound
              </label>
              <span className="tool__ply" aria-label={`Move ${ply} of ${total}`}>
                {ply}/{total}
              </span>
            </div>
          </>
        )}
      </aside>
    </div>
  );
}
