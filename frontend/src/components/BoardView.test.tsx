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

describe("BoardView with a mouse acts on the press", () => {
  // A mouse click is a press then a click on the same square.
  const press = (el: HTMLElement) => fireEvent.mouseDown(el, { button: 0 });
  const click = (el: HTMLElement) => {
    press(el);
    fireEvent.click(el);
  };

  it("picks the piece up while the button is still down", () => {
    const { square, hints } = board();
    press(square("e4"));
    expect(hints(".sq-select")).toEqual(["e4"]);
    expect(hints(".sq-hint")).toEqual(["d5", "e5"]);
  });

  it("moves on the press of a target square", () => {
    const { square, onDrop } = board();
    click(square("e4"));
    press(square("e5"));
    expect(onDrop).toHaveBeenCalledWith("e4", "e5");
  });

  it("puts a piece clicked twice down on the release, not the press", () => {
    const { square, hints } = board();
    click(square("e4"));
    press(square("e4"));
    // Still picked up while held, so it can be dragged.
    expect(hints(".sq-select")).toEqual(["e4"]);
    fireEvent.click(square("e4"));
    expect(hints(".sq-select")).toEqual([]);
  });

  it("ignores the right button", () => {
    const { square, hints } = board();
    fireEvent.mouseDown(square("e4"), { button: 2 });
    expect(hints(".sq-select")).toEqual([]);
  });
});
