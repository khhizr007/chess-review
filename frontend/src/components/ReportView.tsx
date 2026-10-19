import { useMemo } from "react";
import type { Classification } from "../api/generated/Classification";
import type { Phase } from "../api/generated/Phase";
import type { ReviewResult } from "../api/generated/ReviewResult";
import type { Side } from "../api/generated/Side";
import { reportSummary, playerName } from "../lib/coach";
import { CLASSIFICATIONS, PHASES, REPORT_ORDER, formatAccuracy, moveNumberLabel } from "../lib/format";
import { firstPlyOf } from "../lib/review";
import { gameTakeaways } from "../lib/takeaways";
import { ClassificationIcon } from "./ClassificationIcon";
import { CoachBubble } from "./Coach";
import { EvalGraph } from "./EvalGraph";
import { PieceIcon, San } from "./PieceIcon";

interface Props {
  review: ReviewResult;
  current: number;
  onSelect: (ply: number) => void;
  /** Opens move-by-move review at a ply. */
  onReviewAt: (ply: number) => void;
  onStart: () => void;
  onNewGame: () => void;
}

/** Phase accuracy shown as the classification it would earn, like the reference's phase icons. */
function phaseGrade(acc: number): Classification {
  if (acc >= 90) return "best";
  if (acc >= 80) return "excellent";
  if (acc >= 70) return "good";
  if (acc >= 60) return "inaccuracy";
  if (acc >= 50) return "mistake";
  return "blunder";
}

const PHASE_KEYS: Phase[] = ["opening", "middlegame", "endgame"];

export function ReportView({ review, current, onSelect, onReviewAt, onStart, onNewGame }: Props) {
  const { white, black } = review;
  const takeaways = useMemo(() => gameTakeaways(review), [review]);
  const count =(side: Side, c: Classification) =>
    (side === "white" ? white : black).tally.find((t) => t.classification === c)?.count ?? 0;

  const countCell = (side: Side, c: Classification) => {
    const n = count(side, c);
    const target = n > 0 ? firstPlyOf(review, side, c) : null;
    return (
      <td className="report__count">
        {target ? (
          <button
            type="button"
            className="report__count-btn"
            style={{ color: CLASSIFICATIONS[c].color }}
            onClick={() => onReviewAt(target)}
            title={`Go to ${playerName(review, side)}'s first ${CLASSIFICATIONS[c].label.toLowerCase()}`}
          >
            {n}
          </button>
        ) : (
          <span className="report__zero">{n}</span>
        )}
      </td>
    );
  };

  return (
    <div className="report">
      <div className="panel__scroll">
        <CoachBubble>
          <div className="coach__body coach__body--report">
            {reportSummary(review).map((s) => (
              <p key={s}>{s}</p>
            ))}
          </div>
        </CoachBubble>

        {takeaways.length > 0 && (
          <section className="takeaways" aria-label="Takeaways">
            <h2 className="section-label">Takeaways</h2>
            <ul>
              {takeaways.map((t) => (
                <li key={`${t.side}-${t.text}`} className={`takeaway takeaway--${t.kind}`}>
                  <span className={`avatar avatar--${t.side} takeaway__side`} title={playerName(review, t.side)}>
                    <PieceIcon kind="p" side={t.side} size={18} />
                    <span className="visually-hidden">{playerName(review, t.side)}:</span>
                  </span>
                  <span className="takeaway__text">{t.text}</span>
                  <span className="takeaway__moves">
                    {t.plies.map((p) => {
                      const m = review.plies[p - 1];
                      if (!m) return null;
                      return (
                        <button
                          key={p}
                          type="button"
                          className="takeaway__move"
                          onClick={() => onReviewAt(p)}
                          title={`Review ${moveNumberLabel(m.move_number, m.mover)} ${m.san}`}
                        >
                          <span className="line__num">{moveNumberLabel(m.move_number, m.mover)}</span>
                          <San san={m.san} side={m.mover} />
                        </button>
                      );
                    })}
                  </span>
                </li>
              ))}
            </ul>
          </section>
        )}

        <div className="report__graph">
          <EvalGraph
            points={review.graph}
            plies={review.plies}
            current={current}
            phaseStarts={review.phase_starts}
            onSelect={onSelect}
            height={72}
          />
        </div>

        <table className="report__table">
          <colgroup>
            <col className="report__col-label" />
            <col className="report__col-side" />
            <col className="report__col-side" />
          </colgroup>
          <thead>
            <tr className="report__players">
              <th scope="col">
                <span className="visually-hidden">Players</span>
              </th>
              {(["white", "black"] as const).map((side) => (
                <th scope="col" key={side}>
                  <span className={`avatar avatar--${side} avatar--lg`} aria-hidden="true">
                    <PieceIcon kind="p" side={side} size={42} />
                  </span>
                  <span className="report__name">{playerName(review, side)}</span>
                  {(side === "white" ? white : black).elo != null && (
                    <span className="report__elo">{(side === "white" ? white : black).elo}</span>
                  )}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            <tr className="report__accuracy">
              <th scope="row">Accuracy</th>
              <td>
                <span className="acc-box acc-box--white">{formatAccuracy(white.accuracy.overall)}</span>
              </td>
              <td>
                <span className="acc-box acc-box--black">{formatAccuracy(black.accuracy.overall)}</span>
              </td>
            </tr>
          </tbody>
          <tbody className="report__tally">
            {REPORT_ORDER.map((c) => {
              const zero = count("white", c) + count("black", c) === 0;
              return (
                <tr key={c} className={zero ? "is-zero" : ""}>
                  <th scope="row">
                    <span className="report__cls">
                      <ClassificationIcon value={c} size={20} />
                      {CLASSIFICATIONS[c].label}
                    </span>
                  </th>
                  {countCell("white", c)}
                  {countCell("black", c)}
                </tr>
              );
            })}
          </tbody>
          <tbody className="report__phases">
            {PHASE_KEYS.map((phase) => (
              <tr key={phase}>
                <th scope="row">{PHASES[phase]}</th>
                {[white, black].map((s) => {
                  const acc = s.accuracy[phase];
                  return (
                    <td key={s.side}>
                      {acc == null ? (
                        <span className="report__zero">—</span>
                      ) : (
                        <span className="report__phase" title={`${formatAccuracy(acc)} accuracy`}>
                          <ClassificationIcon value={phaseGrade(acc)} size={18} />
                          {acc.toFixed(0)}
                        </span>
                      )}
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>

        <details className="report__about">
          <summary>About this review</summary>
          <ul>
            <li>
              {review.engine},{" "}
              {review.settings.limit.kind === "depth"
                ? `depth ${review.settings.limit.depth}`
                : `${review.settings.limit.millis} ms per position`}
              , {review.settings.multipv} line(s). Win-probability rating: {review.rating_used}.
            </li>
            <li>Forced moves are counted as Best, as in the reference app.</li>
            {review.notes.map((n) => (
              <li key={n}>{n}</li>
            ))}
          </ul>
        </details>
      </div>

      <div className="panel__footer">
        <button type="button" className="cta" onClick={onStart}>
          Start Review
        </button>
        <button type="button" className="link-btn" onClick={onNewGame}>
          Review another game
        </button>
      </div>
    </div>
  );
}
