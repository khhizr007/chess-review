import { Piece } from "./pieces";

/** App mark: a white knight on a green tile. Pure CSS/SVG, no external assets. */
export function LogoMark({ size = 28 }: { size?: number }) {
  return (
    <span className="logo-mark" style={{ width: size, height: size }} aria-hidden="true">
      <Piece kind="n" colour="w" />
    </span>
  );
}

export function Logo() {
  return (
    <span className="logo">
      <LogoMark />
      <span className="logo__word">
        Game<span className="logo__accent">Review</span>
      </span>
    </span>
  );
}
