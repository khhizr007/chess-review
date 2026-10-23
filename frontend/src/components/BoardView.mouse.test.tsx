import { act, cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { canPlay, legalTargets } from "../lib/chess";
import { BoardView, type BoardMoves } from "./BoardView";

// BoardView reads the pointer kind when it loads: here a mouse.
vi.hoisted(() => {
  window.matchMedia = ((query: string) => ({ matches: query === "(pointer: fine)" })) as unknown as typeof window.matchMedia;
});

afterEach(cleanup);

// After 1. e4 d5: White to move, the e4 pawn can push or take on d5.
const FEN = "rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 2";

function board() {
  const onDrop = vi.fn(() => true);
  const moves: BoardMoves = {
    legal: (from, to) => canPlay(FEN, from, to),
    onDrop,
    targets: (square) => legalTargets(FEN, square),
  };
  const view = render(<BoardView fen={FEN} flipped={false} arrows={[]} squareStyles={{}} moves={moves} />);
  const square = (sq: string) => view.container.querySelector<HTMLElement>(`[data-square="${sq}"]`)!;
  const piece = (sq: string) => square(sq).querySelector<HTMLElement>("[data-piece]")!;
  const hints = (selector: string) =>
    [...view.container.querySelectorAll(selector)].map((e) => e.closest<HTMLElement>("[data-square]")!.dataset.square).sort();
  // A mouse press on a piece: its drag starts on the pointer press, then the mouse press follows.
  const press = (sq: string) => {
    act(() => {
      fireEvent.pointerDown(piece(sq), { isPrimary: true, button: 0, clientX: 1, clientY: 1 });
    });
    fireEvent.mouseDown(square(sq), { button: 0 });
  };
  // The lifted piece goes once the overlay has settled, a microtask after the release.
  const release = async (sq: string) => {
    await act(async () => {
      fireEvent.pointerUp(piece(sq), { isPrimary: true, button: 0, clientX: 1, clientY: 1 });
    });
    fireEvent.mouseUp(square(sq), { button: 0 });
  };
  const lifted = () => document.querySelector("[data-piece][style*='grabbing']") !== null;
  return { onDrop, square, hints, press, release, lifted };
}

describe("BoardView with a mouse lifts the piece on the press, as on Lichess", () => {
  it("lifts the pressed piece at once, before the pointer moves, and shows its moves", () => {
    const { press, hints, lifted } = board();
    press("e4");
    expect(lifted()).toBe(true);
    expect(hints(".sq-select")).toEqual(["e4"]);
    expect(hints(".sq-hint")).toEqual(["d5", "e5"]);
  });

  // jsdom has no layout, so a release is a drop off the board: the second click's release (a drop
  // on the piece's own square) is checked in the browser instead.
  it("keeps a piece picked up after the release, and while a second press holds it", async () => {
    const { press, release, hints, lifted } = board();
    press("e4");
    await release("e4");
    expect(lifted()).toBe(false);
    expect(hints(".sq-select")).toEqual(["e4"]);
    press("e4");
    expect(lifted()).toBe(true);
    expect(hints(".sq-select")).toEqual(["e4"]);
  });

  it("takes on the press of a piece the picked-up one can capture, without lifting it", async () => {
    const { press, release, onDrop, lifted } = board();
    press("e4");
    await release("e4");
    press("d5");
    expect(lifted()).toBe(false);
    expect(onDrop).toHaveBeenCalledWith("e4", "d5");
  });
});
