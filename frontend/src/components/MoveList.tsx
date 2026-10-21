import { Fragment, useEffect, useRef } from "react";
import type { Classification } from "../api/generated/Classification";
import type { PlyReview } from "../api/generated/PlyReview";
import { sidelineLabels, type Sideline } from "../lib/explore";
import { CLASSIFICATIONS } from "../lib/format";
import { matchesFilter, showsListIcon, type MoveFilter } from "../lib/review";
import { ClassificationIcon } from "./ClassificationIcon";
import { San } from "./PieceIcon";

/** The sideline the reader is exploring, listed under the game move it replaces. */
export interface ListSideline {
  line: Sideline;
  /** Position the sideline branches from (its move numbers count from there). */
  branchFen: string;
  /** Selects sideline move k (1-based), as the sideline's cursor. */
  onSelect: (cursor: number) => void;
  /** Classification of each sideline move judged so far (docs/explore-mode.md §5.5). */
  classes?: (Classification | null)[];
}

interface Props {
  plies: PlyReview[];
  current: number;
  filter: MoveFilter;
  onSelect: (ply: number) => void;
  sideline?: ListSideline | null;
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

function SidelineRow({ sideline }: { sideline: ListSideline }) {
  const { line, branchFen, onSelect } = sideline;
  const labels = sidelineLabels(branchFen, line.moves.length);
  return (
    <tr className="move-table__sideline">
      <td colSpan={3}>
        <div className="sideline" aria-label="Your sideline">
          {line.moves.map((m, i) => {
            const active = line.cursor === i + 1;
            const label = labels[i]!;
            const cls = sideline.classes?.[i] ?? null;
            const meta = cls ? CLASSIFICATIONS[cls] : null;
            return (
              <Fragment key={i}>
                {label.number && <span className="sideline__number">{label.number}</span>}
                <button
                  type="button"
                  className={`move move--sideline ${active ? "move--active" : ""}`}
                  aria-current={active ? "step" : undefined}
                  data-sideline={i + 1}
                  onClick={() => onSelect(i + 1)}
                  title={meta?.label}
                  style={meta ? { color: meta.color } : undefined}
                >
                  {cls && (
                    <span className="move__icon">
                      <ClassificationIcon value={cls} size={16} />
                    </span>
                  )}
                  <San san={m.san} side={label.mover} />
                </button>
              </Fragment>
            );
          })}
        </div>
      </td>
    </tr>
  );
}

export function MoveList({ plies, current, filter, onSelect, sideline }: Props) {
  const container = useRef<HTMLDivElement>(null);
  const cursor = sideline?.line.cursor ?? 0;

  // Keep the current move visible inside the list without scrolling the page.
  useEffect(() => {
    const box = container.current;
    const el = box?.querySelector<HTMLElement>(cursor > 0 ? `[data-sideline="${cursor}"]` : `[data-ply="${current}"]`);
    if (!box) return;
    if (!el) {
      if (current === 0) box.scrollTop = 0;
      return;
    }
    // A game move scrolls by its row; a sideline move by itself, as its row may wrap.
    const row = (cursor > 0 ? el : (el.closest("tr") ?? el)).getBoundingClientRect();
    const view = box.getBoundingClientRect();
    // Leave one row of context above/below the current move.
    if (row.top < view.top + row.height) box.scrollTop -= view.top + row.height - row.top;
    else if (row.bottom > view.bottom - row.height) box.scrollTop += row.bottom - (view.bottom - row.height);
  }, [current, cursor]);

  // The sideline is listed under the row of the game move it replaces (the last row at the end).
  const all = rows(plies);
  const replaced = sideline ? Math.min(sideline.line.fromPly + 1, plies.length) : -1;
  const at = sideline ? all.findIndex((r) => r.white?.ply === replaced || r.black?.ply === replaced) : -1;

  return (
    <div className="move-list" ref={container}>
      <table className="move-table">
        <tbody>
          {sideline && at < 0 && <SidelineRow sideline={sideline} />}
          {all.map((row, i) => (
            <Fragment key={`${row.number}-${row.white?.ply ?? row.black?.ply}`}>
              <tr className={i % 2 ? "is-striped" : undefined}>
                <th scope="row" className="move-table__number">
                  {row.number}.
                </th>
                <MoveCell ply={row.white} current={cursor > 0 ? -1 : current} filter={filter} onSelect={onSelect} />
                <MoveCell ply={row.black} current={cursor > 0 ? -1 : current} filter={filter} onSelect={onSelect} />
              </tr>
              {sideline && i === at && <SidelineRow sideline={sideline} />}
            </Fragment>
          ))}
        </tbody>
      </table>
    </div>
  );
}
