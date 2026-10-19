import { Fragment, type ReactNode } from "react";
import type { Eval } from "../api/generated/Eval";
import type { Line } from "../api/generated/Line";
import type { PlyReview } from "../api/generated/PlyReview";
import type { Sentence } from "../api/generated/Sentence";
import type { Side } from "../api/generated/Side";
import { favouredSide, formatEval, formatEvalPill, moveNumberLabel, percent, sideLabel } from "../lib/format";
import type { CoachMessage } from "../lib/coach";
import { ClassificationIcon } from "./ClassificationIcon";
import { PieceIcon, San } from "./PieceIcon";

/* ------------------------------------------------------------------ shared bits */

export function CoachAvatar() {
  return (
    <span className="coach__avatar" aria-hidden="true">
      <PieceIcon kind="n" side="white" size={44} />
    </span>
  );
}

export function EvalPill({ value }: { value: Eval }) {
  const black = favouredSide(value) === "black";
  return (
    <span className={`eval-pill ${black ? "eval-pill--black" : "eval-pill--white"}`} title={`Evaluation ${formatEval(value)}`}>
      {formatEvalPill(value)}
    </span>
  );
}

/** Avatar plus white speech bubble, as in the reference coach. */
export function CoachBubble({ children }: { children: ReactNode }) {
  return (
    <div className="coach">
      <CoachAvatar />
      <div className="coach__bubble">
        {children}
      </div>
    </div>
  );
}

export interface CoachAction {
  label: string;
  icon?: ReactNode;
  onClick: () => void;
  primary?: boolean;
  pressed?: boolean;
  hint?: string;
}

export function CoachActions({ actions }: { actions: CoachAction[] }) {
  return (
    <div
      className={`coach-actions ${actions.length >= 4 ? "is-dense" : ""}`}
      style={{ gridTemplateColumns: actions.map((a) => (a.primary && actions.length < 4 ? "1.35fr" : "1fr")).join(" ") }}
    >
      {actions.map((a) => (
        <button
          key={a.label}
          type="button"
          className={`coach-btn ${a.primary ? "coach-btn--primary" : ""} ${a.pressed ? "is-pressed" : ""}`}
          onClick={a.onClick}
          aria-pressed={a.pressed}
          title={a.hint}
        >
          {a.icon}
          {a.label}
        </button>
      ))}
    </div>
  );
}

/* ------------------------------------------------------------------ move verdict */

export function MoveVerdict({ ply, message }: { ply: PlyReview; message: CoachMessage }) {
  return (
    <>
      <div className="coach__head">
        <ClassificationIcon value={message.classification} size={26} />
        <h2 className="coach__title">
          <San san={ply.san} side={ply.mover} /> {message.verdict}
        </h2>
        <EvalPill value={ply.eval_after} />
      </div>
      {(message.advice.length > 0 || message.sentences.length > 0 || message.swing) && (
        <div className="coach__body">
          {message.sentences.map((s, i) => (
            <CoachSentence key={i} sentence={s} />
          ))}
          {message.advice.map((s) => (
            <p key={s}>{s}</p>
          ))}
          {message.swing && <WinChance side={ply.mover} before={message.swing.before} after={message.swing.after} />}
        </div>
      )}
      {ply.pgn_comment && <blockquote className="coach__comment">“{ply.pgn_comment}”</blockquote>}
    </>
  );
}

/** A coach sentence; fragments that name a move or piece shown on the board are emphasised. */
function CoachSentence({ sentence }: { sentence: Sentence }) {
  return (
    <p>
      {sentence.fragments.map((f, i) =>
        f.arrows.length > 0 || f.squares.length > 0 ? (
          <strong key={i} className="coach__ref">
            {f.text}
          </strong>
        ) : (
          <Fragment key={i}>{f.text}</Fragment>
        ),
      )}
    </p>
  );
}

function WinChance({ side, before, after }: { side: Side; before: number; after: number }) {
  const up = after > before;
  return (
    <p className={`win-chance ${up ? "is-up" : "is-down"}`}>
      {sideLabel(side)}'s winning chances <strong>{percent(before)}</strong>
      <span className="win-chance__arrow" aria-label={up ? "rose to" : "fell to"}>
        {up ? "↗" : "↘"}
      </span>
      <strong>{percent(after)}</strong>
    </p>
  );
}

/* ------------------------------------------------------------------ engine lines */

export interface LinePreview {
  /** Lines from before the played move ("alternatives"), from the shown position ("next"), or the best move alone. */
  source: "alternatives" | "next";
  line: number;
  move: number;
}

export function LineView({
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
  firstMover: Side;
  preview: LinePreview | null;
  onPreview: (p: LinePreview | null) => void;
}) {
  let number = firstMoveNumber;
  let mover = firstMover;
  return (
    <li className="line">
      <EvalPill value={line.eval} />
      <span className="line__moves">
        {line.moves.map((m, i) => {
          const label = mover === "white" ? `${number}.` : i === 0 ? `${number}…` : "";
          const active = preview?.source === source && preview.line === index && preview.move === i;
          const side = mover;
          const el = (
            <button
              key={m.uci + i}
              type="button"
              className={`line__move ${active ? "is-active" : ""}`}
              onClick={() => onPreview(active ? null : { source, line: index, move: i })}
              aria-pressed={active}
            >
              {label && <span className="line__num">{label}</span>}
              <San san={m.san} side={side} />
            </button>
          );
          if (mover === "black") number += 1;
          mover = mover === "white" ? "black" : "white";
          return el;
        })}
      </span>
    </li>
  );
}

/* ------------------------------------------------------------------ explain drawer */

/** The numbers behind the verdict, shown on demand so the coach stays readable. */
export function MoveDetails({
  ply,
  facts,
  preview,
  onPreview,
}: {
  ply: PlyReview;
  facts: string[];
  preview: LinePreview | null;
  onPreview: (p: LinePreview | null) => void;
}) {
  const lost = ply.ep_lost < 0.0005 ? "0" : (ply.ep_lost * 100).toFixed(1);
  return (
    <section className="details" aria-label="Move details">
      <dl className="details__stats">
        <div>
          <dt>Eval</dt>
          <dd>
            {formatEvalPill(ply.eval_before)} <span className="muted">→</span> <strong>{formatEvalPill(ply.eval_after)}</strong>
          </dd>
        </div>
        <div>
          <dt>Win %</dt>
          <dd>
            {percent(ply.ep_before)} <span className="muted">→</span> <strong>{percent(ply.ep_after)}</strong>
          </dd>
        </div>
        <div>
          <dt>Lost</dt>
          <dd className={ply.ep_lost >= 0.05 ? "is-bad" : undefined}>{lost}%</dd>
        </div>
        <div>
          <dt>Accuracy</dt>
          <dd>{ply.accuracy == null ? "—" : Math.max(0, ply.accuracy).toFixed(0)}</dd>
        </div>
      </dl>

      {facts.length > 0 && (
        <ul className="details__facts">
          {facts.map((f) => (
            <li key={f}>{f}</li>
          ))}
        </ul>
      )}

      {ply.lines.length > 0 && (
        <div className="details__lines">
          <h3 className="section-label">
            Engine lines instead of{" "}
            <span className="section-label__san">
              {moveNumberLabel(ply.move_number, ply.mover)} {ply.san}
            </span>
          </h3>
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
          <p className="details__hint">Click a move to see it on the board. Depth {ply.lines[0]?.depth}.</p>
        </div>
      )}
    </section>
  );
}
