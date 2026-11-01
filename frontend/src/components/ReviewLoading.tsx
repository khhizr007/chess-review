import type { GameSummary } from "../api/generated/GameSummary";
import type { Side } from "../api/generated/Side";
import { materialFromFen } from "../lib/material";
import { BoardView } from "./BoardView";
import { CoachBubble } from "./Coach";
import { EvalBar } from "./EvalBar";
import { LogoMark } from "./Logo";
import { PlayerStrip } from "./PlayerStrip";

const START_FEN = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

interface Props {
  /** The game being reviewed, from the import parse; null when the page was reopened mid-review. */
  game: GameSummary | null;
  /** Positions checked so far, or null while queued or putting the result together. */
  progress: { done: number; total: number } | null;
  /** What the coach says under the title. */
  status: string;
  onCancel?: () => void;
}

function ProgressBar({ done, total }: { done: number; total: number }) {
  const pct = Math.round((Math.min(done, total) / Math.max(1, total)) * 100);
  return (
    <div className="progress" role="progressbar" aria-label="Review progress" aria-valuemin={0} aria-valuemax={100} aria-valuenow={pct}>
      <div className="progress__track">
        <div className="progress__fill" style={{ width: `${pct}%` }} />
      </div>
      <span className="progress__pct">{pct}%</span>
    </div>
  );
}

/**
 * The review layout while the engine works, as in the reference: the game's final position, both
 * player strips and an empty eval bar straight away, with the coach and progress in the panel.
 */
export function ReviewLoading({ game, progress, status, onCancel }: Props) {
  const fen = game?.moves[game.moves.length - 1]?.fen ?? game?.start_fen ?? START_FEN;
  const material = materialFromFen(fen);
  const toMove: Side = fen.split(" ")[1] === "b" ? "black" : "white";
  const strip = (side: Side) => (
    <PlayerStrip
      side={side}
      name={(side === "white" ? game?.white : game?.black) ?? (side === "white" ? "White" : "Black")}
      elo={(side === "white" ? game?.white_elo : game?.black_elo) ?? null}
      material={material[side]}
      toMove={toMove === side}
    />
  );

  return (
    <div className="review review--loading">
      <section className="board-col" aria-label="Board">
        {strip("black")}
        <div className="board-row">
          <EvalBar whiteExpectedPoints={null} evaluation={null} flipped={false} />
          <BoardView fen={fen} flipped={false} arrows={[]} squareStyles={{}} />
        </div>
        {strip("white")}
      </section>

      <aside className="panel" aria-label="Game review">
        <header className="panel__header">
          <span className="icon-btn-spacer" />
          <h1 className="panel__title">
            <LogoMark size={26} />
            Game Review
          </h1>
          <span className="icon-btn-spacer" />
        </header>
        <div className="panel__scroll" aria-live="polite">
          <CoachBubble>
            <div className="coach__head">
              <h2 className="coach__title">Analyzing your game…</h2>
            </div>
            <div className="coach__body">
              <p>{status}</p>
            </div>
            <ProgressBar done={progress?.done ?? 0} total={progress?.total ?? 1} />
          </CoachBubble>
        </div>
        {onCancel && (
          <div className="panel__footer">
            <button type="button" className="link-btn" onClick={onCancel}>
              Cancel
            </button>
          </div>
        )}
      </aside>
    </div>
  );
}
