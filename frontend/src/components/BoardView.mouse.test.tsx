import { act, cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { canPlay, legalTargets } from "../lib/chess";
import { BoardView, type BoardMoves } from "./BoardView";

// BoardView reads the pointer kind when it loads: here a mouse.
vi.hoisted(() => {
  window.matchMedia = ((query: string) => ({ matches: query === "(pointer: fine)" })) as unknown as typeof window.matchMedia;
});

// jsdom lays nothing out: the board is 400 px square in the page's corner, so a square is 50 px.
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
// White can castle short: the king may be dropped on g1 or on its rook.
const CASTLE = "rnbqk2r/pppp1ppp/5n2/2b1p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4";

/** The centre of a square, White at the bottom; `dx`, `dy` move off it. */
const at = (sq: string, dx = 0, dy = 0) => ({
  clientX: (sq.charCodeAt(0) - 97 + 0.5) * 50 + dx,
  clientY: (8 - Number(sq[1]) + 0.5) * 50 + dy,
});

/** Chessground moves a lifted piece on the next frame, and reports a move a moment after playing it. */
const frame = () => act(() => new Promise((resolve) => setTimeout(resolve, 40)));

function board(fen = FEN) {
  const onDrop = vi.fn(() => true);
  const moves: BoardMoves = {
    legal: (from, to) => canPlay(fen, from, to),
    onDrop,
    targets: (square) => legalTargets(fen, square),
  };
  const view = render(<BoardView fen={fen} flipped={false} arrows={[]} squareStyles={{}} moves={moves} />);
  const cgBoard = view.container.querySelector<HTMLElement>("cg-board")!;
  const hints = (selector: string) =>
    [...view.container.querySelectorAll(selector)].map((e) => e.closest<HTMLElement>("[data-square]")!.dataset.square).sort();
  const press = (sq: string, dx = 0, dy = 0) => fireEvent.mouseDown(cgBoard, { button: 0, buttons: 1, ...at(sq, dx, dy) });
  const moveTo = (pos: { clientX: number; clientY: number }) => fireEvent.mouseMove(cgBoard, { buttons: 1, ...pos });
  const release = (pos: { clientX: number; clientY: number }) => fireEvent.mouseUp(cgBoard, { button: 0, ...pos });
  /** Presses a piece, takes it to `to` (a square, or a point off the board) and lets go. */
  const drag = async (from: string, to: string | { clientX: number; clientY: number }) => {
    press(from);
    await frame();
    const pos = typeof to === "string" ? at(to) : to;
    moveTo(pos);
    await frame();
    release(pos);
    await frame();
  };
  const lifted = () => cgBoard.querySelector<HTMLElement>("piece.dragging");
  return { onDrop, cgBoard, hints, press, release, drag, lifted };
}

describe("BoardView with a mouse lifts the piece on the press, as on Lichess", () => {
  it("lifts the pressed piece at once and centres it under the pointer before it moves", async () => {
    const { press, hints, lifted } = board();
    // 20 px up and left of e4's centre (its top-left corner is at 200, 200).
    press("e4", -20, -20);
    expect(lifted()).not.toBeNull();
    expect(hints(".sq-select")).toEqual(["e4"]);
    expect(hints(".sq-hint")).toEqual(["d5", "e5"]);
    await frame();
    expect(lifted()!.style.transform).toBe("translate(180px,180px)");
  });

  it("keeps a piece picked up after the release, and while a second press holds it", async () => {
    const { press, release, hints, lifted } = board();
    press("e4");
    await frame();
    release(at("e4"));
    await frame();
    expect(lifted()).toBeNull();
    expect(hints(".sq-select")).toEqual(["e4"]);
    press("e4");
    expect(lifted()).not.toBeNull();
    expect(hints(".sq-select")).toEqual(["e4"]);
    await frame();
    release(at("e4"));
    expect(hints(".sq-select")).toEqual([]);
  });

  it("takes on the press of a piece the picked-up one can capture, without lifting it", async () => {
    const { press, release, onDrop, lifted } = board();
    press("e4");
    release(at("e4"));
    press("d5");
    expect(lifted()).toBeNull();
    await frame();
    expect(onDrop).toHaveBeenCalledWith("e4", "d5");
  });
});

describe("BoardView drops, as on Chess.com", () => {
  it("plays a legal drop", async () => {
    const { drag, onDrop, hints } = board();
    await drag("e4", "d5");
    expect(onDrop).toHaveBeenCalledWith("e4", "d5");
    expect(hints(".sq-select")).toEqual([]);
  });

  it("refuses an illegal drop: the square flashes and the piece goes back, still picked up", async () => {
    const { drag, onDrop, hints, cgBoard } = board();
    await drag("e4", "e6");
    expect(onDrop).not.toHaveBeenCalled();
    expect(hints(".sq-refused")).toEqual(["e6"]);
    expect(hints(".sq-select")).toEqual(["e4"]);
    expect(hints(".sq-hint")).toEqual(["d5", "e5"]);
    const pawn = [...cgBoard.querySelectorAll<HTMLElement & { cgKey?: string }>("piece")].find((p) => p.cgKey === "e4");
    expect(pawn?.classList.contains("white")).toBe(true);
  });

  it("takes a piece dropped off the board back, still picked up and without a flash", async () => {
    const { drag, onDrop, hints } = board();
    await drag("g1", { clientX: 450, clientY: 300 });
    expect(onDrop).not.toHaveBeenCalled();
    expect(hints(".sq-refused")).toEqual([]);
    expect(hints(".sq-select")).toEqual(["g1"]);
  });

  it("outlines the square under a dragged piece", async () => {
    const { press, cgBoard, hints } = board();
    press("e4");
    await frame();
    fireEvent.mouseMove(cgBoard, { buttons: 1, ...at("e6") });
    expect(hints(".sq-held:not([hidden])")).toEqual(["e6"]);
  });

  it("castles with the king dropped on its rook, though the hint is on g1", async () => {
    const { drag, press, release, onDrop, hints } = board(CASTLE);
    press("e1");
    release(at("e1"));
    expect(hints(".sq-hint")).toEqual(["e2", "f1", "g1"]);
    await drag("e1", "h1");
    expect(onDrop).toHaveBeenCalledWith("e1", "h1");
  });
});
