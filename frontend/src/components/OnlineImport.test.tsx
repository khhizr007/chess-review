import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { OnlineGamesError, type OnlineGame } from "../lib/onlineGames";
import { OnlineImport } from "./OnlineImport";

const fetchRecentGames = vi.fn();
vi.mock("../lib/onlineGames", async (importActual) => ({
  ...(await importActual<typeof import("../lib/onlineGames")>()),
  fetchRecentGames: (...args: unknown[]) => fetchRecentGames(...args),
}));

beforeEach(() => localStorage.clear());
afterEach(() => {
  cleanup();
  fetchRecentGames.mockReset();
});

const game: OnlineGame = {
  site: "lichess",
  id: "a1",
  url: "https://lichess.org/a1",
  pgn: '[White "carol"]\n[Black "me"]\n\n1. e4 e5 *',
  white: { name: "carol", rating: 1500 },
  black: { name: "me", rating: 1450 },
  result: "0-1",
  speed: "blitz",
  timeControl: "3+2",
  endedAt: Date.UTC(2026, 9, 20),
  rated: true,
  chess960: false,
  userSide: "black",
};

describe("OnlineImport", () => {
  it("lists the player's games and hands the chosen one over", async () => {
    fetchRecentGames.mockResolvedValue([game]);
    const onPick = vi.fn();
    render(<OnlineImport onPick={onPick} currentPgn="" />);
    fireEvent.click(screen.getByLabelText("Lichess"));
    fireEvent.change(screen.getByLabelText("Lichess username"), { target: { value: " me " } });
    fireEvent.click(screen.getByRole("button", { name: "Show games" }));

    const row = await screen.findByRole("button", { name: /vs carol/ });
    expect(fetchRecentGames).toHaveBeenCalledWith("lichess", "me", expect.objectContaining({ signal: expect.any(AbortSignal) }));
    expect(row.textContent).toContain("W");
    expect(row.textContent).toContain("blitz · 3+2 · rated");
    fireEvent.click(row);
    expect(onPick).toHaveBeenCalledWith(game, "me");
    // The account is remembered for next time.
    expect(JSON.parse(localStorage.getItem("chess-review:online-account")!)).toEqual({ site: "lichess", username: "me" });
  });

  it("shows why games could not be loaded", async () => {
    fetchRecentGames.mockRejectedValue(new OnlineGamesError("not_found", 'No Chess.com player is called "zz".'));
    render(<OnlineImport onPick={() => {}} currentPgn="" />);
    fireEvent.change(screen.getByLabelText("Chess.com username"), { target: { value: "zz" } });
    fireEvent.submit(screen.getByLabelText("Chess.com username").closest("form")!);
    expect(await screen.findByText('No Chess.com player is called "zz".')).toBeTruthy();
  });
});
