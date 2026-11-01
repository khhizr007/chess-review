import { describe, expect, it, vi } from "vitest";
import { OnlineGamesError, chessComTimeControl, fetchRecentGames, findChessComGame, outcomeFor } from "./onlineGames";

/** A fake fetch answering from a table of URL → [status, body]. */
function fakeFetch(routes: Record<string, [number, string]>) {
  return vi.fn<(url: string, init?: RequestInit) => Promise<Response>>(async (url) => {
    const route = routes[url];
    if (!route) throw new TypeError(`no route for ${url}`);
    return new Response(route[1], { status: route[0] });
  });
}

const ccGame = (n: number, white: string, black: string, extra: object = {}) => ({
  url: `https://www.chess.com/game/live/${n}`,
  uuid: `uuid-${n}`,
  pgn: `[Event "Live Chess"]\n[White "${white}"]\n[Black "${black}"]\n\n1. e4 e5 *`,
  time_control: "180+2",
  time_class: "blitz",
  end_time: 1_700_000_000 + n,
  rated: true,
  rules: "chess",
  white: { username: white, rating: 1000 + n, result: "win" },
  black: { username: black, rating: 900, result: "resigned" },
  ...extra,
});

const ARCHIVES = "https://api.chess.com/pub/player/khhizr7/games/archives";
const OCT = "https://api.chess.com/pub/player/khhizr7/games/2026/10";
const SEP = "https://api.chess.com/pub/player/khhizr7/games/2026/09";

describe("fetchRecentGames on Chess.com", () => {
  it("reads the latest monthly archives, newest game first, and keeps only standard chess and Chess960", async () => {
    const fetch = fakeFetch({
      [ARCHIVES]: [200, JSON.stringify({ archives: [SEP, OCT] })],
      [OCT]: [
        200,
        JSON.stringify({
          games: [
            ccGame(1, "khhizr7", "bob"),
            ccGame(3, "carol", "KHHIZR7", { white: { username: "carol", result: "agreed" }, black: { username: "KHHIZR7", result: "agreed" } }),
            ccGame(2, "dave", "khhizr7", { rules: "crazyhouse" }),
          ],
        }),
      ],
      [SEP]: [200, JSON.stringify({ games: [ccGame(0, "khhizr7", "eve", { rules: "chess960", time_control: "1/86400", time_class: "daily" })] })],
    });
    const games = await fetchRecentGames("chesscom", " KHHIZR7 ", { fetch });
    expect(games.map((g) => g.id)).toEqual(["uuid-3", "uuid-1", "uuid-0"]);
    expect(games[0]).toMatchObject({ result: "½-½", userSide: "black", timeControl: "3+2", speed: "blitz" });
    expect(games[1]).toMatchObject({ result: "1-0", userSide: "white", white: { name: "khhizr7", rating: 1001 } });
    expect(games[2]).toMatchObject({ chess960: true, timeControl: "1 day", endedAt: 1_700_000_000_000 });
  });

  it("stops reading archives once there are enough games", async () => {
    const fetch = fakeFetch({
      [ARCHIVES]: [200, JSON.stringify({ archives: [SEP, OCT] })],
      [OCT]: [200, JSON.stringify({ games: [ccGame(1, "khhizr7", "a"), ccGame(2, "khhizr7", "b")] })],
    });
    const games = await fetchRecentGames("chesscom", "khhizr7", { fetch, max: 2 });
    expect(games).toHaveLength(2);
    expect(fetch).toHaveBeenCalledTimes(2);
  });

  it("skips a month Chess.com fails to return, and says so when none can be read", async () => {
    // Seen live: the current month's archive answers 404 "An internal error has occurred".
    const internal = '{"code":0,"message":"An internal error has occurred."}';
    const fetch = fakeFetch({
      [ARCHIVES]: [200, JSON.stringify({ archives: [SEP, OCT] })],
      [OCT]: [404, internal],
      [SEP]: [200, JSON.stringify({ games: [ccGame(1, "khhizr7", "a")] })],
    });
    expect((await fetchRecentGames("chesscom", "khhizr7", { fetch })).map((g) => g.id)).toEqual(["uuid-1"]);

    const broken = fakeFetch({
      [ARCHIVES]: [200, JSON.stringify({ archives: [SEP, OCT] })],
      [OCT]: [404, internal],
      [SEP]: [500, ""],
    });
    await expect(fetchRecentGames("chesscom", "khhizr7", { fetch: broken })).rejects.toMatchObject({ kind: "network" });
  });

  it("says when the player does not exist or the site cannot be reached", async () => {
    const notFound = fakeFetch({ [ARCHIVES]: [404, "{}"] });
    await expect(fetchRecentGames("chesscom", "khhizr7", { fetch: notFound })).rejects.toMatchObject({ kind: "not_found" });
    const offline = vi.fn(async () => {
      throw new TypeError("Failed to fetch");
    });
    await expect(fetchRecentGames("chesscom", "khhizr7", { fetch: offline })).rejects.toMatchObject({ kind: "network" });
    const busy = fakeFetch({ [ARCHIVES]: [429, ""] });
    await expect(fetchRecentGames("chesscom", "khhizr7", { fetch: busy })).rejects.toMatchObject({ kind: "rate_limited" });
  });

  it("refuses an invalid username without asking the site", async () => {
    const fetch = vi.fn();
    await expect(fetchRecentGames("chesscom", "bad name/..", { fetch })).rejects.toBeInstanceOf(OnlineGamesError);
    expect(fetch).not.toHaveBeenCalled();
  });
});

describe("fetchRecentGames on Lichess", () => {
  const line = (id: string, extra: object = {}) =>
    JSON.stringify({
      id,
      rated: true,
      variant: "standard",
      speed: "rapid",
      status: "resign",
      lastMoveAt: 1_792_593_356_868,
      players: { white: { user: { name: "Thibault" }, rating: 1798 }, black: { aiLevel: 3 } },
      winner: "white",
      pgn: `[Site "https://lichess.org/${id}"]\n\n1. e4 e5 *`,
      clock: { initial: 600, increment: 0 },
      ...extra,
    });

  it("reads the NDJSON games and drops variants and unfinished games", async () => {
    const url =
      "https://lichess.org/api/games/user/thibault?max=25&pgnInJson=true&clocks=false&evals=false&opening=false";
    const body = [
      line("a1"),
      line("a2", { variant: "crazyhouse" }),
      line("a3", { status: "aborted" }),
      line("a4", { winner: undefined, status: "draw", clock: { initial: 30, increment: 0 } }),
    ].join("\n");
    const fetch = fakeFetch({ [url]: [200, body + "\n"] });
    const games = await fetchRecentGames("lichess", "thibault", { fetch });
    expect(fetch.mock.calls[0]![1]).toMatchObject({ headers: { Accept: "application/x-ndjson" } });
    expect(games.map((g) => g.id)).toEqual(["a1", "a4"]);
    expect(games[0]).toMatchObject({
      url: "https://lichess.org/a1",
      userSide: "white",
      result: "1-0",
      timeControl: "10+0",
      black: { name: "Stockfish level 3", rating: null },
    });
    expect(games[1]).toMatchObject({ result: "½-½", timeControl: "½+0" });
    expect(outcomeFor(games[0]!)).toBe("won");
    expect(outcomeFor(games[1]!)).toBe("drawn");
  });
});

describe("chessComTimeControl", () => {
  it("formats live and daily time controls", () => {
    expect(chessComTimeControl("600")).toBe("10+0");
    expect(chessComTimeControl("60+1")).toBe("1+1");
    expect(chessComTimeControl("1/259200")).toBe("3 days");
    expect(chessComTimeControl("-")).toBe("");
  });
});

describe("findChessComGame", () => {
  it("finds a game by id in an older archive and reports the player's side", async () => {
    const fetch = fakeFetch({
      [ARCHIVES]: [200, JSON.stringify({ archives: [SEP, OCT] })],
      [OCT]: [200, JSON.stringify({ games: [ccGame(5, "khhizr7", "a")] })],
      [SEP]: [200, JSON.stringify({ games: [ccGame(4, "bob", "khhizr7"), ccGame(41, "khhizr7", "c")] })],
    });
    const game = await findChessComGame("KHHIZR7", "live", "4", { fetch });
    expect(game).toMatchObject({ url: "https://www.chess.com/game/live/4", userSide: "black" });
    // "41" ends in "1" but not in "/4": ids are matched whole.
    expect(await findChessComGame("khhizr7", "live", "1", { fetch })).toBeNull();
  });

  it("does not take a daily game for a live one", async () => {
    const fetch = fakeFetch({
      [ARCHIVES]: [200, JSON.stringify({ archives: [OCT] })],
      [OCT]: [200, JSON.stringify({ games: [ccGame(7, "khhizr7", "a")] })],
    });
    expect(await findChessComGame("khhizr7", "daily", "7", { fetch })).toBeNull();
  });

  it("returns null for a variant game, which cannot be reviewed", async () => {
    const fetch = fakeFetch({
      [ARCHIVES]: [200, JSON.stringify({ archives: [OCT] })],
      [OCT]: [200, JSON.stringify({ games: [ccGame(8, "khhizr7", "a", { rules: "crazyhouse" })] })],
    });
    expect(await findChessComGame("khhizr7", "live", "8", { fetch })).toBeNull();
  });

  it("rejects ids that are not numbers before asking Chess.com", async () => {
    const fetch = fakeFetch({});
    await expect(findChessComGame("khhizr7", "live", "../x", { fetch })).rejects.toMatchObject({ kind: "invalid" });
    expect(fetch).not.toHaveBeenCalled();
  });
});
