import { useMemo, type KeyboardEvent, type MouseEvent } from "react";
import type { Classification } from "../api/generated/Classification";
import type { GraphPoint } from "../api/generated/GraphPoint";
import type { PhaseStarts } from "../api/generated/PhaseStarts";
import { CLASSIFICATIONS, formatEval, percent } from "../lib/format";

interface Props {
  points: GraphPoint[];
  current: number;
  phaseStarts: PhaseStarts;
  onSelect: (ply: number) => void;
}

const W = 600;
const H = 140;
/** Classifications marked on the graph. */
const MARKED: ReadonlySet<Classification> = new Set(["brilliant", "great_find", "miss", "mistake", "blunder"]);

/**
 * White's expected score per ply (backend values). Area above the curve is
 * Black's share, below is White's; markers show notable moves.
 */
export function EvalGraph({ points, current, phaseStarts, onSelect }: Props) {
  const last = Math.max(1, points.length - 1);
  const x = (ply: number) => (ply / last) * W;
  const y = (ep: number) => (1 - ep) * H;

  const area = useMemo(() => {
    if (points.length === 0) return "";
    const line = points.map((p) => `${x(p.ply).toFixed(1)},${y(p.white_expected_points).toFixed(1)}`).join(" L");
    return `M0,${H} L${line} L${W},${H} Z`;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [points, last]);

  const pick = (e: MouseEvent<SVGSVGElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const ratio = (e.clientX - rect.left) / rect.width;
    onSelect(Math.round(Math.min(1, Math.max(0, ratio)) * last));
  };

  const onKey = (e: KeyboardEvent<SVGSVGElement>) => {
    if (e.key === "ArrowLeft") onSelect(Math.max(0, current - 1));
    else if (e.key === "ArrowRight") onSelect(Math.min(last, current + 1));
    else return;
    e.preventDefault();
  };

  const cur = points[current];

  return (
    <figure className="eval-graph">
      <div className="eval-graph__plot">
      <svg
        viewBox={`0 0 ${W} ${H}`}
        preserveAspectRatio="none"
        role="slider"
        tabIndex={0}
        aria-label="Evaluation graph"
        aria-valuemin={0}
        aria-valuemax={last}
        aria-valuenow={current}
        aria-valuetext={cur ? `Ply ${cur.ply}: ${formatEval(cur.eval)}, White ${percent(cur.white_expected_points)}` : undefined}
        onClick={pick}
        onKeyDown={onKey}
      >
        <rect className="eval-graph__black" x={0} y={0} width={W} height={H} />
        <path className="eval-graph__white" d={area} />
        <line className="eval-graph__mid" vectorEffect="non-scaling-stroke" x1={0} x2={W} y1={H / 2} y2={H / 2} />
        {[phaseStarts.middlegame, phaseStarts.endgame].map(
          (ply, i) =>
            ply != null && (
              <line key={i} className="eval-graph__phase" vectorEffect="non-scaling-stroke" x1={x(ply - 1)} x2={x(ply - 1)} y1={0} y2={H} />
            ),
        )}
        <line className="eval-graph__cursor" vectorEffect="non-scaling-stroke" x1={x(current)} x2={x(current)} y1={0} y2={H} />
      </svg>
      <div className="eval-graph__markers" aria-hidden="true">
        {points.map((p) =>
          p.classification && MARKED.has(p.classification) ? (
            <span
              key={p.ply}
              className="eval-graph__marker"
              title={`Ply ${p.ply}: ${CLASSIFICATIONS[p.classification].label}`}
              style={{
                left: `${(p.ply / last) * 100}%`,
                top: `${(1 - p.white_expected_points) * 100}%`,
                background: CLASSIFICATIONS[p.classification].color,
              }}
            />
          ) : null,
        )}
      </div>
      </div>
      <figcaption className="muted small">
        White's expected score by ply · dashed lines mark the middlegame and endgame · click to jump
      </figcaption>
    </figure>
  );
}
