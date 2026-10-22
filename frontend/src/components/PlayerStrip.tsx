import type { Side } from "../api/generated/Side";
import { formatClock } from "../lib/clock";
import type { MaterialSide } from "../lib/material";
import { PieceIcon } from "./PieceIcon";

interface Props {
  side: Side;
  name: string;
  elo: number | null;
  material: MaterialSide;
  /** Whether it is this side's turn in the shown position. */
  toMove: boolean;
  /** Time left (ms) in the shown position, from the PGN's clocks; null when the game has none. */
  clock?: number | null;
}

/**
 * Player row above or below the board, as in the reference: avatar, name, rating, captured material,
 * and the clock on the right (light, with a clock icon, for the side to move).
 */
export function PlayerStrip({ side, name, elo, material, toMove, clock = null }: Props) {
  const opponent: Side = side === "white" ? "black" : "white";
  const colour = side === "white" ? "White" : "Black";
  return (
    <div className={`player-strip ${toMove ? "is-to-move" : ""}`}>
      <span className={`avatar avatar--${side}`} aria-hidden="true">
        <PieceIcon kind="p" side={side} size={34} />
      </span>
      <div className="player-strip__info">
        <div className="player-strip__name">
          <strong>{name}</strong>
          {elo != null && <span className="player-strip__elo">({elo})</span>}
        </div>
        <div className="captured" aria-label={`Captured: ${material.captured.join(" ") || "nothing"}`}>
          {material.captured.map((k, i) => (
            <PieceIcon
              key={i}
              kind={k}
              side={opponent}
              size={20}
              className={`captured__piece ${material.captured[i - 1] === k ? "is-stacked" : ""}`}
            />
          ))}
          {material.lead > 0 && <span className="captured__lead">+{material.lead}</span>}
        </div>
      </div>
      {clock != null ? (
        <div className={`clock ${toMove ? "is-running" : ""}`} aria-label={`${colour}'s clock: ${formatClock(clock)}`}>
          {toMove && (
            <svg className="clock__icon" width="14" height="14" viewBox="0 0 24 24" aria-hidden="true">
              <circle cx="12" cy="12" r="9.5" fill="none" stroke="currentColor" strokeWidth="2.6" />
              <path fill="none" stroke="currentColor" strokeWidth="2.6" strokeLinecap="round" d="M12 6.5V12l3.5 2.5" />
            </svg>
          )}
          <span className="clock__time">{formatClock(clock)}</span>
        </div>
      ) : (
        toMove && <span className="player-strip__turn" title={`${colour} to move`} />
      )}
    </div>
  );
}
