import { useEffect, useRef } from "react";
import type { PlyReview } from "../api/generated/PlyReview";
import { CLASSIFICATIONS, moveNumberLabel } from "../lib/format";
import { matchesFilter, type MoveFilter } from "../lib/review";
import { ClassificationBadge } from "./ClassificationBadge";

interface Props {
  plies: PlyReview[];
  current: number;
  filter: MoveFilter;
  onSelect: (ply: number) => void;
}

interface Row {
  number: number;
  white?: PlyReview;
  black?: PlyReview;
}

function rows(plies: PlyReview[]): Row[] {
  const out: Row[] = [];
  for (const p of plies) {
    const last = out[out.length - 1];
    if (p.mover === "white" || !last || last.black || last.number !== p.move_number) {
      out.push({ number: p.move_number, [p.mover]: p });
    } else {
      last.black = p;
    }
  }
  return out;
}

function MoveCell({ ply, current, onSelect }: { ply?: PlyReview; current: number; onSelect: (n: number) => void }) {
  if (!ply) return <td className="move-cell move-cell--empty">…</td>;
  const active = ply.ply === current;
  return (
    <td className="move-cell">
      <button
        type="button"
        className={`move ${active ? "move--active" : ""}`}
        aria-current={active ? "step" : undefined}
        data-ply={ply.ply}
        onClick={() => onSelect(ply.ply)}
        title={ply.classification ? CLASSIFICATIONS[ply.classification].label : undefined}
      >
        <ClassificationBadge value={ply.classification} />
        <span className="move__san">{ply.san}</span>
      </button>
    </td>
  );
}

export function MoveList({ plies, current, filter, onSelect }: Props) {
  const container = useRef<HTMLDivElement>(null);

  // Keep the current move visible while navigating.
  useEffect(() => {
    const el = container.current?.querySelector<HTMLElement>(`[data-ply="${current}"]`);
    el?.scrollIntoView?.({ block: "nearest" });
  }, [current, filter]);

  if (filter !== "all") {
    const matching = plies.filter((p) => matchesFilter(p, filter));
    return (
      <div className="move-list" ref={container}>
        {matching.length === 0 && <p className="muted small pad">No moves match this view.</p>}
        <ul className="move-filtered">
          {matching.map((p) => (
            <li key={p.ply}>
              <button
                type="button"
                data-ply={p.ply}
                className={`move move--wide ${p.ply === current ? "move--active" : ""}`}
                onClick={() => onSelect(p.ply)}
              >
                <span className="muted move__number">{moveNumberLabel(p.move_number, p.mover)}</span>
                <ClassificationBadge value={p.classification} />
                <span className="move__san">{p.san}</span>
                <span className="muted small move__meta">
                  {p.classification ? CLASSIFICATIONS[p.classification].label : ""}
                  {p.ep_lost >= 0.005 ? ` · −${(p.ep_lost * 100).toFixed(0)}%` : ""}
                </span>
              </button>
            </li>
          ))}
        </ul>
      </div>
    );
  }

  return (
    <div className="move-list" ref={container}>
      <table className="move-table">
        <tbody>
          {rows(plies).map((row) => (
            <tr key={`${row.number}-${row.white?.ply ?? row.black?.ply}`}>
              <th scope="row" className="muted move-table__number">
                {row.number}.
              </th>
              <MoveCell ply={row.white} current={current} onSelect={onSelect} />
              <MoveCell ply={row.black} current={current} onSelect={onSelect} />
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
