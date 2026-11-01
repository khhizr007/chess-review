// The board's piece set in React (the drawing is in lib/pieceShapes.ts).

import type { CSSProperties, JSX } from "react";
import { GRADIENTS, INK, SHAPES, type PieceColour, type PieceKind } from "../lib/pieceShapes";

export type { PieceColour, PieceKind };
type Kind = PieceKind;
type Colour = PieceColour;

/** One piece; renders nothing for an unknown letter. */
export function Piece({ kind, colour, style }: { kind: string; colour: Colour; style?: CSSProperties }): JSX.Element | null {
  const s = SHAPES[kind.toLowerCase() as Kind];
  if (!s) return null;
  const ink = INK[colour];
  return (
    <svg className={`pc pc--${colour}`} viewBox="0 0 45 45" width="100%" height="100%" style={style} aria-hidden="true">
      <g fill={ink.fill} stroke={ink.outline} strokeWidth={1.5} strokeLinecap="round" strokeLinejoin="round">
        {s.cross?.map((d) => <path key={d} d={d} fill="none" />)}
        {s.bodies.map((d) => <path key={d} d={d} />)}
        {s.balls?.map(([cx, cy, r]) => <circle key={`${cx}`} cx={cx} cy={cy} r={r} />)}
      </g>
      {colour === "b" && s.sheen && <path d={s.sheen} fill="rgba(255,255,255,0.32)" />}
      {s.detail?.[colour]?.map((d) => (
        <path key={d} d={d} fill="none" stroke={ink.detail} strokeWidth={colour === "b" ? 1.1 : 1.5} strokeLinecap="round" strokeLinejoin="round" />
      ))}
      {s.marks?.map((m) => (
        <path key={m.d} d={m.d} transform={m.transform} fill={ink.detail} stroke={ink.detail} strokeWidth={1.5} />
      ))}
    </svg>
  );
}

/** Gradients the pieces fill with. Render once per page, before any piece. */
export function PieceDefs() {
  return (
    <svg width="0" height="0" style={{ position: "absolute" }} aria-hidden="true" focusable="false">
      <defs>
        {(["w", "b"] as const).map((colour) => (
          <linearGradient key={colour} id={`pc-${colour}`} x1="0" y1="0" x2="0.25" y2="1">
            {GRADIENTS[colour].map(([offset, color]) => (
              <stop key={offset} offset={offset} stopColor={color} />
            ))}
          </linearGradient>
        ))}
      </defs>
    </svg>
  );
}
