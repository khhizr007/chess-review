import { cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { canPlay, legalTargets } from "../lib/chess";
import { BoardView, type BoardMoves } from "./BoardView";

afterEach(cleanup);

// After 1. e4 d5: White to move, the e4 pawn can push or take on d5.
const FEN = "rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 2";

function board(onDrop = vi.fn(() => true)) {
  const moves: BoardMoves = {
    legal: (from, to) => canPlay(FEN, from, to),
    onDrop,
    targets: (square) => legalTargets(FEN, square),
  };
  const props = { flipped: false, arrows: [], squareStyles: {}, moves };
  const view = render(<BoardView fen={FEN} {...props} />);
  const square = (sq: string) => view.container.querySelector<HTMLElement>(`[data-square="${sq}"]`)!;
  const hints = (selector: string) =>
    [...view.container.querySelectorAll(selector)].map((e) => e.closest<HTMLElement>("[data-square]")!.dataset.square).sort();
  const step = (fen: string) => view.rerender(<BoardView fen={fen} {...props} />);
  return { onDrop, square, hints, step };
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

  it("puts the piece down when it or an empty square is clicked", () => {
    const { square, hints, onDrop } = board();
    fireEvent.click(square("e4"));
    fireEvent.click(square("e4"));
    expect(hints(".sq-select")).toEqual([]);
    fireEvent.click(square("g1"));
    expect(hints(".sq-hint")).toEqual(["e2", "f3", "h3"]);
    fireEvent.click(square("a5"));
    expect(hints(".sq-select, .sq-hint")).toEqual([]);
    expect(onDrop).not.toHaveBeenCalled();
  });

  it("switches to any other piece, the opponent's too", () => {
    const { square, hints } = board();
    fireEvent.click(square("e4"));
    fireEvent.click(square("b1"));
    expect(hints(".sq-select")).toEqual(["b1"]);
    expect(hints(".sq-hint")).toEqual(["a3", "c3"]);
    fireEvent.click(square("g8"));
    expect(hints(".sq-select")).toEqual(["g8"]);
    expect(hints(".sq-hint")).toEqual([]);
  });
});

describe("BoardView picks up any piece, as Chess.com does", () => {
  it("picks up a piece of the side not to move, with no hints", () => {
    const { square, hints } = board();
    fireEvent.click(square("e7"));
    expect(hints(".sq-select")).toEqual(["e7"]);
    expect(hints(".sq-hint")).toEqual([]);
  });

  it("picks up a piece that cannot move, with no hints", () => {
    const { square, hints } = board();
    fireEvent.click(square("a1"));
    expect(hints(".sq-select")).toEqual(["a1"]);
    expect(hints(".sq-hint")).toEqual([]);
  });

  it("refuses a move by the side not to move: the square flashes and nothing is played", () => {
    const { square, hints, onDrop } = board();
    fireEvent.click(square("e7"));
    fireEvent.click(square("e5"));
    expect(onDrop).not.toHaveBeenCalled();
    expect(hints(".sq-refused")).toEqual(["e5"]);
    expect(hints(".sq-select")).toEqual([]);
  });

  it("flashes an empty square clicked with a piece picked up, and nothing without one", () => {
    const { square, hints } = board();
    fireEvent.click(square("a5"));
    expect(hints(".sq-refused")).toEqual([]);
    fireEvent.click(square("e4"));
    fireEvent.click(square("a5"));
    expect(hints(".sq-refused")).toEqual(["a5"]);
  });
});

describe("BoardView with a mouse acts on the press", () => {
  // A mouse click is a press then a click on the same square.
  const press = (el: HTMLElement, button = 0) => fireEvent.mouseDown(el, { button });
  const click = (el: HTMLElement) => {
    press(el);
    fireEvent.click(el);
  };

  it("picks the piece up while the button is still down, and outlines its square", () => {
    const { square, hints } = board();
    press(square("e4"));
    expect(hints(".sq-select")).toEqual(["e4"]);
    expect(hints(".sq-hint")).toEqual(["d5", "e5"]);
    expect(hints(".sq-held")).toEqual(["e4"]);
    fireEvent.mouseUp(window);
    expect(hints(".sq-held")).toEqual([]);
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

  it("puts the piece down on a right press", () => {
    const { square, hints } = board();
    click(square("e4"));
    press(square("e4"), 2);
    expect(hints(".sq-select")).toEqual([]);
  });

  it("marks squares with right clicks until a left press or a new position", () => {
    const { square, hints, step } = board();
    fireEvent.contextMenu(square("a4"));
    fireEvent.contextMenu(square("d5"));
    fireEvent.contextMenu(square("h6"));
    fireEvent.contextMenu(square("h6"));
    expect(hints(".sq-mark")).toEqual(["a4", "d5"]);
    press(square("c3"));
    expect(hints(".sq-mark")).toEqual([]);
    fireEvent.contextMenu(square("a4"));
    // The same pieces with the other side to move (a moved piece would need a laid-out board).
    step(FEN.replace(" w ", " b "));
    expect(hints(".sq-mark")).toEqual([]);
  });
});
