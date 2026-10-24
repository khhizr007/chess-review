// Which side the user played. The choice is remembered per username, so a
// later game with the same name in its PGN headers preselects that side.
import type { Side } from "../api/generated/Side";

const KEY = "chess-review:my-usernames";

type Store = Pick<Storage, "getItem" | "setItem">;

function defaultStore(): Store | null {
  try {
    return globalThis.localStorage ?? null;
  } catch {
    return null;
  }
}

function normalise(name: string): string {
  return name.trim().toLowerCase();
}

/** Usernames the user has said are theirs (normalised). */
export function myUsernames(store: Store | null = defaultStore()): string[] {
  try {
    const parsed: unknown = JSON.parse(store?.getItem(KEY) ?? "[]");
    return Array.isArray(parsed) ? parsed.filter((n): n is string => typeof n === "string") : [];
  } catch {
    return [];
  }
}

function save(names: string[], store: Store | null) {
  try {
    store?.setItem(KEY, JSON.stringify(names));
  } catch {
    // Storage may be unavailable (private mode); the choice just won't persist.
  }
}

/**
 * Remembers the user's choice for a game: `name` (the player on the chosen
 * side) is theirs; the other player's name, and any matched name the user
 * said was not them, are not.
 */
export function rememberChoice(
  game: { white: string | null; black: string | null },
  side: Side | null,
  store: Store | null = defaultStore(),
) {
  const mine = side === "white" ? game.white : side === "black" ? game.black : null;
  const notMine = [game.white, game.black].filter((n): n is string => !!n && n !== mine).map(normalise);
  const names = myUsernames(store).filter((n) => !notMine.includes(n));
  if (mine?.trim() && !names.includes(normalise(mine))) names.push(normalise(mine));
  save(names, store);
}

/** Remembers a username as the user's own (one they fetched their games with). */
export function rememberUsername(name: string, store: Store | null = defaultStore()) {
  const names = myUsernames(store);
  if (name.trim() && !names.includes(normalise(name))) save([...names, normalise(name)], store);
}

/** The side whose PGN name is one of the user's usernames, if exactly one is. */
export function mySide(game: { white: string | null; black: string | null }, names: string[]): Side | null {
  const is = (n: string | null) => !!n && names.includes(normalise(n));
  const white = is(game.white);
  const black = is(game.black);
  if (white === black) return null;
  return white ? "white" : "black";
}
