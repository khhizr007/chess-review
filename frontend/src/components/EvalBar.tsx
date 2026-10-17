import type { Eval } from "../api/generated/Eval";
import { favouredSide, formatEval, formatEvalShort, percent } from "../lib/format";

interface Props {
  /** White's expected points (0–1) for the shown position, from the backend. */
  whiteExpectedPoints: number;
  evaluation: Eval;
  flipped: boolean;
}

/** Vertical evaluation bar; White's share grows from White's side of the board. */
export function EvalBar({ whiteExpectedPoints, evaluation, flipped }: Props) {
  const white = Math.min(1, Math.max(0, whiteExpectedPoints));
  const favoured = favouredSide(evaluation);
  return (
    <div
      className={`eval-bar ${flipped ? "eval-bar--flipped" : ""}`}
      role="meter"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(white * 100)}
      aria-label={`Evaluation ${formatEval(evaluation)}, White expected score ${percent(white)}`}
      title={`${formatEval(evaluation)} · White ${percent(white)}`}
    >
      <div className="eval-bar__white" style={{ height: `${white * 100}%` }} />
      <span className={`eval-bar__label eval-bar__label--${favoured === "black" ? "black" : "white"}`}>
        {formatEvalShort(evaluation)}
      </span>
    </div>
  );
}
