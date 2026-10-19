import { useCallback, useEffect, useMemo, useState, type CSSProperties } from "react";
import type { Arrow } from "react-chessboard";
import type { ArrowKind } from "../api/generated/ArrowKind";
import type { MoveRef } from "../api/generated/MoveRef";
import type { ReviewResult } from "../api/generated/ReviewResult";
import type { Side } from "../api/generated/Side";
import { LEARNABLE, coachArrows, coachMessage, coachMood, playerName } from "../lib/coach";
import { kingSquareToMove, uciSquares } from "../lib/fen";
import { CLASSIFICATIONS, formatEvalPill, moveNumberLabel } from "../lib/format";
import { materialFromFen } from "../lib/material";
import { hintLadder, hintStep, judgeRetry, type RetryAnswer } from "../lib/retry";
import { FILTERS, fenAt, nextPlyAt, plyAt, stepFiltered, type MoveFilter } from "../lib/review";
import { BoardView, type BoardBadge, type BoardRetry } from "./BoardView";
import { ClassificationIcon } from "./ClassificationIcon";
import {
  CoachActions,
  CoachBubble,
  EvalPill,
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
/** Pause between moves when a coach line plays itself. */
const LINE_STEP_MS = 1400;
const COACH_ARROW: Record<ArrowKind, string> = {
  move: "rgba(255, 164, 89, 0.9)",
  attack: "rgba(250, 65, 45, 0.85)",
  defence: SUGGEST_ARROW,
  best: BEST_ARROW,
};

function isTyping(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName);
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

export function ReviewScreen({ review, onNewGame }: Props) {
  const total = review.plies.length;
  const [mode, setMode] = useState<Mode>("report");
  const [ply, setPly] = useState(0);
  const [flipped, setFlipped] = useState(false);
  const [filter, setFilter] = useState<MoveFilter>("all");
  const [preview, setPreview] = useState<LinePreview | null>(null);
  const [retry, setRetry] = useState<Retry | null>(null);
  const [showSuggestion, setShowSuggestion] = useState(() => loadFlag("review.engineArrow", false));
  const [explain, setExplain] = useState(() => loadFlag("review.explain", false));
  const [playing, setPlaying] = useState(false);
  const [linePlaying, setLinePlaying] = useState(false);
  const [hovered, setHovered] = useState<Mark | null>(null);
  const [pinned, setPinned] = useState<Mark | null>(null);

  const go = useCallback(
    (target: number) => {
      setPreview(null);
      setLinePlaying(false);
      setRetry(null);
      setPinned(null);
      setHovered(null);
      setPly(Math.max(0, Math.min(total, target)));
    },
    [total],
  );

  const current = plyAt(review, ply);
  const next = nextPlyAt(review, ply);

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
        step(-1);
      } else if (e.key === "ArrowRight") {
        setMode("review");
        step(1);
      } else if (e.key === "Home" || e.key === "End") {
        setMode("review");
        go(e.key === "Home" ? 0 : total);
      } else if (e.key === "f") setFlipped((f) => !f);
      else if (e.key === "Escape") {
        setPreview(null);
        setLinePlaying(false);
        setRetry(null);
        setPinned(null);
      } else return;
      e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [go, step, stepPreview, preview, total]);

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
      setRetry({ ...activeRetry, status: "correct", attempt: null, answer });
      return true;
    }
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
    : (previewMove?.fen ?? (showLine && previewing ? showLine.start_fen : fenAt(review, ply)));

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
        if (king) styles[king] = { ...styles[king], backgroundImage: "radial-gradient(circle, rgba(235,40,40,.9) 0%, rgba(235,40,40,.4) 45%, transparent 72%)" };
      }
    }
    if (showSuggestion && next?.lines[0]?.moves[0]) {
      const sq = uciSquares(next.lines[0].moves[0].uci);
      if (sq && !arrows.some((a) => a.startSquare === sq.from && a.endSquare === sq.to)) {
        arrows.push({ startSquare: sq.from, endSquare: sq.to, color: SUGGEST_ARROW });
      }
    }
    return { arrows, squareStyles: styles, badge };
  }, [activeRetry, solvedMove, bestMove, ladder, showLine, previewMove, preview, current, markedFragment, next, showSuggestion]);

  const boardRetry: BoardRetry | null =
    activeRetry && current && !retrySolved ? { side: current.mover, onDrop: onRetryDrop } : null;

  const material = materialFromFen(displayFen);
  const toMove: Side = displayFen.split(" ")[1] === "b" ? "black" : "white";
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
          {point && (
            <EvalBar whiteExpectedPoints={point.white_expected_points} evaluation={point.eval} flipped={flipped} />
          )}
          <BoardView
            fen={displayFen}
            flipped={flipped}
            arrows={arrows}
            squareStyles={squareStyles}
            badge={badge}
            retry={boardRetry}
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
              {explain && current && msg && !activeRetry && (
                <div className="review-pane__details">
                  <MoveDetails ply={current} facts={msg.facts} preview={preview} onPreview={setPreview} />
                </div>
              )}
              <MoveList plies={review.plies} current={ply} filter={filter} onSelect={go} />
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
              <button type="button" className="nav__btn" onClick={() => step(-1)} aria-label="Previous move" title="Previous (←)">
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
              <button type="button" className="nav__btn" onClick={() => step(1)} aria-label="Next move" title="Next (→)">
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
