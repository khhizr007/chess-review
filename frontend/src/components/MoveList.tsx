import { Fragment, useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import type { Classification } from "../api/generated/Classification";
import type { PlyReview } from "../api/generated/PlyReview";
import type { PlayedMove } from "../lib/chess";
import { moveLabel, type ExploreNode } from "../lib/explore";
import { CLASSIFICATIONS } from "../lib/format";
import { lastBookPly, matchesFilter, showsListIcon, type MoveFilter } from "../lib/review";
import { ClassificationIcon } from "./ClassificationIcon";
import { San } from "./PieceIcon";

/**
 * The lines explored instead of one game move, listed under it while the board shows that move or is
 * in one of the lines (Chess.com's review lists no other variations).
 */
export interface ListSideline {
  /** Game ply the lines branch from. */
  fromPly: number;
  /** The first move of each line, in the order played. */
  lines: ExploreNode[];
  /** Position the lines branch from (their move numbers count from there). */
  branchFen: string;
  /** Path of the explored move shown (child indices from `lines`), or null on a game position. */
  shown: number[] | null;
  /** Shows the explored move at `path`. */
  onSelect: (path: number[]) => void;
  /** Classification of an explored move judged so far (docs/explore-mode.md §5.5): the last of `moves`. */
  classOf?: (moves: PlayedMove[]) => Classification | null;
}

/** Chess.com's variation picker: which continuation of the shown move → follows. */
export interface ListChoice {
  options: PlayedMove[];
  index: number;
  onHover: (index: number) => void;
  onPick: (index: number) => void;
  onClose: () => void;
}

interface Props {
  plies: PlyReview[];
  current: number;
  filter: MoveFilter;
  onSelect: (ply: number) => void;
  sideline?: ListSideline | null;
  choice?: ListChoice | null;
  /** The game's result ("1-0"), listed after the last move as on Chess.com; null when unfinished. */
  result?: string | null;
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
  side,
  lastBook,
  current,
  filter,
  onSelect,
}: {
  ply?: PlyReview;
  side: "white" | "black";
  lastBook: number | null;
  current: number;
  filter: MoveFilter;
  onSelect: (n: number) => void;
}) {
  // A game started by Black shows "…" for White's move; Black's cell after the last move stays empty.
  if (!ply) return <td className="move-cell move-cell--empty">{side === "white" ? "…" : null}</td>;
  const active = ply.ply === current;
  const icon = showsListIcon(ply, lastBook);
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

/** The `data-sideline` key of an explored move: its path, as "0.1.0". */
const pathKey = (path: number[]) => path.join(".");

/**
 * One explored line in PGN order, as Chess.com lists it: "2. e4 d5 (2... e5) 3. exd5". A move's
 * variations follow the move they replace, in brackets, and the line goes on after them.
 */
function SidelineRow({ sideline, root }: { sideline: ListSideline; root: number }) {
  const { branchFen, shown, onSelect, classOf } = sideline;
  const shownKey = shown ? pathKey(shown) : null;

  const move = (node: ExploreNode, path: number[], moves: PlayedMove[], opens: boolean): ReactNode => {
    const label = moveLabel(branchFen, path.length - 1, opens);
    const key = pathKey(path);
    const active = key === shownKey;
    const cls = classOf?.(moves) ?? null;
    const meta = cls ? CLASSIFICATIONS[cls] : null;
    return (
      <Fragment key={key}>
        {label.number && <span className="sideline__number">{label.number}</span>}
        <button
          type="button"
          className={`move move--sideline ${active ? "move--active" : ""}`}
          aria-current={active ? "step" : undefined}
          data-sideline={key}
          onClick={() => onSelect(path)}
          title={meta?.label}
          style={meta ? { color: meta.color } : undefined}
        >
          {cls && (
            <span className="move__icon">
              <ClassificationIcon value={cls} size={16} />
            </span>
          )}
          <San san={node.move.san} side={label.mover} />
        </button>
      </Fragment>
    );
  };

  /**
   * The moves after `node`: its first continuation, the alternatives to that in brackets, then on.
   * `opens` numbers the first one, as after a variation ("3... Nf6").
   */
  const after = (node: ExploreNode, path: number[], moves: PlayedMove[], opens: boolean, out: ReactNode[]) => {
    const [main, ...alternatives] = node.next;
    if (!main) return;
    const mainPath = [...path, 0];
    const mainMoves = [...moves, main.move];
    out.push(move(main, mainPath, mainMoves, opens));
    alternatives.forEach((alt, i) => {
      const altPath = [...path, i + 1];
      const altMoves = [...moves, alt.move];
      out.push(
        <span key={`(${pathKey(altPath)}`} className="sideline__variation">
          (
        </span>,
      );
      out.push(move(alt, altPath, altMoves, true));
      after(alt, altPath, altMoves, false, out);
      out.push(
        <span key={`)${pathKey(altPath)}`} className="sideline__variation">
          )
        </span>,
      );
    });
    after(main, mainPath, mainMoves, alternatives.length > 0, out);
  };

  const first = sideline.lines[root]!;
  const out: ReactNode[] = [move(first, [root], [first.move], true)];
  after(first, [root], [first.move], false, out);
  return (
    <tr className="move-table__sideline">
      <td colSpan={3}>
        <div className="sideline" aria-label="Your sideline">
          {out}
        </div>
      </td>
    </tr>
  );
}

/**
 * Chess.com's variation picker, under the shown move: ↑ ↓ or the mouse choose, → Enter or a click
 * follow. A click elsewhere or scrolling the list closes it.
 */
function VariationPicker({ choice, anchor, side }: { choice: ListChoice; anchor: HTMLElement | null; side: "white" | "black" }) {
  const box = useRef<HTMLDivElement>(null);
  const [at, setAt] = useState<{ top: number; left: number } | null>(null);
  useLayoutEffect(() => {
    if (!anchor) return;
    const r = anchor.getBoundingClientRect();
    setAt({ top: r.bottom, left: r.left });
  }, [anchor]);
  const { onClose } = choice;
  useEffect(() => {
    const outside = (e: MouseEvent) => {
      if (!box.current?.contains(e.target as Node)) onClose();
    };
    document.addEventListener("mousedown", outside);
    return () => document.removeEventListener("mousedown", outside);
  }, [onClose]);
  if (!at) return null;
  return (
    <div className="variation-picker" ref={box} role="listbox" aria-label="Choose a variation" style={at}>
      {choice.options.map((m, i) => (
        <div
          key={m.uci}
          role="option"
          aria-selected={i === choice.index}
          className={`variation-picker__row ${i === choice.index ? "is-selected" : ""}`}
          onMouseEnter={() => choice.onHover(i)}
          onClick={() => choice.onPick(i)}
        >
          <span className="variation-picker__arrow" aria-hidden="true">
            {i === choice.index && "→"}
          </span>
          <San san={m.san} side={side} />
        </div>
      ))}
    </div>
  );
}

export function MoveList({ plies, current, filter, onSelect, sideline, choice, result }: Props) {
  const container = useRef<HTMLDivElement>(null);
  const shownKey = sideline?.shown ? pathKey(sideline.shown) : null;
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);

  // Keep the current move visible inside the list without scrolling the page.
  useEffect(() => {
    const box = container.current;
    const el = box?.querySelector<HTMLElement>(shownKey ? `[data-sideline="${shownKey}"]` : `[data-ply="${current}"]`);
    if (!box) return;
    if (!el) {
      if (current === 0) box.scrollTop = 0;
      return;
    }
    // A game move scrolls by its row; a sideline move by itself, as its row may wrap.
    const row = (shownKey ? el : (el.closest("tr") ?? el)).getBoundingClientRect();
    const view = box.getBoundingClientRect();
    // Leave one row of context above/below the current move.
    if (row.top < view.top + row.height) box.scrollTop -= view.top + row.height - row.top;
    else if (row.bottom > view.bottom - row.height) box.scrollTop += row.bottom - (view.bottom - row.height);
  }, [current, shownKey]);

  // The picker hangs under the shown move, found once the list has scrolled to it.
  const open = !!choice;
  useLayoutEffect(() => {
    setAnchor(open && shownKey ? (container.current?.querySelector<HTMLElement>(`[data-sideline="${shownKey}"]`) ?? null) : null);
  }, [open, shownKey]);

  // The lines are listed under the row of the game move they replace (the last row at the end).
  const all = rows(plies);
  const lastBook = lastBookPly(plies);
  const replaced = sideline ? Math.min(sideline.fromPly + 1, plies.length) : -1;
  const at = sideline ? all.findIndex((r) => r.white?.ply === replaced || r.black?.ply === replaced) : -1;
  const lineRows = sideline?.lines.map((_, i) => <SidelineRow key={i} sideline={sideline} root={i} />);
  const inLine = !!shownKey;
  // The picker lists moves of the side to move after the shown move.
  const pickerSide = sideline?.shown ? moveLabel(sideline.branchFen, sideline.shown.length, false).mover : "white";

  return (
    <div className="move-list" ref={container} onScroll={choice?.onClose}>
      <table className="move-table">
        <tbody>
          {sideline && at < 0 && lineRows}
          {all.map((row, i) => (
            <Fragment key={`${row.number}-${row.white?.ply ?? row.black?.ply}`}>
              <tr className={i % 2 ? "is-striped" : undefined}>
                <th scope="row" className="move-table__number">
                  {row.number}.
                </th>
                <MoveCell ply={row.white} side="white" lastBook={lastBook} current={inLine ? -1 : current} filter={filter} onSelect={onSelect} />
                <MoveCell ply={row.black} side="black" lastBook={lastBook} current={inLine ? -1 : current} filter={filter} onSelect={onSelect} />
              </tr>
              {sideline && i === at && lineRows}
            </Fragment>
          ))}
          {result && (
            <tr className="move-table__result">
              <td colSpan={3}>{result}</td>
            </tr>
          )}
        </tbody>
      </table>
      {choice && <VariationPicker choice={choice} anchor={anchor} side={pickerSide} />}
    </div>
  );
}
