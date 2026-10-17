import { useCallback, useEffect, useMemo, useState, type CSSProperties } from "react";
import type { Arrow } from "react-chessboard";
import type { MoveRef } from "../api/generated/MoveRef";
import type { ReviewResult } from "../api/generated/ReviewResult";
import type { Side } from "../api/generated/Side";
import { LEARNABLE, coachMessage, playerName } from "../lib/coach";
import { kingSquareToMove, uciSquares } from "../lib/fen";
import { CLASSIFICATIONS, formatEvalPill, moveNumberLabel } from "../lib/format";
import { materialFromFen } from "../lib/material";
import { FILTERS, fenAt, nextPlyAt, plyAt, stepFiltered, type MoveFilter } from "../lib/review";
import { BoardView, type BoardBadge, type BoardRetry } from "./BoardView";
import { ClassificationIcon, ClassificationIconDefs } from "./ClassificationIcon";
import {
  CoachActions,
  CoachBubble,
  EvalPill,
  MoveDetails,
  MoveVerdict,
  type CoachAction,
  type LinePreview,
} from "./Coach";
import { EvalBar } from "./EvalBar";
import { Icons } from "./icons";
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
  hint: boolean;
}

const BEST_ARROW = "rgba(129, 182, 76, 0.9)";
const SUGGEST_ARROW = "rgba(92, 139, 176, 0.85)";
const PREVIEW_TINT = "rgba(255, 196, 64, 0.5)";

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

  const go = useCallback(
    (target: number) => {
      setPreview(null);
      setRetry(null);
      setPly(Math.max(0, Math.min(total, target)));
    },
    [total],
  );

  const current = plyAt(review, ply);
  const next = nextPlyAt(review, ply);

  const previewLine: MoveRef[] | undefined = useMemo(() => {
    if (!preview) return undefined;
    return (preview.source === "alternatives" ? current?.lines : next?.lines)?.[preview.line]?.moves;
  }, [preview, current, next]);
  const previewMove = preview ? previewLine?.[preview.move] : undefined;

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
      if (m < 0) setPreview(null);
      else if (m < previewLine.length) setPreview({ ...preview, move: m });
    },
    [preview, previewLine],
  );

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (isTyping(e.target) || e.altKey || e.ctrlKey || e.metaKey) return;
      if (preview && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
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
        setRetry(null);
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

  const onRetryDrop = (from: string, to: string): boolean => {
    if (!current?.best_move || !activeRetry) return false;
    const uci = `${from}${to}`;
    if (current.best_move.uci.startsWith(uci)) {
      setRetry({ ...activeRetry, status: "correct", attempt: null });
      return true;
    }
    const known = current.lines.find((l) => l.moves[0]?.uci.startsWith(uci))?.moves[0]?.san ?? null;
    setRetry({ ...activeRetry, status: "wrong", attempt: known ?? (current.uci.startsWith(uci) ? current.san : null) });
    return false;
  };

  /* ---------------------------------------------------------------- board */

  const displayFen = activeRetry
    ? retrySolved && bestMove
      ? bestMove.fen
      : (current?.fen_before ?? fenAt(review, ply))
    : (previewMove?.fen ?? fenAt(review, ply));

  const { arrows, squareStyles, badge } = useMemo(() => {
    const arrows: Arrow[] = [];
    const styles: Record<string, CSSProperties> = {};
    let badge: BoardBadge | null = null;

    if (activeRetry && bestMove) {
      const sq = uciSquares(bestMove.uci);
      if (sq && (activeRetry.status === "correct" || activeRetry.status === "revealed")) {
        styles[sq.from] = tint(CLASSIFICATIONS.best.color, 45);
        styles[sq.to] = tint(CLASSIFICATIONS.best.color, 60);
        badge = { square: sq.to, classification: "best" };
      } else if (sq && activeRetry.hint) {
        styles[sq.from] = { boxShadow: "inset 0 0 0 4px rgba(255, 196, 64, 0.95)", background: "rgba(255, 196, 64, 0.25)" };
      }
      return { arrows, squareStyles: styles, badge };
    }

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
      if (current.show_best_move && current.best_move && !current.played_is_best) {
        const sq = uciSquares(current.best_move.uci);
        if (sq) arrows.push({ startSquare: sq.from, endSquare: sq.to, color: BEST_ARROW });
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
  }, [activeRetry, bestMove, previewMove, preview, current, next, showSuggestion]);

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
      const actions: CoachAction[] = retrySolved
        ? [
            { label: "Back", icon: Icons.back, onClick: () => setRetry(null) },
            { label: "Next", icon: Icons.next, onClick: advance, primary: true },
          ]
        : [
            { label: "Hint", icon: Icons.bulb, onClick: () => setRetry({ ...activeRetry, hint: true }), pressed: activeRetry.hint },
            { label: "Show", icon: Icons.eye, onClick: () => setRetry({ ...activeRetry, status: "revealed" }), hint: "Show the answer" },
            { label: "Cancel", onClick: () => setRetry(null) },
          ];
      return (
        <>
          <CoachBubble>
            <div className="coach__head">
              {retrySolved ? <ClassificationIcon value="best" size={26} /> : <span className="coach__glyph">{Icons.target}</span>}
              <h2 className="coach__title">
                {activeRetry.status === "correct" ? (
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
              {retrySolved && current.eval_best && <EvalPill value={current.eval_best} />}
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
                  {activeRetry.hint ? "." : ", or ask for a hint."}
                </p>
              )}
              {activeRetry.hint && !retrySolved && <p>The highlighted piece is the one to move.</p>}
              {retrySolved && current.eval_best && (
                <p>
                  It keeps the evaluation at {formatEvalPill(current.eval_best)} instead of {formatEvalPill(current.eval_after)}.
                </p>
              )}
            </div>
          </CoachBubble>
          <CoachActions actions={actions} />
        </>
      );
    }

    // Previewing an engine line.
    if (preview && previewLine && previewMove && current) {
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
            setPreview(null);
            setRetry({ ply: current.ply, status: "trying", attempt: null, hint: false });
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
        <CoachBubble>
          <MoveVerdict ply={current} message={msg} />
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
      <ClassificationIconDefs />
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
            <span className="panel__logo" aria-hidden="true">
              <ClassificationIcon value="best" size={22} />
            </span>
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
