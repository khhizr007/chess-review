// Opening a Chess.com game straight from its id: the browser extension (extension/) sends
// `#chesscom=live/<id>` (or `daily/<id>`), optionally with `&user=<name>` for each player it knows.
// The game is looked up in the players' public archives, so the app needs one of their usernames.
import { myUsernames } from "./players";
import { OnlineGamesError, findChessComGame, type OnlineGame } from "./onlineGames";

export interface ChessComHandoff {
  type: "live" | "daily";
  id: string;
  /** Usernames named in the link, in order. */
  users: string[];
}

export function parseHandoffHash(hash: string): ChessComHandoff | null {
  const m = /^#chesscom=(live|daily)\/(\d+)((?:&user=[A-Za-z0-9_-]+)*)$/.exec(hash);
  if (!m) return null;
  const users = [...m[3]!.matchAll(/&user=([A-Za-z0-9_-]+)/g)].map((u) => u[1]!);
  return { type: m[1] as "live" | "daily", id: m[2]!, users };
}

/** The Chess.com username saved by the "Your games" box (components/OnlineImport.tsx). */
function savedChessComUsername(): string | null {
  try {
    const saved: unknown = JSON.parse(localStorage.getItem("chess-review:online-account") ?? "null");
    if (saved && typeof saved === "object" && "site" in saved && "username" in saved) {
      const { site, username } = saved as { site: unknown; username: unknown };
      if (site === "chesscom" && typeof username === "string" && username.trim()) return username.trim();
    }
  } catch {
    // Storage may be unavailable (private mode).
  }
  return null;
}

/** Usernames to search, without repeats: the link's, then the ones this app remembers. */
export function handoffCandidates(handoff: ChessComHandoff, saved: string | null = savedChessComUsername(), mine: string[] = myUsernames()): string[] {
  const out: string[] = [];
  for (const name of [...handoff.users, ...(saved ? [saved] : []), ...mine]) {
    if (!out.some((n) => n.toLowerCase() === name.toLowerCase())) out.push(name);
  }
  return out;
}

interface FindOptions {
  signal?: AbortSignal;
  /** Searches before giving up; a game that just ended can take a moment to reach the archives. */
  attempts?: number;
  delayMs?: number;
  find?: typeof findChessComGame;
  sleep?: (ms: number, signal?: AbortSignal) => Promise<void>;
}

function sleep(ms: number, signal?: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(resolve, ms);
    signal?.addEventListener("abort", () => {
      clearTimeout(timer);
      reject(signal.reason);
    });
  });
}

/** The handed-off game, or an error message for the user. */
export async function findHandoffGame(
  handoff: ChessComHandoff,
  candidates: string[],
  options: FindOptions = {},
): Promise<{ game: OnlineGame } | { error: string }> {
  if (candidates.length === 0) {
    return { error: "Enter your Chess.com username under “Your games” once, then click Game Review again." };
  }
  const attempts = options.attempts ?? 4;
  const find = options.find ?? findChessComGame;
  let lastError: OnlineGamesError | null = null;
  for (let attempt = 0; attempt < attempts; attempt++) {
    if (attempt > 0) await (options.sleep ?? sleep)(options.delayMs ?? 5000, options.signal);
    for (const name of candidates) {
      try {
        const game = await find(name, handoff.type, handoff.id, { signal: options.signal });
        if (game) return { game };
      } catch (err) {
        if (!(err instanceof OnlineGamesError)) throw err;
        // A wrong saved name should not hide the right one.
        if (err.kind === "not_found" || err.kind === "invalid") continue;
        lastError = err;
      }
    }
  }
  if (lastError) return { error: lastError.message };
  return {
    error: `Chess.com game ${handoff.id} is not in the public games of ${candidates.join(", ")}. If it just ended, wait a minute and try again.`,
  };
}
