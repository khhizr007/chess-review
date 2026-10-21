import { Fragment, type ReactNode } from "react";
import type { Eval } from "../api/generated/Eval";
import type { Line } from "../api/generated/Line";
import type { PlyReview } from "../api/generated/PlyReview";
import type { Sentence } from "../api/generated/Sentence";
import type { ShowLine } from "../api/generated/ShowLine";
import type { Side } from "../api/generated/Side";
import { favouredSide, formatEval, formatEvalPill, moveNumberLabel, percent, sideLabel } from "../lib/format";
import type { CoachMessage, CoachMood } from "../lib/coach";
import { ENGINE_NAME } from "../lib/engine/controller";
import type { LiveEngine } from "../hooks/useLiveEngine";
import { ClassificationIcon } from "./ClassificationIcon";
import { Icons } from "./icons";
import { PieceIcon, San } from "./PieceIcon";

/* ------------------------------------------------------------------ shared bits */

/** The coach's face; its mood is drawn with CSS only (tilt, glow, colour). */
export function CoachAvatar({ mood = "neutral" }: { mood?: CoachMood }) {
  return (
    <span className={`coach__avatar coach__avatar--${mood}`} aria-hidden="true" key={mood}>
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
export function CoachBubble({ children, mood }: { children: ReactNode; mood?: CoachMood }) {
  return (
    <div className="coach">
      <CoachAvatar mood={mood} />
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

/** A fragment of a coach sentence, by position. */
export interface FragmentRef {
  sentence: number;
  fragment: number;
}

/** Board marking of sentence fragments: hover or focus shows one, a click or tap pins it. */
export interface FragmentMarking {
  /** The fragment whose arrows and squares are on the board. */
  active: FragmentRef | null;
  pinned: FragmentRef | null;
  onHover: (ref: FragmentRef | null) => void;
  onToggle: (ref: FragmentRef) => void;
}

export function MoveVerdict({
  ply,
  message,
  marking,
  onShowLine,
}: {
  ply: PlyReview;
  message: CoachMessage;
  marking?: FragmentMarking;
  /** Starts replaying one of the coach's show-lines; omitted where lines cannot be shown. */
  onShowLine?: (index: number) => void;
}) {
  const lines = onShowLine ? (ply.coach?.show_lines ?? []) : [];
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
            <CoachSentence key={i} index={i} sentence={s} marking={marking} />
          ))}
          {message.advice.map((s) => (
            <p key={s}>{s}</p>
          ))}
          {message.swing && <WinChance side={ply.mover} before={message.swing.before} after={message.swing.after} />}
        </div>
      )}
      {lines.length > 0 && (
        <div className="coach__lines">
          {lines.map((l, i) => (
            <button key={`${l.label}-${i}`} type="button" className="coach__line-btn" onClick={() => onShowLine?.(i)}>
              {Icons.eye}
              {l.label}
            </button>
          ))}
        </div>
      )}
      {ply.pgn_comment && <blockquote className="coach__comment">“{ply.pgn_comment}”</blockquote>}
    </>
  );
}

const sameRef = (a: FragmentRef | null | undefined, b: FragmentRef) => !!a && a.sentence === b.sentence && a.fragment === b.fragment;

/**
 * A coach sentence. Fragments that name a move or piece are buttons: hovering, focusing or
 * tapping one draws its arrows and squares on the board.
 */
function CoachSentence({ sentence, index, marking }: { sentence: Sentence; index: number; marking?: FragmentMarking }) {
  return (
    <p>
      {sentence.fragments.map((f, i) => {
        if (f.arrows.length === 0 && f.squares.length === 0) return <Fragment key={i}>{f.text}</Fragment>;
        if (!marking) {
          return (
            <strong key={i} className="coach__ref">
              {f.text}
            </strong>
          );
        }
        const ref = { sentence: index, fragment: i };
        const pinned = sameRef(marking.pinned, ref);
        return (
          <button
            key={i}
            type="button"
            className={`coach__ref coach__ref--live ${sameRef(marking.active, ref) ? "is-active" : ""}`}
            aria-pressed={pinned}
            title="Show on the board"
            onPointerEnter={(e) => e.pointerType === "mouse" && marking.onHover(ref)}
            onPointerLeave={(e) => e.pointerType === "mouse" && marking.onHover(null)}
            onFocus={() => marking.onHover(ref)}
            onBlur={() => marking.onHover(null)}
            onClick={() => {
              // A second tap unpins; drop the focus marking too so the board clears on touch.
              if (pinned) marking.onHover(null);
              marking.onToggle(ref);
            }}
          >
            {f.text}
          </button>
        );
      })}
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
  /**
   * Engine lines from before the played move ("alternatives") or from the shown position ("next"),
   * or one of the coach's show-lines for the move ("coach").
   */
  source: "alternatives" | "next" | "coach";
  line: number;
  /** Index of the move shown; -1 is a coach line's start position. */
  move: number;
}

/** Moves of a coach show-line as clickable chips, numbered from its start position. */
export function ShowLineMoves({ line, move, onSelect }: { line: ShowLine; move: number; onSelect: (move: number) => void }) {
  const fields = line.start_fen.split(" ");
  let mover: Side = fields[1] === "b" ? "black" : "white";
  let number = Number(fields[5]) || 1;
  return (
    <div className="show-line__moves">
      {line.moves.map((m, i) => {
        const label = mover === "white" ? `${number}.` : i === 0 ? `${number}…` : "";
        const side = mover;
        if (mover === "black") number += 1;
        mover = mover === "white" ? "black" : "white";
        return (
          <button
            key={m.uci + i}
            type="button"
            className={`line__move ${i === move ? "is-active" : ""}`}
            onClick={() => onSelect(i)}
            aria-current={i === move ? "step" : undefined}
          >
            {label && <span className="line__num">{label}</span>}
            <San san={m.san} side={side} />
          </button>
        );
      })}
    </div>
  );
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

/**
 * The browser engine's lines for a sideline position, with the depth reached and where the numbers
 * come from (docs/explore-mode.md §5.4). Clicking a move plays the line up to it.
 */
export function LiveLines({
  fen,
  engine,
  onPlay,
}: {
  fen: string;
  engine: LiveEngine;
  onPlay: (line: number, move: number) => void;
}) {
  const fields = fen.split(" ");
  const mover: Side = fields[1] === "b" ? "black" : "white";
  const number = Number(fields[5]) || 1;
  const { analysis, status, target } = engine;
  const searching = !!analysis && !engine.finished;
  return (
    <section className="details live-lines" aria-label="Engine lines">
      <h3 className="section-label live-lines__head">
        Engine
        <span className="live-lines__meta" aria-live="off">
          {analysis ? `depth ${analysis.depth}${searching ? `/${target}` : ""}` : status === "error" ? "unavailable" : "starting…"}
          {" · "}
          {ENGINE_NAME}, in your browser
        </span>
        {searching && <span className="live-lines__spinner" aria-hidden="true" />}
      </h3>
      {status === "error" ? (
        <p className="details__hint">The engine could not start in this browser{engine.error ? ` (${engine.error})` : ""}.</p>
      ) : analysis ? (
        <ol>
          {analysis.lines.map((line, i) => (
            <LineView
              key={line.rank}
              source="next"
              line={line}
              index={i}
              firstMoveNumber={number}
              firstMover={mover}
              preview={null}
              onPreview={(p) => p && onPlay(i, p.move)}
            />
          ))}
        </ol>
      ) : (
        <p className="details__hint">Loading the engine…</p>
      )}
      {analysis && (
        <p className="details__hint">
          Click a move to play the line up to it.
          {engine.deeper && (
            <button type="button" className="link-btn live-lines__deeper" onClick={engine.deeper}>
              Go deeper
            </button>
          )}
        </p>
      )}
    </section>
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
