import { useCallback, useEffect, useMemo, useState, type CSSProperties } from "react";
import type { Arrow } from "react-chessboard";
import type { ReviewResult } from "../api/generated/ReviewResult";
import { CLASSIFICATIONS } from "../lib/format";
import { kingSquareToMove, uciSquares } from "../lib/fen";
import { FILTERS, fenAt, nextPlyAt, plyAt, stepFiltered, type MoveFilter } from "../lib/review";
import { BoardView } from "./BoardView";
import { EvalBar } from "./EvalBar";
import { MoveList } from "./MoveList";
import { MovePanel, type LinePreview } from "./MovePanel";
import { SummaryPanel } from "./SummaryPanel";

interface Props {
  review: ReviewResult;
  onNewGame: () => void;
}

const BEST_ARROW = "rgba(64, 160, 90, 0.85)";
const SUGGEST_ARROW = "rgba(60, 120, 210, 0.75)";

function isTyping(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName);
}

export function ReviewScreen({ review, onNewGame }: Props) {
  const total = review.plies.length;
  const [ply, setPly] = useState(0);
  const [flipped, setFlipped] = useState(false);
  const [filter, setFilter] = useState<MoveFilter>("all");
  const [preview, setPreview] = useState<LinePreview | null>(null);
  const [showSuggestion, setShowSuggestion] = useState(true);
  const [playing, setPlaying] = useState(false);

  const go = useCallback(
    (target: number) => {
      setPreview(null);
      setPly(Math.max(0, Math.min(total, target)));
    },
    [total],
  );

  // Autoplay through the game.
  useEffect(() => {
    if (!playing) return;
    if (ply >= total) {
      setPlaying(false);
      return;
    }
    const t = setTimeout(() => go(ply + 1), 900);
    return () => clearTimeout(t);
  }, [playing, ply, total, go]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (isTyping(e.target) || e.altKey || e.ctrlKey || e.metaKey) return;
      if (e.key === "ArrowLeft") go(ply - 1);
      else if (e.key === "ArrowRight") go(ply + 1);
      else if (e.key === "Home") go(0);
      else if (e.key === "End") go(total);
      else if (e.key === "f") setFlipped((f) => !f);
      else if (e.key === "Escape") setPreview(null);
      else return;
      e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [go, ply, total]);

  const current = plyAt(review, ply);
  const next = nextPlyAt(review, ply);

  const previewMove = useMemo(() => {
    if (!preview) return undefined;
    const lines = preview.source === "alternatives" ? current?.lines : next?.lines;
    return lines?.[preview.line]?.moves[preview.move];
  }, [preview, current, next]);

  const fen = previewMove?.fen ?? fenAt(review, ply);

  const { arrows, squareStyles } = useMemo(() => {
    const arrows: Arrow[] = [];
    const styles: Record<string, CSSProperties> = {};
    if (previewMove) {
      const sq = uciSquares(previewMove.uci);
      if (sq) {
        styles[sq.from] = { background: "rgba(230, 150, 40, 0.35)" };
        styles[sq.to] = { background: "rgba(230, 150, 40, 0.55)" };
      }
      return { arrows, squareStyles: styles };
    }
    if (current) {
      const color = current.classification ? CLASSIFICATIONS[current.classification].color : "var(--c-best)";
      const tint = `color-mix(in srgb, ${color} 45%, transparent)`;
      if (current.from) styles[current.from] = { background: tint };
      styles[current.to] = { background: tint };
      if (current.show_best_move && current.best_move) {
        const sq = uciSquares(current.best_move.uci);
        if (sq) arrows.push({ startSquare: sq.from, endSquare: sq.to, color: BEST_ARROW });
      }
      if (current.flags.check) {
        const king = kingSquareToMove(current.fen_after);
        if (king) styles[king] = { background: "radial-gradient(circle, rgba(220,40,40,.75) 0%, transparent 70%)" };
      }
    }
    if (showSuggestion && next?.lines[0]?.moves[0]) {
      const sq = uciSquares(next.lines[0].moves[0].uci);
      if (sq && !arrows.some((a) => a.startSquare === sq.from && a.endSquare === sq.to)) {
        arrows.push({ startSquare: sq.from, endSquare: sq.to, color: SUGGEST_ARROW });
      }
    }
    return { arrows, squareStyles: styles };
  }, [current, next, previewMove, showSuggestion]);

  const point = review.graph[ply];
  const step = (dir: 1 | -1) => {
    if (filter === "all") return go(ply + dir);
    const target = stepFiltered(review, ply, dir, filter);
    if (target !== null) go(target);
  };

  const g = review.game;
  return (
    <div className="review">
      <div className="review__head">
        <div>
          <h1 className="review__title">
            {g.white ?? "White"} <span className="muted">vs</span> {g.black ?? "Black"}
            {g.result && <span className="result">{g.result}</span>}
          </h1>
          <p className="muted small">
            {[g.event, g.site, g.date, g.opening && `${g.opening.eco} ${g.opening.name}`, g.termination]
              .filter(Boolean)
              .join(" · ")}
          </p>
        </div>
        <button type="button" className="button button--ghost" onClick={onNewGame}>
          New review
        </button>
      </div>

      <div className="review__board">
        <div className="board-row">
          {point && (
            <EvalBar whiteExpectedPoints={point.white_expected_points} evaluation={point.eval} flipped={flipped} />
          )}
          <BoardView fen={fen} flipped={flipped} arrows={arrows} squareStyles={squareStyles} />
        </div>
        {previewMove && (
          <div className="notice notice--preview">
            Previewing engine line after <strong>{previewMove.san}</strong>.{" "}
            <button type="button" className="link" onClick={() => setPreview(null)}>
              Back to game (Esc)
            </button>
          </div>
        )}
        <div className="controls" role="toolbar" aria-label="Move navigation">
          <button type="button" className="button" onClick={() => go(0)} aria-label="First move" title="Home">
            ⏮
          </button>
          <button type="button" className="button" onClick={() => step(-1)} aria-label="Previous move" title="←">
            ◀
          </button>
          <button
            type="button"
            className="button"
            onClick={() => (ply >= total ? (go(0), setPlaying(true)) : setPlaying((p) => !p))}
            aria-label={playing ? "Pause" : "Play"}
          >
            {playing ? "❚❚" : "▶︎▶︎"}
          </button>
          <button type="button" className="button" onClick={() => step(1)} aria-label="Next move" title="→">
            ▶
          </button>
          <button type="button" className="button" onClick={() => go(total)} aria-label="Last move" title="End">
            ⏭
          </button>
          <button
            type="button"
            className="button"
            onClick={() => setFlipped((f) => !f)}
            aria-pressed={flipped}
            title="Flip board (f)"
          >
            ⇅ Flip
          </button>
          <label className="toggle small">
            <input type="checkbox" checked={showSuggestion} onChange={(e) => setShowSuggestion(e.target.checked)} />
            Engine arrow
          </label>
          <span className="muted small controls__ply">
            {ply}/{total}
          </span>
        </div>
      </div>

      <aside className="review__side">
        <MovePanel ply={current} next={next} preview={preview} onPreview={setPreview} />
        <section className="card moves-card">
          <div className="card__header">
            <h2 className="card__title">Moves</h2>
            <select
              aria-label="Show moves"
              value={filter}
              onChange={(e) => setFilter(e.target.value as MoveFilter)}
              className="select-small"
            >
              {FILTERS.map((f) => (
                <option key={f.id} value={f.id}>
                  {f.label}
                </option>
              ))}
            </select>
          </div>
          <MoveList plies={review.plies} current={ply} filter={filter} onSelect={go} />
        </section>
      </aside>

      <div className="review__summary">
        <SummaryPanel review={review} current={ply} onSelect={go} />
      </div>
    </div>
  );
}
