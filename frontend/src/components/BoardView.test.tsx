import { act, cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { canPlay, legalTargets } from "../lib/chess";
import { BoardView, type BoardMoves } from "./BoardView";

// jsdom lays nothing out. Chessground finds the square under the pointer from the board's box, so
// the board is 400 px square in the page's corner: a square is 50 px.
beforeEach(() => {
  const box = { x: 0, y: 0, left: 0, top: 0, width: 400, height: 400, right: 400, bottom: 400 };
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({ ...box, toJSON: () => box } as DOMRect);
});
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

// After 1. e4 d5: White to move, the e4 pawn can push or take on d5.
const FEN = "rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 2";

/** The centre of a square, White at the bottom. */
const at = (sq: string) => ({ clientX: (sq.charCodeAt(0) - 97 + 0.5) * 50, clientY: (8 - Number(sq[1]) + 0.5) * 50 });

/** Chessground reports a move a moment after playing it. */
const settle = () => act(() => new Promise((resolve) => setTimeout(resolve, 10)));

function board(onDrop = vi.fn(() => true)) {
  const moves: BoardMoves = {
    legal: (from, to) => canPlay(FEN, from, to),
    onDrop,
    targets: (square) => legalTargets(FEN, square),
  };
  const props = { flipped: false, arrows: [], squareStyles: {}, moves };
  const view = render(<BoardView fen={FEN} {...props} />);
  const cgBoard = view.container.querySelector<HTMLElement>("cg-board")!;
  const hints = (selector: string) =>
    [...view.container.querySelectorAll(selector)].map((e) => e.closest<HTMLElement>("[data-square]")!.dataset.square).sort();
  // A finger's tap: chessground handles the touch itself, and browsers send no mouse press for it.
  const tap = (sq: string) => {
    fireEvent.touchStart(cgBoard, { targetTouches: [at(sq)], touches: [at(sq)] });
    fireEvent.touchEnd(cgBoard, { targetTouches: [], touches: [] });
  };
  const step = (fen: string) => view.rerender(<BoardView fen={fen} {...props} />);
  return { onDrop, cgBoard, tap, hints, step };
}

describe("BoardView moves by click", () => {
  it("picks a piece up and shows dots for free squares and rings for captures", () => {
    const { tap, hints } = board();
    tap("e4");
    expect(hints(".sq-select")).toEqual(["e4"]);
    expect(hints(".sq-hint:not(.sq-hint--capture)")).toEqual(["e5"]);
    expect(hints(".sq-hint--capture")).toEqual(["d5"]);
  });

  it("moves the picked-up piece to a clicked target", async () => {
    const { tap, hints, onDrop } = board();
    tap("e4");
    tap("d5");
    await settle();
    expect(onDrop).toHaveBeenCalledWith("e4", "d5");
    expect(hints(".sq-select")).toEqual([]);
  });

  it("puts the piece down when it or an empty square is clicked", async () => {
    const { tap, hints, onDrop } = board();
    tap("e4");
    tap("e4");
    expect(hints(".sq-select")).toEqual([]);
    tap("g1");
    expect(hints(".sq-hint")).toEqual(["e2", "f3", "h3"]);
    tap("a5");
    expect(hints(".sq-select, .sq-hint")).toEqual([]);
    await settle();
    expect(onDrop).not.toHaveBeenCalled();
  });

  it("switches to any other piece, the opponent's too", () => {
    const { tap, hints } = board();
    tap("e4");
    tap("b1");
    expect(hints(".sq-select")).toEqual(["b1"]);
    expect(hints(".sq-hint")).toEqual(["a3", "c3"]);
    tap("g8");
    expect(hints(".sq-select")).toEqual(["g8"]);
    expect(hints(".sq-hint")).toEqual([]);
  });
});

describe("BoardView picks up any piece, as Chess.com does", () => {
  it("picks up a piece of the side not to move, with no hints", () => {
    const { tap, hints } = board();
    tap("e7");
    expect(hints(".sq-select")).toEqual(["e7"]);
    expect(hints(".sq-hint")).toEqual([]);
  });

  it("picks up a piece that cannot move, with no hints", () => {
    const { tap, hints } = board();
    tap("a1");
    expect(hints(".sq-select")).toEqual(["a1"]);
    expect(hints(".sq-hint")).toEqual([]);
  });

  it("refuses a move by the side not to move: the square flashes and nothing is played", async () => {
    const { tap, hints, onDrop } = board();
    tap("e7");
    tap("e5");
    await settle();
    expect(onDrop).not.toHaveBeenCalled();
    expect(hints(".sq-refused")).toEqual(["e5"]);
    expect(hints(".sq-select")).toEqual([]);
  });

  it("flashes an empty square clicked with a piece picked up, and nothing without one", () => {
    const { tap, hints } = board();
    tap("a5");
    expect(hints(".sq-refused")).toEqual([]);
    tap("e4");
    tap("a5");
    expect(hints(".sq-refused")).toEqual(["a5"]);
  });
});

describe("BoardView with a mouse acts on the press", () => {
  function mouse() {
    const b = board();
    const press = (sq: string, button = 0) => fireEvent.mouseDown(b.cgBoard, { button, buttons: button === 2 ? 2 : 1, ...at(sq) });
    const release = (sq: string, button = 0) => fireEvent.mouseUp(b.cgBoard, { button, ...at(sq) });
    const click = (sq: string) => {
      press(sq);
      release(sq);
    };
    const rightClick = (sq: string) => {
      press(sq, 2);
      release(sq, 2);
    };
    return { ...b, press, release, click, rightClick };
  }

  it("picks the piece up while the button is still down, and outlines its square", () => {
    const { press, release, hints } = mouse();
    press("e4");
    expect(hints(".sq-select")).toEqual(["e4"]);
    expect(hints(".sq-hint")).toEqual(["d5", "e5"]);
    expect(hints(".sq-held:not([hidden])")).toEqual(["e4"]);
    release("e4");
    expect(hints(".sq-held:not([hidden])")).toEqual([]);
  });

  it("moves on the press of a target square", async () => {
    const { click, press, onDrop } = mouse();
    click("e4");
    press("e5");
    await settle();
    expect(onDrop).toHaveBeenCalledWith("e4", "e5");
  });

  it("puts a piece clicked twice down on the release, not the press", () => {
    const { click, press, release, hints } = mouse();
    click("e4");
    press("e4");
    // Still picked up while held, so it can be dragged.
    expect(hints(".sq-select")).toEqual(["e4"]);
    release("e4");
    expect(hints(".sq-select")).toEqual([]);
  });

  it("puts the piece down on a right press", () => {
    const { click, press, hints } = mouse();
    click("e4");
    press("e4", 2);
    expect(hints(".sq-select")).toEqual([]);
  });

  it("marks squares with right clicks until a left press or a new position", () => {
    const { rightClick, press, hints, step } = mouse();
    rightClick("a4");
    rightClick("d5");
    rightClick("h6");
    rightClick("h6");
    expect(hints(".sq-mark")).toEqual(["a4", "d5"]);
    press("c3");
    expect(hints(".sq-mark")).toEqual([]);
    rightClick("a4");
    step(FEN.replace(" w ", " b "));
    expect(hints(".sq-mark")).toEqual([]);
  });

  it("draws an arrow with a right drag, and takes it away with the same drag", () => {
    const { cgBoard, press, release } = mouse();
    const arrows = () => cgBoard.closest(".board")!.querySelectorAll(".board-arrows path").length;
    const drag = () => {
      press("g1", 2);
      fireEvent.mouseMove(cgBoard, { button: 2, buttons: 2, ...at("f3") });
      release("f3", 2);
    };
    drag();
    expect(arrows()).toBe(1);
    drag();
    expect(arrows()).toBe(0);
  });
});
