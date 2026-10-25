import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Classification } from "../api/generated/Classification";
import type { ReviewResult } from "../api/generated/ReviewResult";
import { ReportView } from "./ReportView";

afterEach(cleanup);

const tally = (counts: Partial<Record<Classification, number>>) =>
  Object.entries(counts).map(([classification, count]) => ({ classification, count }));

// Chess.com's own counts for game 142636591844.
const review = {
  engine: "Stockfish 18",
  rating_used: 1500,
  notes: [],
  settings: { limit: { kind: "depth", depth: 18 }, multipv: 3, player_side: null },
  game: { result: "1-0", opening: { eco: "C40", name: "King's Pawn Game" } },
  white: {
    side: "white",
    name: "khhizr7",
    elo: 835,
    accuracy: { overall: 78.7, opening: 90, middlegame: 70, endgame: null },
    tally: tally({ book: 2, best: 7, excellent: 3, good: 4, inaccuracy: 1, miss: 1 }),
  },
  black: {
    side: "black",
    name: "siddytopcoder",
    elo: 780,
    accuracy: { overall: 49.3, opening: 60, middlegame: 40, endgame: null },
    tally: tally({ book: 1, best: 2, excellent: 2, good: 1, inaccuracy: 3, mistake: 7 }),
  },
  plies: [],
  graph: [],
  phase_starts: { middlegame: null, endgame: null },
  biggest_swings: [],
} as unknown as ReviewResult;

function show() {
  render(<ReportView review={review} current={0} onSelect={vi.fn()} onReviewAt={vi.fn()} onStart={vi.fn()} onNewGame={vi.fn()} />);
}

const rowNames = () =>
  screen
    .getAllByRole("rowheader")
    .filter((th) => !th.closest("tr")?.hidden)
    .map((th) => th.textContent);

describe("ReportView", () => {
  it("keeps the reference's order: one coach line, then players, accuracy and tallies", () => {
    show();
    expect(screen.getByText("khhizr7 won. Let's review!")).toBeTruthy();
    expect(rowNames().slice(0, 4)).toEqual(["Players", "Accuracy", "Brilliant", "Great find"]);
    expect(screen.getByText("78.7").className).toContain("acc-box--white");
    // The opening name moved below the tallies.
    const notes = screen.getByText("Opening: King's Pawn Game.");
    expect(screen.getByText("Accuracy").compareDocumentPosition(notes) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("shows every class in Chess.com's order, and folds Brilliant, Great, Book and Inaccuracy behind the chevron", () => {
    show();
    const chevron = screen.getByRole("button", { name: "Show fewer move classes" });
    expect(chevron.getAttribute("aria-expanded")).toBe("true");
    expect(rowNames().slice(2, 12)).toEqual([
      "Brilliant",
      "Great find",
      "Book",
      "Best",
      "Excellent",
      "Good",
      "Inaccuracy",
      "Mistake",
      "Miss",
      "Blunder",
    ]);
    fireEvent.click(chevron);
    expect(chevron.getAttribute("aria-expanded")).toBe("false");
    expect(rowNames()).not.toContain("Book");
  });

  it("colours every count by its class, zeros included", () => {
    show();
    const blunders = screen.getByRole("rowheader", { name: "Blunder" }).closest("tr")!;
    const cells = [...blunders.querySelectorAll("td.report__count")] as HTMLElement[];
    expect(cells.map((c) => c.textContent)).toEqual(["0", "0"]);
    expect(cells.every((c) => c.style.color === "var(--c-blunder)")).toBe(true);
  });
});
