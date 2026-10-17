import type { ReviewResult } from "../api/generated/ReviewResult";
import type { SideSummary } from "../api/generated/SideSummary";
import { CLASSIFICATIONS, formatAccuracy, moveNumberLabel } from "../lib/format";
import { ClassificationBadge } from "./ClassificationBadge";
import { EvalGraph } from "./EvalGraph";

interface Props {
  review: ReviewResult;
  current: number;
  onSelect: (ply: number) => void;
}

function PlayerCard({ s }: { s: SideSummary }) {
  return (
    <div className={`player player--${s.side}`}>
      <div className="player__name">
        <span className={`dot dot--${s.side}`} aria-hidden="true" />
        <strong>{s.name ?? (s.side === "white" ? "White" : "Black")}</strong>
        {s.elo != null && <span className="muted"> ({s.elo})</span>}
      </div>
      <div className="player__accuracy">
        <span className="big-number">{formatAccuracy(s.accuracy.overall)}</span>
        <span className="muted small">accuracy</span>
      </div>
      <dl className="phase-acc">
        <div>
          <dt>Opening</dt>
          <dd>{formatAccuracy(s.accuracy.opening)}</dd>
        </div>
        <div>
          <dt>Middlegame</dt>
          <dd>{formatAccuracy(s.accuracy.middlegame)}</dd>
        </div>
        <div>
          <dt>Endgame</dt>
          <dd>{formatAccuracy(s.accuracy.endgame)}</dd>
        </div>
      </dl>
    </div>
  );
}

function MomentList({
  title,
  plies,
  review,
  current,
  onSelect,
}: {
  title: string;
  plies: number[];
  review: ReviewResult;
  current: number;
  onSelect: (ply: number) => void;
}) {
  return (
    <div className="moments">
      <h3 className="facts__title">{title}</h3>
      {plies.length === 0 ? (
        <p className="muted small">None.</p>
      ) : (
        <ul>
          {plies.map((n) => {
            const p = review.plies[n - 1];
            if (!p) return null;
            return (
              <li key={n}>
                <button
                  type="button"
                  className={`move move--wide ${n === current ? "move--active" : ""}`}
                  onClick={() => onSelect(n)}
                >
                  <span className="muted move__number">{moveNumberLabel(p.move_number, p.mover)}</span>
                  <ClassificationBadge value={p.classification} />
                  <span className="move__san">{p.san}</span>
                  {p.classification && <span className="muted small">{CLASSIFICATIONS[p.classification].label}</span>}
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}

export function SummaryPanel({ review, current, onSelect }: Props) {
  const { white, black } = review;
  return (
    <section className="summary" aria-label="Game summary">
      <div className="card summary__players">
        <PlayerCard s={white} />
        <PlayerCard s={black} />
      </div>

      <div className="card summary__graph">
        <h2 className="card__title">Evaluation</h2>
        <EvalGraph points={review.graph} current={current} phaseStarts={review.phase_starts} onSelect={onSelect} />
      </div>

      <div className="card summary__tally">
        <h2 className="card__title">Move quality</h2>
        <table className="tally">
          <thead>
            <tr>
              <th scope="col">
                <span className="visually-hidden">Classification</span>
              </th>
              <th scope="col">{white.name ?? "White"}</th>
              <th scope="col">{black.name ?? "Black"}</th>
            </tr>
          </thead>
          <tbody>
            {white.tally.map((entry, i) => {
              const b = black.tally[i]?.count ?? 0;
              const meta = CLASSIFICATIONS[entry.classification];
              return (
                <tr key={entry.classification} className={entry.count + b === 0 ? "is-zero" : ""}>
                  <th scope="row">
                    <ClassificationBadge value={entry.classification} /> {meta.label}
                  </th>
                  <td>{entry.count}</td>
                  <td>{b}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
        <p className="muted small">Forced moves are counted as Best, as in the reference app.</p>
      </div>

      <div className="card summary__moments">
        <MomentList
          title="Critical moments"
          plies={review.critical_moments}
          review={review}
          current={current}
          onSelect={onSelect}
        />
        <MomentList title="Key moments" plies={review.key_moments} review={review} current={current} onSelect={onSelect} />
        <MomentList
          title="Biggest swings"
          plies={review.biggest_swings}
          review={review}
          current={current}
          onSelect={onSelect}
        />
      </div>

      <details className="card summary__notes">
        <summary>About this review</summary>
        <ul className="small">
          <li>
            Engine: {review.engine},{" "}
            {review.settings.limit.kind === "depth"
              ? `depth ${review.settings.limit.depth}`
              : `${review.settings.limit.millis} ms per position`}
            , {review.settings.multipv} line(s). Win-probability rating: {review.rating_used}.
          </li>
          {review.notes.map((n) => (
            <li key={n}>{n}</li>
          ))}
        </ul>
      </details>
    </section>
  );
}
