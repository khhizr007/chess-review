import { pieceSvg, type PieceKind } from "./pieceShapes";

const ROLES: [PieceKind, string][] = [
  ["p", "pawn"],
  ["n", "knight"],
  ["b", "bishop"],
  ["r", "rook"],
  ["q", "queen"],
  ["k", "king"],
];

/** Our piece set for chessground, which draws each piece as a `<piece class="white pawn">` background. */
const boardPiecesCss = (["w", "b"] as const)
  .flatMap((colour) =>
    ROLES.map(([kind, role]) => {
      const url = `data:image/svg+xml,${encodeURIComponent(pieceSvg(kind, colour))}`;
      return `.cg-wrap piece.${colour === "w" ? "white" : "black"}.${role}{background-image:url("${url}")}`;
    }),
  )
  .join("\n");

let installed = false;

/** Adds the piece set's stylesheet to the page, once. */
export function installBoardPieces() {
  if (installed || typeof document === "undefined") return;
  installed = true;
  const style = document.createElement("style");
  style.dataset.boardPieces = "";
  style.textContent = boardPiecesCss;
  document.head.appendChild(style);
}
