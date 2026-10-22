import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { GameSummary } from "../api/generated/GameSummary";
import { ReviewLoading } from "./ReviewLoading";

afterEach(cleanup);

const game = {
  index: 0,
  white: "khhizr7",
  black: "siddytopcoder",
  white_elo: 835,
  black_elo: 780,
  event: null,
  date: null,
  result: "1-0",
  ply_count: 2,
  start_fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
  moves: [
    { san: "e4", uci: "e2e4", fen: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1" },
    { san: "e5", uci: "e7e5", fen: "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2" },
  ],
} as GameSummary;

describe("ReviewLoading", () => {
  it("shows the game's players, an empty eval bar and the progress in the coach bubble", () => {
    const onCancel = vi.fn();
    render(<ReviewLoading game={game} progress={{ done: 31, total: 62 }} status="Checked 31 of 62 positions." onCancel={onCancel} />);
    expect(screen.getByText("khhizr7")).toBeTruthy();
    expect(screen.getByText("(780)")).toBeTruthy();
    expect(screen.getByRole("meter", { name: "No evaluation for this position" })).toBeTruthy();
    expect(screen.getByRole("progressbar", { name: "Review progress" }).getAttribute("aria-valuenow")).toBe("50");
    expect(screen.getByText("Checked 31 of 62 positions.").closest(".coach__bubble")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onCancel).toHaveBeenCalled();
  });

  it("falls back to the colours and no Cancel when the game is unknown", () => {
    render(<ReviewLoading game={null} progress={null} status="Waiting for a free engine." />);
    expect(screen.getByText("White")).toBeTruthy();
    expect(screen.getByText("Black")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Cancel" })).toBeNull();
  });
});
