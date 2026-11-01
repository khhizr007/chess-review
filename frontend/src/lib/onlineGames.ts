// Recent games of a Chess.com or Lichess player, from the sites' public APIs (no key, CORS allowed).
// The browser asks the site directly: the username never reaches this app's server.
import type { Side } from "../api/generated/Side";

export type Site = "chesscom" | "lichess";

export const SITE_NAMES: Record<Site, string> = { chesscom: "Chess.com", lichess: "Lichess" };

export interface OnlinePlayer {
  name: string;
  rating: number | null;
}

export interface OnlineGame {
  site: Site;
  id: string;
  /** The game's page on the site. */
  url: string;
  pgn: string;
  white: OnlinePlayer;
  black: OnlinePlayer;
  result: "1-0" | "0-1" | "½-½" | "*";
  /** "bullet", "blitz", "rapid", "classical", "daily", … */
  speed: string;
  /** "3+2", "10+0", "1 day", or "" when unknown. */
  timeControl: string;
  /** When the game ended (ms since the epoch). */
  endedAt: number;
  rated: boolean;
  chess960: boolean;
  /** The side the searched player had. */
  userSide: Side | null;
}

export type OnlineErrorKind = "invalid" | "not_found" | "rate_limited" | "network";

export class OnlineGamesError extends Error {
  constructor(
    readonly kind: OnlineErrorKind,
    message: string,
  ) {
    super(message);
    this.name = "OnlineGamesError";
  }
}

type Fetch = (url: string, init?: RequestInit) => Promise<Response>;

interface Options {
  /** Most games returned. */
  max?: number;
  signal?: AbortSignal;
  fetch?: Fetch;
}

/** Usernames on both sites: letters, digits, "_" and "-" (Chess.com 3–25, Lichess 2–30 characters). */
const USERNAME = /^[A-Za-z0-9_-]{2,30}$/;
/** Most monthly Chess.com archives read to fill `max` games. */
const MAX_ARCHIVES = 3;

/** The player's most recent finished games on `site`, newest first (standard chess and Chess960 only). */
export async function fetchRecentGames(site: Site, username: string, options: Options = {}): Promise<OnlineGame[]> {
  const name = username.trim();
  if (!USERNAME.test(name)) throw new OnlineGamesError("invalid", `"${name}" is not a valid ${SITE_NAMES[site]} username.`);
  const max = options.max ?? 20;
  // A 404 means no such player only for the player's own URLs; a monthly archive can also 404 on an
  // internal error at Chess.com (seen for a busy player's current month).
  const get = async (url: string, init?: RequestInit, missing: "player" | "error" = "player"): Promise<Response> => {
    let res: Response;
    try {
      res = await (options.fetch ?? fetch)(url, { ...init, signal: options.signal });
    } catch (err) {
      if (options.signal?.aborted) throw err;
      throw new OnlineGamesError("network", `Could not reach ${SITE_NAMES[site]}. Check the connection and try again.`);
    }
    if (res.status === 404 && missing === "player") throw new OnlineGamesError("not_found", `No ${SITE_NAMES[site]} player is called "${name}".`);
    if (res.status === 429) throw new OnlineGamesError("rate_limited", `${SITE_NAMES[site]} asks to wait a minute before trying again.`);
    if (!res.ok) throw new OnlineGamesError("network", `${SITE_NAMES[site]} answered with an error (${res.status}).`);
    return res;
  };
  return site === "chesscom" ? chessComGames(name, max, get) : lichessGames(name, max, get);
}

/* ------------------------------------------------------------------ Chess.com */

interface ChessComPlayer {
  username: string;
  rating?: number;
  result: string;
}

interface ChessComGame {
  url: string;
  pgn?: string;
  uuid?: string;
  time_control?: string;
  time_class?: string;
  end_time: number;
  rated?: boolean;
  rules: string;
  white: ChessComPlayer;
  black: ChessComPlayer;
}

const CHESSCOM_DRAWS = new Set(["agreed", "repetition", "stalemate", "insufficient", "50move", "timevsinsufficient"]);

type Get = (url: string, init?: RequestInit, missing?: "player" | "error") => Promise<Response>;

async function chessComGames(name: string, max: number, get: Get): Promise<OnlineGame[]> {
  const base = `https://api.chess.com/pub/player/${encodeURIComponent(name.toLowerCase())}`;
  const { archives = [] } = (await (await get(`${base}/games/archives`)).json()) as { archives?: string[] };
  const out: OnlineGame[] = [];
  let failure: OnlineGamesError | null = null;
  // Archives are monthly, oldest first; read back from the latest until there are enough games.
  // A month Chess.com fails to return is skipped.
  for (const archive of archives.slice(-MAX_ARCHIVES).reverse()) {
    let games: ChessComGame[];
    try {
      ({ games = [] } = (await (await get(archive, undefined, "error")).json()) as { games?: ChessComGame[] });
    } catch (err) {
      if (!(err instanceof OnlineGamesError)) throw err;
      failure ??= err;
      continue;
    }
    const usable = games
      .filter((g) => (g.rules === "chess" || g.rules === "chess960") && !!g.pgn)
      .sort((a, b) => b.end_time - a.end_time)
      .map((g) => fromChessCom(g, name));
    out.push(...usable);
    if (out.length >= max) break;
  }
  if (out.length === 0 && failure) throw failure;
  return out.slice(0, max);
}

/** Most monthly archives searched for one game (a year of games). */
const FIND_MAX_ARCHIVES = 12;

/**
 * One Chess.com game by its id ("live" or "daily", from the game's URL), searched in `username`'s
 * monthly archives from the newest back. Null when that player has no such game in the last year.
 */
export async function findChessComGame(
  username: string,
  type: "live" | "daily",
  id: string,
  options: Omit<Options, "max"> = {},
): Promise<OnlineGame | null> {
  const name = username.trim();
  if (!USERNAME.test(name)) throw new OnlineGamesError("invalid", `"${name}" is not a valid Chess.com username.`);
  if (!/^\d+$/.test(id)) throw new OnlineGamesError("invalid", `"${id}" is not a Chess.com game id.`);
  const get = async (url: string, missing: "player" | "error"): Promise<Response> => {
    let res: Response;
    try {
      res = await (options.fetch ?? fetch)(url, { signal: options.signal });
    } catch (err) {
      if (options.signal?.aborted) throw err;
      throw new OnlineGamesError("network", "Could not reach Chess.com. Check the connection and try again.");
    }
    if (res.status === 404 && missing === "player") throw new OnlineGamesError("not_found", `No Chess.com player is called "${name}".`);
    if (res.status === 429) throw new OnlineGamesError("rate_limited", "Chess.com asks to wait a minute before trying again.");
    if (!res.ok) throw new OnlineGamesError("network", `Chess.com answered with an error (${res.status}).`);
    return res;
  };
  const base = `https://api.chess.com/pub/player/${encodeURIComponent(name.toLowerCase())}`;
  const { archives = [] } = (await (await get(`${base}/games/archives`, "player")).json()) as { archives?: string[] };
  const suffix = `/game/${type}/${id}`;
  for (const archive of archives.slice(-FIND_MAX_ARCHIVES).reverse()) {
    let games: ChessComGame[];
    try {
      ({ games = [] } = (await (await get(archive, "error")).json()) as { games?: ChessComGame[] });
    } catch (err) {
      // A month Chess.com fails to return is skipped, as in the game list.
      if (err instanceof OnlineGamesError && err.kind === "network") continue;
      throw err;
    }
    const game = games.find((g) => g.url.endsWith(suffix));
    if (game) return game.pgn && (game.rules === "chess" || game.rules === "chess960") ? fromChessCom(game, name) : null;
  }
  return null;
}

function fromChessCom(g: ChessComGame, name: string): OnlineGame {
  const result = g.white.result === "win" ? "1-0" : g.black.result === "win" ? "0-1" : CHESSCOM_DRAWS.has(g.white.result) ? "½-½" : "*";
  const me = name.toLowerCase();
  return {
    site: "chesscom",
    id: g.uuid ?? g.url,
    url: g.url,
    pgn: g.pgn!,
    white: { name: g.white.username, rating: g.white.rating ?? null },
    black: { name: g.black.username, rating: g.black.rating ?? null },
    result,
    speed: g.time_class ?? "",
    timeControl: chessComTimeControl(g.time_control),
    endedAt: g.end_time * 1000,
    rated: !!g.rated,
    chess960: g.rules === "chess960",
    userSide: g.white.username.toLowerCase() === me ? "white" : g.black.username.toLowerCase() === me ? "black" : null,
  };
}

/** "180+2" → "3+2", "600" → "10+0", "1/86400" → "1 day". */
export function chessComTimeControl(tc: string | undefined): string {
  if (!tc || tc === "-") return "";
  const daily = /^1\/(\d+)$/.exec(tc);
  if (daily) {
    const days = Math.round(Number(daily[1]) / 86400);
    return `${days} day${days === 1 ? "" : "s"}`;
  }
  const [base, inc = "0"] = tc.split("+");
  return clock(Number(base), Number(inc));
}

/* ------------------------------------------------------------------ Lichess */

interface LichessPlayer {
  user?: { name: string };
  aiLevel?: number;
  rating?: number;
}

interface LichessGame {
  id: string;
  rated?: boolean;
  variant: string;
  speed?: string;
  status: string;
  lastMoveAt?: number;
  createdAt?: number;
  players: { white: LichessPlayer; black: LichessPlayer };
  winner?: Side;
  pgn?: string;
  clock?: { initial: number; increment: number };
  daysPerTurn?: number;
}

const LICHESS_VARIANTS = new Set(["standard", "chess960", "fromPosition"]);
/** Extra Lichess games asked for in case some are variants or unfinished. */
const LICHESS_SPARE = 5;
const LICHESS_UNFINISHED = new Set(["created", "started", "aborted", "noStart"]);

async function lichessGames(name: string, max: number, get: Get): Promise<OnlineGame[]> {
  // A few extra games are asked for, so that variants and aborted games can be dropped (Lichess
  // streams about 15 games a second to anonymous clients).
  const url = `https://lichess.org/api/games/user/${encodeURIComponent(name)}?max=${max + LICHESS_SPARE}&pgnInJson=true&clocks=false&evals=false&opening=false`;
  const text = await (await get(url, { headers: { Accept: "application/x-ndjson" } })).text();
  const games = text
    .split("\n")
    .filter((l) => l.trim())
    .map((l) => JSON.parse(l) as LichessGame);
  return games
    .filter((g) => LICHESS_VARIANTS.has(g.variant) && !LICHESS_UNFINISHED.has(g.status) && !!g.pgn)
    .slice(0, max)
    .map((g) => fromLichess(g, name));
}

function lichessPlayer(p: LichessPlayer): OnlinePlayer {
  return { name: p.user?.name ?? (p.aiLevel ? `Stockfish level ${p.aiLevel}` : "Anonymous"), rating: p.rating ?? null };
}

function fromLichess(g: LichessGame, name: string): OnlineGame {
  const me = name.toLowerCase();
  const white = lichessPlayer(g.players.white);
  const black = lichessPlayer(g.players.black);
  const result = g.winner === "white" ? "1-0" : g.winner === "black" ? "0-1" : g.status === "draw" || g.status === "stalemate" ? "½-½" : "*";
  return {
    site: "lichess",
    id: g.id,
    url: `https://lichess.org/${g.id}`,
    pgn: g.pgn!,
    white,
    black,
    result,
    speed: g.speed ?? "",
    timeControl: g.clock ? clock(g.clock.initial, g.clock.increment) : g.daysPerTurn ? `${g.daysPerTurn} day${g.daysPerTurn === 1 ? "" : "s"}` : "",
    endedAt: g.lastMoveAt ?? g.createdAt ?? 0,
    rated: !!g.rated,
    chess960: g.variant === "chess960",
    userSide: white.name.toLowerCase() === me ? "white" : black.name.toLowerCase() === me ? "black" : null,
  };
}

/** Base time in seconds and increment as "minutes+increment" ("½+0" for 30 seconds). */
function clock(base: number, inc: number): string {
  const minutes = base % 60 === 0 ? String(base / 60) : base === 30 ? "½" : base === 15 ? "¼" : (base / 60).toFixed(1);
  return `${minutes}+${inc}`;
}

/** The game's result from the searched player's side. */
export function outcomeFor(game: OnlineGame): "won" | "lost" | "drawn" | null {
  if (!game.userSide || game.result === "*") return null;
  if (game.result === "½-½") return "drawn";
  return (game.result === "1-0") === (game.userSide === "white") ? "won" : "lost";
}
