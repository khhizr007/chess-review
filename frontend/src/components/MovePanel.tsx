import type { Line } from "../api/generated/Line";
import type { PlyReview } from "../api/generated/PlyReview";
import { CLASSIFICATIONS, MISS_KINDS, PHASES, formatEval, moveNumberLabel, percent, sideLabel } from "../lib/format";
import { ClassificationBadge } from "./ClassificationBadge";

export interface LinePreview {
  /** Lines from before the played move ("alternatives") or from the shown position ("next"). */
  source: "alternatives" | "next";
  /** Index of the engine line and of the move within it being shown. */
  line: number;
  move: number;
}

interface Props {
  /** The move that led to the shown position (undefined at the start). */
  ply?: PlyReview;
  /** Analysis of the shown position (its engine lines), if any. */
  next?: PlyReview;
  preview: LinePreview | null;
  onPreview: (preview: LinePreview | null) => void;
}

function LineView({
  source,
  line,
  index,
  firstMoveNumber,
  firstMover,
  preview,
  onPreview,
}: {
  source: LinePreview["source"];
  line: Line;
  index: number;
  firstMoveNumber: number;
  firstMover: "white" | "black";
  preview: LinePreview | null;
  onPreview: (p: LinePreview | null) => void;
}) {
  let number = firstMoveNumber;
  let mover = firstMover;
  return (
    <li className="line">
      <span className={`line__eval ${line.eval.kind === "mate" || line.eval.cp >= 0 ? "is-white" : "is-black"}`}>
        {formatEval(line.eval)}
      </span>
      <span className="line__moves">
        {line.moves.map((m, i) => {
          const label = mover === "white" ? `${number}.` : i === 0 ? `${number}…` : "";
          const active = preview?.source === source && preview.line === index && preview.move === i;
          const el = (
            <button
              key={m.uci + i}
              type="button"
              className={`line__move ${active ? "is-active" : ""}`}
              onClick={() => onPreview(active ? null : { source, line: index, move: i })}
              aria-pressed={active}
            >
              {label && <span className="muted">{label}</span>}
              {m.san}
            </button>
          );
          if (mover === "black") number += 1;
          mover = mover === "white" ? "black" : "white";
          return el;
        })}
      </span>
      <span className="muted small line__depth">d{line.depth}</span>
    </li>
  );
}

export function MovePanel({ ply, next, preview, onPreview }: Props) {
  const nextMover = next?.mover ?? (ply ? (ply.mover === "white" ? "black" : "white") : "white");

  return (
    <section className="card move-panel" aria-live="polite">
      {ply ? (
        <>
          <header className="move-panel__header">
            <ClassificationBadge value={ply.classification} size="m" />
            <div>
              <h2 className="move-panel__title">
                {moveNumberLabel(ply.move_number, ply.mover)} {ply.san}{" "}
                {ply.classification && (
                  <span style={{ color: CLASSIFICATIONS[ply.classification].color }}>
                    {CLASSIFICATIONS[ply.classification].label.toLowerCase()}
                  </span>
                )}
              </h2>
              <p className="muted small">
                {sideLabel(ply.mover)} · {PHASES[ply.phase]}
                {ply.miss_kind && ` · ${MISS_KINDS[ply.miss_kind]}`}
                {ply.is_key_moment && " · key moment"}
              </p>
            </div>
          </header>

          <dl className="stats">
            <div>
              <dt>Evaluation</dt>
              <dd>
                {formatEval(ply.eval_before)} → <strong>{formatEval(ply.eval_after)}</strong>
              </dd>
            </div>
            <div>
              <dt>{sideLabel(ply.mover)} expected score</dt>
              <dd>
                {percent(ply.ep_before)} → <strong>{percent(ply.ep_after)}</strong>
              </dd>
            </div>
            <div>
              <dt>Lost vs best</dt>
              <dd>{ply.ep_lost < 0.0005 ? "—" : `${(ply.ep_lost * 100).toFixed(1)} pts`}</dd>
            </div>
            <div>
              <dt>Move accuracy</dt>
              <dd>{ply.accuracy == null ? "—" : Math.max(0, ply.accuracy).toFixed(0)}</dd>
            </div>
          </dl>

          {ply.show_best_move && ply.best_move && (
            <p className="best-move">
              Best was <strong>{ply.best_move.san}</strong>
              {ply.eval_best && <span className="muted"> ({formatEval(ply.eval_best)})</span>}
            </p>
          )}

          {ply.explanation.length > 0 && (
            <div className="facts">
              <h3 className="facts__title">
                Engine facts <span className="tag">generated</span>
              </h3>
              <ul>
                {ply.explanation.map((s) => (
                  <li key={s}>{s}</li>
                ))}
              </ul>
            </div>
          )}

          {ply.pgn_comment && (
            <blockquote className="pgn-comment">
              <span className="muted small">PGN comment</span>
              <p>{ply.pgn_comment}</p>
            </blockquote>
          )}

          {ply.lines.length > 0 && (
            <div className="lines">
              <h3 className="facts__title">Alternatives to {ply.san}</h3>
              <ol>
                {ply.lines.map((line, i) => (
                  <LineView
                    key={line.rank}
                    source="alternatives"
                    line={line}
                    index={i}
                    firstMoveNumber={ply.move_number}
                    firstMover={ply.mover}
                    preview={preview}
                    onPreview={onPreview}
                  />
                ))}
              </ol>
            </div>
          )}
        </>
      ) : (
        <header className="move-panel__header">
          <div>
            <h2 className="move-panel__title">Starting position</h2>
            <p className="muted small">Use the arrow keys or the move list to step through the game.</p>
          </div>
        </header>
      )}

      {next && next.lines.length > 0 && (
        <div className="lines lines--next">
          <h3 className="facts__title">Engine lines for {sideLabel(nextMover)} here</h3>
          <ol>
            {next.lines.map((line, i) => (
              <LineView
                key={line.rank}
                source="next"
                line={line}
                index={i}
                firstMoveNumber={next.move_number}
                firstMover={next.mover}
                preview={preview}
                onPreview={onPreview}
              />
            ))}
          </ol>
        </div>
      )}
    </section>
  );
}
