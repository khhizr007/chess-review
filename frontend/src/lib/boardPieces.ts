import { createElement, type CSSProperties } from "react";
import type { PieceRenderObject } from "react-chessboard";
import { Piece, type PieceKind } from "../components/pieces";

const KINDS: PieceKind[] = ["p", "n", "b", "r", "q", "k"];

/** react-chessboard piece renderers keyed "wP", "bN", … */
export const boardPieces: PieceRenderObject = Object.fromEntries(
  (["w", "b"] as const).flatMap((colour) =>
    KINDS.map((kind) => [
      `${colour}${kind.toUpperCase()}`,
      (props?: { svgStyle?: CSSProperties }) => createElement(Piece, { kind, colour, style: props?.svgStyle }),
    ]),
  ),
);
