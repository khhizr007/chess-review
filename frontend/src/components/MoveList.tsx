import { useEffect, useRef } from "react";
import type { PlyReview } from "../api/generated/PlyReview";
import { CLASSIFICATIONS } from "../lib/format";
import { matchesFilter, showsListIcon, type MoveFilter } from "../lib/review";
import { ClassificationIcon } from "./ClassificationIcon";
import { San } from "./PieceIcon";

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

function MoveCell({
  ply,
  current,
  filter,
  onSelect,
}: {
  ply?: PlyReview;
  current: number;
  filter: MoveFilter;
  onSelect: (n: number) => void;
}) {
  if (!ply) return <td className="move-cell move-cell--empty">…</td>;
  const active = ply.ply === current;
  const icon = showsListIcon(ply);
  const meta = ply.classification ? CLASSIFICATIONS[ply.classification] : null;
  const dim = filter !== "all" && !matchesFilter(ply, filter);
  return (
    <td className="move-cell">
      <button
        type="button"
        className={`move ${active ? "move--active" : ""} ${dim ? "is-dim" : ""}`}
        aria-current={active ? "step" : undefined}
        data-ply={ply.ply}
        onClick={() => onSelect(ply.ply)}
        title={meta?.label}
        style={icon && meta ? { color: meta.color } : undefined}
      >
        <span className="move__icon">{icon && <ClassificationIcon value={ply.classification} size={18} />}</span>
        <San san={ply.san} side={ply.mover} />
      </button>
    </td>
  );
}

export function MoveList({ plies, current, filter, onSelect }: Props) {
  const container = useRef<HTMLDivElement>(null);

  // Keep the current move visible inside the list without scrolling the page.
  useEffect(() => {
    const box = container.current;
    const el = box?.querySelector<HTMLElement>(`[data-ply="${current}"]`);
    if (!box) return;
    if (!el) {
      if (current === 0) box.scrollTop = 0;
      return;
    }
    const row = (el.closest("tr") ?? el).getBoundingClientRect();
    const view = box.getBoundingClientRect();
    // Leave one row of context above/below the current move.
    if (row.top < view.top + row.height) box.scrollTop -= view.top + row.height - row.top;
    else if (row.bottom > view.bottom - row.height) box.scrollTop += row.bottom - (view.bottom - row.height);
  }, [current]);

  return (
    <div className="move-list" ref={container}>
      <table className="move-table">
        <tbody>
          {rows(plies).map((row) => (
            <tr key={`${row.number}-${row.white?.ply ?? row.black?.ply}`}>
              <th scope="row" className="move-table__number">
                {row.number}.
              </th>
              <MoveCell ply={row.white} current={current} filter={filter} onSelect={onSelect} />
              <MoveCell ply={row.black} current={current} filter={filter} onSelect={onSelect} />
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
