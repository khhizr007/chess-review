import type { Side } from "../api/generated/Side";
import { Piece } from "./pieces";

interface PieceProps {
  /** Lower-case piece letter: p, n, b, r, q, k. */
  kind: string;
  side: Side;
  size?: number;
  className?: string;
}

/** A single piece glyph, drawn with the board's own piece set. */
export function PieceIcon({ kind, side, size = 16, className }: PieceProps) {
  return (
    <span className={`piece-icon ${className ?? ""}`} style={{ width: size, height: size }} aria-hidden="true">
      <Piece kind={kind} colour={side === "white" ? "w" : "b"} />
    </span>
  );
}

/** SAN with figurine notation: "Nf3" → ♘f3, drawn in the mover's colour. */
export function San({ san, side }: { san: string; side: Side }) {
  const piece = /^[KQRBN]/.exec(san)?.[0];
  if (!piece) return <span className="san">{san}</span>;
  return (
    <span className="san">
      <span className="visually-hidden">{piece}</span>
      <PieceIcon kind={piece} side={side} className="san__piece" />
      {san.slice(1)}
    </span>
  );
}
