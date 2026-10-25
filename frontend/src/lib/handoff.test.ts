import { describe, expect, it, vi } from "vitest";
import { findHandoffGame, handoffCandidates, parseHandoffHash } from "./handoff";
import { OnlineGamesError, type OnlineGame } from "./onlineGames";

const GAME = { id: "x", pgn: "1. e4 *", userSide: "white" } as OnlineGame;
const noSleep = async () => {};

describe("parseHandoffHash", () => {
  it("reads the game type, id and usernames", () => {
    expect(parseHandoffHash("#chesscom=live/185885368440")).toEqual({ type: "live", id: "185885368440", users: [] });
    expect(parseHandoffHash("#chesscom=daily/12&user=Bob&user=al_ice")).toEqual({ type: "daily", id: "12", users: ["Bob", "al_ice"] });
  });

  it("ignores other hashes and malformed links", () => {
    for (const hash of ["", "#review=0", "#chesscom=live/abc", "#chesscom=computer/1", "#chesscom=live/1&user=a b", "#chesscom=live/1&x=y"]) {
      expect(parseHandoffHash(hash)).toBeNull();
    }
  });
});

describe("handoffCandidates", () => {
  it("puts the link's names first and drops repeats in any case", () => {
    const link = { type: "live" as const, id: "1", users: ["Bob"] };
    expect(handoffCandidates(link, "khhizr7", ["bob", "KHHIZR7", "carol"])).toEqual(["Bob", "khhizr7", "carol"]);
  });
});

describe("findHandoffGame", () => {
  const link = { type: "live" as const, id: "1", users: [] };

  it("asks for a username when none is known", async () => {
    expect(await findHandoffGame(link, [])).toHaveProperty("error");
  });

  it("skips an unknown username and finds the game under the next one", async () => {
    const find = vi.fn(async (name: string) => {
      if (name === "gone") throw new OnlineGamesError("not_found", "no such player");
      return GAME;
    });
    expect(await findHandoffGame(link, ["gone", "me"], { find, sleep: noSleep })).toEqual({ game: GAME });
    expect(find).toHaveBeenCalledTimes(2);
  });

  it("searches again when a just-finished game is not in the archive yet", async () => {
    let calls = 0;
    const find = vi.fn(async () => (++calls < 3 ? null : GAME));
    expect(await findHandoffGame(link, ["me"], { find, sleep: noSleep })).toEqual({ game: GAME });
    expect(find).toHaveBeenCalledTimes(3);
  });

  it("gives up after its attempts with a message naming the searched players", async () => {
    const find = vi.fn(async () => null);
    const out = await findHandoffGame(link, ["me"], { find, sleep: noSleep, attempts: 2 });
    expect(find).toHaveBeenCalledTimes(2);
    expect(out).toEqual({ error: expect.stringContaining("me") });
  });

  it("reports a network failure rather than a missing game", async () => {
    const find = vi.fn(async () => {
      throw new OnlineGamesError("network", "Could not reach Chess.com.");
    });
    expect(await findHandoffGame(link, ["me"], { find, sleep: noSleep, attempts: 1 })).toEqual({ error: "Could not reach Chess.com." });
  });
});
