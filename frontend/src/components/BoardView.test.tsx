import { cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { canMoveFrom, legalTargets } from "../lib/chess";
import { BoardView, type BoardMoves } from "./BoardView";

afterEach(cleanup);

// After 1. e4 d5: White to move, the e4 pawn can push or take on d5.
const FEN = "rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 2";

function board(onDrop = vi.fn(() => true)) {
  const moves: BoardMoves = {
    canDrag: (square) => canMoveFrom(FEN, square),
    onDrop,
    targets: (square) => legalTargets(FEN, square),
  };
  const view = render(<BoardView fen={FEN} flipped={false} arrows={[]} squareStyles={{}} moves={moves} />);
  const square = (sq: string) => view.container.querySelector<HTMLElement>(`[data-square="${sq}"]`)!;
  const hints = (selector: string) =>
    [...view.container.querySelectorAll(selector)].map((e) => e.closest<HTMLElement>("[data-square]")!.dataset.square).sort();
  return { onDrop, square, hints };
}

describe("BoardView moves by click", () => {
  it("picks a piece up and shows dots for free squares and rings for captures", () => {
    const { square, hints } = board();
    fireEvent.click(square("e4"));
    expect(hints(".sq-select")).toEqual(["e4"]);
    expect(hints(".sq-hint:not(.sq-hint--capture)")).toEqual(["e5"]);
    expect(hints(".sq-hint--capture")).toEqual(["d5"]);
  });

  it("moves the picked-up piece to a clicked target", () => {
    const { square, hints, onDrop } = board();
    fireEvent.click(square("e4"));
    fireEvent.click(square("d5"));
    expect(onDrop).toHaveBeenCalledWith("e4", "d5");
    expect(hints(".sq-select")).toEqual([]);
  });

  it("puts the piece down when it, an empty square or the opponent's piece is clicked", () => {
    const { square, hints, onDrop } = board();
    fireEvent.click(square("e4"));
    fireEvent.click(square("e4"));
    expect(hints(".sq-select")).toEqual([]);
    fireEvent.click(square("g1"));
    expect(hints(".sq-hint")).toEqual(["e2", "f3", "h3"]);
    fireEvent.click(square("a5"));
    expect(hints(".sq-hint")).toEqual([]);
    fireEvent.click(square("d5"));
    expect(hints(".sq-select")).toEqual([]);
    expect(onDrop).not.toHaveBeenCalled();
  });

  it("switches to another of the mover's pieces", () => {
    const { square, hints } = board();
    fireEvent.click(square("e4"));
    fireEvent.click(square("b1"));
    expect(hints(".sq-select")).toEqual(["b1"]);
    expect(hints(".sq-hint")).toEqual(["a3", "c3"]);
  });
});
