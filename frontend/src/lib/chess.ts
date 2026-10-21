// Chess rules for exploring sidelines (docs/explore-mode.md §5.1): legality, SAN, FEN and move
// flags come from chessops here. Everything about the game itself still comes from the backend.
import { Chess, castlingSide, normalizeMove } from "chessops/chess";
import { makeFen, parseFen } from "chessops/fen";
import { makeSanAndPlay } from "chessops/san";
import { charToRole, makeSquare, makeUci, parseSquare } from "chessops/util";
import type { NormalMove } from "chessops/types";
import type { MoveFlags } from "../api/generated/MoveFlags";
import type { MoveRef } from "../api/generated/MoveRef";

/** A move played on the board: its SAN, UCI and FEN, the squares to highlight and its flags. */
export interface PlayedMove extends MoveRef {
  from: string;
  /** Where the moving piece lands (the king's square when castling). */
  to: string;
  flags: MoveFlags;
}

export type PromotionPiece = "q" | "r" | "b" | "n";

/** How a position ends the game, if it does. */
export type Ending = "checkmate" | "stalemate" | "insufficient_material" | "fifty_moves";

function position(fen: string): Chess | null {
  const setup = parseFen(fen);
  if (setup.isErr) return null;
  const pos = Chess.fromSetup(setup.value);
  return pos.isOk ? pos.value : null;
}

function rawMove(from: string, to: string, promotion?: PromotionPiece): NormalMove | null {
  const f = parseSquare(from);
  const t = parseSquare(to);
  if (f === undefined || t === undefined) return null;
  return promotion ? { from: f, to: t, promotion: charToRole(promotion) } : { from: f, to: t };
}

/** Whether the piece on `square` belongs to the side to move and has a legal move. */
export function canMoveFrom(fen: string, square: string): boolean {
  const pos = position(fen);
  const sq = parseSquare(square);
  if (!pos || sq === undefined || pos.isEnd()) return false;
  return pos.board[pos.turn].has(sq) && pos.dests(sq).nonEmpty();
}

/** Whether moving `from`→`to` is a legal pawn move that needs a promotion piece. */
export function needsPromotion(fen: string, from: string, to: string): boolean {
  const pos = position(fen);
  const move = rawMove(from, to, "q");
  if (!pos || !move) return false;
  const pawn = pos.board.getRole(move.from) === "pawn";
  const lastRank = Math.floor(move.to / 8) === (pos.turn === "white" ? 7 : 0);
  return pawn && lastRank && pos.isLegal(move);
}

/**
 * Plays `from`→`to` (a castling king may be dropped on its target square or on its rook), or
 * returns null when the move is illegal. UCI follows the backend: king squares in standard chess,
 * king-takes-rook in Chess960.
 */
export function playMove(fen: string, from: string, to: string, promotion?: PromotionPiece, chess960 = false): PlayedMove | null {
  const pos = position(fen);
  const raw = rawMove(from, to, promotion);
  if (!pos || !raw) return null;
  const side = castlingSide(pos, raw);
  const move = normalizeMove(pos, raw) as NormalMove;
  if (promotion) move.promotion = charToRole(promotion);
  if (!pos.isLegal(move)) return null;

  const role = pos.board.getRole(move.from);
  const enPassant = role === "pawn" && move.to === pos.epSquare && !pos.board.occupied.has(move.to);
  const castle = !!side && role === "king";
  const capture = enPassant || (!castle && pos.board.occupied.has(move.to));
  const turn = pos.turn;

  const san = makeSanAndPlay(pos, move);
  const kingTo = castle ? makeSquare((side === "h" ? 6 : 2) + (turn === "white" ? 0 : 56)) : makeSquare(move.to);
  const uci = castle && !chess960 ? makeSquare(move.from) + kingTo : makeUci(move);
  const checkmate = pos.isCheckmate();
  return {
    san,
    uci,
    fen: makeFen(pos.toSetup()),
    from: makeSquare(move.from),
    to: kingTo,
    flags: {
      check: pos.isCheck(),
      checkmate,
      stalemate: pos.isStalemate(),
      capture,
      castle,
      promotion: !!move.promotion,
      en_passant: enPassant,
    },
  };
}

/** How the position ends the game: mate, stalemate, dead material, or the fifty-move rule (claimable). */
export function ending(fen: string): Ending | null {
  const pos = position(fen);
  if (!pos) return null;
  if (pos.isCheckmate()) return "checkmate";
  if (pos.isStalemate()) return "stalemate";
  if (pos.isInsufficientMaterial()) return "insufficient_material";
  if (pos.halfmoves >= 100) return "fifty_moves";
  return null;
}
