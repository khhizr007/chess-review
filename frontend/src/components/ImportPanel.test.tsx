import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { CreateReviewRequest } from "../api/generated/CreateReviewRequest";
import type { GameSummary } from "../api/generated/GameSummary";
import { myUsernames } from "../lib/players";
import { ImportPanel } from "./ImportPanel";

const game = (white: string, black: string) => ({
  index: 0,
  game: {
    index: 0,
    white,
    black,
    white_elo: null,
    black_elo: null,
    event: null,
    date: null,
    result: "1-0",
    ply_count: 7,
    start_fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
    moves: [],
  },
  error: null,
  error_message: null,
});

function serve(white: string, black: string) {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (path: string) => {
      const body = path.endsWith("/health")
        ? { status: "ok", version: "test", engine: "Stockfish", engine_error: null, opening_positions: 0 }
        : { games: [game(white, black)] };
      return new Response(JSON.stringify(body), { status: 200 });
    }),
  );
}

async function load(onStart: (req: CreateReviewRequest, game: GameSummary | null) => void) {
  render(<ImportPanel onStart={onStart} busy={false} />);
  fireEvent.change(screen.getByLabelText("PGN"), { target: { value: "1. e4 e5" } });
  await screen.findByRole("radiogroup", { name: "Games in the PGN" }, { timeout: 2000 });
  await waitFor(() => expect((screen.getByRole("button", { name: "Start review" }) as HTMLButtonElement).disabled).toBe(false));
}

const sideOption = (name: RegExp) =>
  within(screen.getByRole("group", { name: "Which side you played" })).getByRole("radio", { name });

beforeEach(() => localStorage.clear());
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

describe("ImportPanel: this is me", () => {
  it("sends the chosen side and remembers the username", async () => {
    serve("Alice", "Bob");
    const onStart = vi.fn();
    await load(onStart);
    // Nothing is known yet: the coach stays neutral.
    expect((sideOption(/Neither/) as HTMLInputElement).checked).toBe(true);
    fireEvent.click(sideOption(/Bob/));
    fireEvent.click(screen.getByRole("button", { name: "Start review" }));
    // The parsed game goes along, so the review layout can show its board while the engine works.
    expect(onStart).toHaveBeenCalledWith(expect.objectContaining({ player_side: "black" }), expect.objectContaining({ white: "Alice", black: "Bob" }));
    expect(myUsernames()).toEqual(["bob"]);
  });

  it("preselects the side of a remembered username", async () => {
    localStorage.setItem("chess-review:my-usernames", JSON.stringify(["bob"]));
    serve("Bob", "Carol");
    const onStart = vi.fn();
    await load(onStart);
    await waitFor(() => expect((sideOption(/Bob/) as HTMLInputElement).checked).toBe(true));
    fireEvent.click(sideOption(/Neither/));
    fireEvent.click(screen.getByRole("button", { name: "Start review" }));
    expect(onStart.mock.calls[0]?.[0]).not.toHaveProperty("player_side");
    expect(myUsernames()).toEqual([]);
  });
});

describe("ImportPanel: analysis settings", () => {
  it("offers no depth or line choice and always asks for depth 18", async () => {
    serve("Alice", "Bob");
    const onStart = vi.fn();
    await load(onStart);
    expect(screen.queryByRole("group", { name: "Strength" })).toBeNull();
    expect(screen.queryByLabelText("Engine lines per position")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Start review" }));
    const req = onStart.mock.calls[0]?.[0] as CreateReviewRequest;
    expect(req.strength).toBe("standard");
    expect(req).not.toHaveProperty("multipv");
  });
});
