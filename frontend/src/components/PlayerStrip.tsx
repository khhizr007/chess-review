import type { Side } from "../api/generated/Side";
import type { MaterialSide } from "../lib/material";
import { PieceIcon } from "./PieceIcon";

interface Props {
  side: Side;
  name: string;
  elo: number | null;
  material: MaterialSide;
  /** Whether it is this side's turn in the shown position. */
  toMove: boolean;
}

/** Player row above or below the board, as in the reference: avatar, name, rating, captured material. */
export function PlayerStrip({ side, name, elo, material, toMove }: Props) {
  const opponent: Side = side === "white" ? "black" : "white";
  return (
    <div className={`player-strip ${toMove ? "is-to-move" : ""}`}>
      <span className={`avatar avatar--${side}`} aria-hidden="true">
        <PieceIcon kind="p" side={side} size={26} />
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
              size={17}
              className={`captured__piece ${material.captured[i - 1] === k ? "is-stacked" : ""}`}
            />
          ))}
          {material.lead > 0 && <span className="captured__lead">+{material.lead}</span>}
        </div>
      </div>
      {toMove && <span className="player-strip__turn" title={`${side === "white" ? "White" : "Black"} to move`} />}
    </div>
  );
}
