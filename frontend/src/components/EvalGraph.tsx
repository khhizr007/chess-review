import { useMemo, useState, type KeyboardEvent, type MouseEvent } from "react";
import type { GraphPoint } from "../api/generated/GraphPoint";
import type { PhaseStarts } from "../api/generated/PhaseStarts";
import type { PlyReview } from "../api/generated/PlyReview";
import { CLASSIFICATIONS, formatEvalPill, moveNumberLabel, percent } from "../lib/format";

interface Props {
  points: GraphPoint[];
  plies: PlyReview[];
  current: number;
  phaseStarts: PhaseStarts;
  onSelect: (ply: number) => void;
  /** Plot height in px. */
  height?: number;
}

const W = 600;
const H = 100;

/**
 * White's expected score per ply (backend values). The white area is White's
 * share; dots mark notable moves; the cursor shows the current move.
 */
export function EvalGraph({ points, plies, current, phaseStarts, onSelect, height = 84 }: Props) {
  const [hover, setHover] = useState<number | null>(null);
  const last = Math.max(1, points.length - 1);
  const x = (ply: number) => (ply / last) * W;
  // Keep a thin margin so ±100% stays visible against the frame.
  const y = (ep: number) => 3 + (1 - ep) * (H - 6);

  const area = useMemo(() => {
    if (points.length === 0) return "";
    const line = points.map((p) => `${x(p.ply).toFixed(1)},${y(p.white_expected_points).toFixed(1)}`).join(" L");
    return `M0,${H} L${line} L${W},${H} Z`;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [points, last]);

  const plyFromEvent = (e: MouseEvent<HTMLElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const ratio = (e.clientX - rect.left) / rect.width;
    return Math.round(Math.min(1, Math.max(0, ratio)) * last);
  };

  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === "ArrowLeft") onSelect(Math.max(0, current - 1));
    else if (e.key === "ArrowRight") onSelect(Math.min(last, current + 1));
    else return;
    e.preventDefault();
    e.stopPropagation();
  };

  const cur = points[current];
  const curCls = plies[current - 1]?.classification;
  const hp = hover != null ? points[hover] : undefined;
  const hm = hover != null ? plies[hover - 1] : undefined;

  return (
    <div
      className="eval-graph"
      style={{ height }}
      role="slider"
      tabIndex={0}
      aria-label="Evaluation graph"
      aria-valuemin={0}
      aria-valuemax={last}
      aria-valuenow={current}
      aria-valuetext={cur ? `Move ${cur.ply}: ${formatEvalPill(cur.eval)}, White ${percent(cur.white_expected_points)}` : undefined}
      onClick={(e) => onSelect(plyFromEvent(e))}
      onMouseMove={(e) => setHover(plyFromEvent(e))}
      onMouseLeave={() => setHover(null)}
      onKeyDown={onKey}
    >
      <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" aria-hidden="true">
        <path className="eval-graph__white" d={area} />
        <line className="eval-graph__mid" vectorEffect="non-scaling-stroke" x1={0} x2={W} y1={H / 2} y2={H / 2} />
        {[phaseStarts.middlegame, phaseStarts.endgame].map(
          (ply, i) =>
            ply != null && (
              <line key={i} className="eval-graph__phase" vectorEffect="non-scaling-stroke" x1={x(ply - 1)} x2={x(ply - 1)} y1={0} y2={H} />
            ),
        )}
        {hover != null && hover !== current && (
          <line className="eval-graph__hover" vectorEffect="non-scaling-stroke" x1={x(hover)} x2={x(hover)} y1={0} y2={H} />
        )}
        <line
          className="eval-graph__cursor"
          style={curCls ? { stroke: CLASSIFICATIONS[curCls].color } : undefined}
          vectorEffect="non-scaling-stroke"
          x1={x(current)}
          x2={x(current)}
          y1={0}
          y2={H}
        />
      </svg>
      <div className="eval-graph__markers" aria-hidden="true">
        {points.map((p) =>
          p.classification && CLASSIFICATIONS[p.classification].notable ? (
            <span
              key={p.ply}
              className={`eval-graph__marker ${p.ply === current ? "is-current" : ""}`}
              style={{
                left: `${(p.ply / last) * 100}%`,
                top: `${(y(p.white_expected_points) / H) * 100}%`,
                background: CLASSIFICATIONS[p.classification].color,
              }}
            />
          ) : null,
        )}
      </div>
      {hp && (
        <div
          className={`eval-graph__tip ${hover! / last > 0.6 ? "is-left" : ""}`}
          style={{ left: `${(hover! / last) * 100}%` }}
          aria-hidden="true"
        >
          {hm ? (
            <>
              <span className="muted">{moveNumberLabel(hm.move_number, hm.mover)}</span> {hm.san}
            </>
          ) : (
            "Start"
          )}
          <strong>{formatEvalPill(hp.eval)}</strong>
        </div>
      )}
    </div>
  );
}
