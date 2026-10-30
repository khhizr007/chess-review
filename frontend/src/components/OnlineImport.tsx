import { useEffect, useRef, useState, type ReactNode } from "react";
import { OnlineGamesError, SITE_NAMES, fetchRecentGames, outcomeFor, type OnlineGame, type Site } from "../lib/onlineGames";

interface Props {
  /** A game chosen from the list; its PGN goes to the importer. */
  onPick: (game: OnlineGame, username: string) => void;
  /** The PGN being imported, to mark the game it came from. */
  currentPgn: string;
  /** Shown right under the picked game: what to do with it (side, Start review). */
  pickedPanel?: ReactNode;
}

const KEY = "chess-review:online-account";
const SITES: Site[] = ["chesscom", "lichess"];

function loadAccount(): { site: Site; username: string } {
  try {
    const saved: unknown = JSON.parse(localStorage.getItem(KEY) ?? "null");
    if (saved && typeof saved === "object" && "site" in saved && "username" in saved) {
      const { site, username } = saved as { site: unknown; username: unknown };
      if ((site === "chesscom" || site === "lichess") && typeof username === "string") return { site, username };
    }
  } catch {
    // Storage may be unavailable (private mode): start empty.
  }
  return { site: "chesscom", username: "" };
}

function saveAccount(site: Site, username: string) {
  try {
    localStorage.setItem(KEY, JSON.stringify({ site, username }));
  } catch {
    // The account just won't be remembered.
  }
}

function playedOn(ms: number): string {
  const date = new Date(ms);
  const sameYear = date.getFullYear() === new Date().getFullYear();
  return date.toLocaleDateString(undefined, { month: "short", day: "numeric", ...(sameYear ? {} : { year: "numeric" }) });
}

const OUTCOME_LABEL = { won: "W", lost: "L", drawn: "D" } as const;

/** Lists a Chess.com or Lichess player's recent games; picking one imports its PGN. */
export function OnlineImport({ onPick, currentPgn, pickedPanel }: Props) {
  const [site, setSite] = useState<Site>(() => loadAccount().site);
  const [username, setUsername] = useState(() => loadAccount().username);
  const [games, setGames] = useState<OnlineGame[] | null>(null);
  const [fetchedFor, setFetchedFor] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const request = useRef<AbortController | null>(null);

  useEffect(() => () => request.current?.abort(), []);

  const fetchGames = async () => {
    const name = username.trim();
    if (!name) return;
    request.current?.abort();
    const controller = new AbortController();
    request.current = controller;
    setLoading(true);
    setError(null);
    try {
      const found = await fetchRecentGames(site, name, { signal: controller.signal });
      setGames(found);
      setFetchedFor(name);
      saveAccount(site, name);
    } catch (err) {
      if (controller.signal.aborted) return;
      setGames(null);
      setError(err instanceof OnlineGamesError ? err.message : `Could not load the games: ${String(err)}`);
    } finally {
      if (request.current === controller) setLoading(false);
    }
  };

  return (
    <div className="card card--online">
      <div className="card__header">
        <h2 className="card__title">Your games</h2>
        <fieldset className="segmented segmented--sites" aria-label="Site">
          {SITES.map((s) => (
            <label key={s} className={site === s ? "is-active" : ""}>
              <input
                type="radio"
                name="online-site"
                className="visually-hidden"
                checked={site === s}
                onChange={() => {
                  setSite(s);
                  setGames(null);
                  setError(null);
                }}
              />
              {SITE_NAMES[s]}
            </label>
          ))}
        </fieldset>
      </div>
      <form
        className="online-form"
        onSubmit={(e) => {
          e.preventDefault();
          void fetchGames();
        }}
      >
        <input
          type="text"
          className="online-form__input"
          aria-label={`${SITE_NAMES[site]} username`}
          placeholder={`${SITE_NAMES[site]} username`}
          autoComplete="username"
          spellCheck={false}
          value={username}
          onChange={(e) => setUsername(e.target.value)}
        />
        <button type="submit" className="button button--primary" disabled={loading || !username.trim()}>
          {loading ? "Loading…" : "Show games"}
        </button>
      </form>
      {error && <div className="notice notice--error">{error}</div>}
      {games && games.length === 0 && (
        <p className="empty-hint">
          No recent standard games for {fetchedFor} on {SITE_NAMES[site]}.
        </p>
      )}
      {games && games.length > 0 && (
        <ul className="online-games" aria-label={`Recent games of ${fetchedFor}`}>
          {games.map((g) => {
            const outcome = outcomeFor(g);
            const opponent = g.userSide === "black" ? g.white : g.black;
            const picked = currentPgn === g.pgn;
            return (
              <li key={g.id} className={picked ? "is-picked" : undefined}>
                <button
                  type="button"
                  className={`online-game ${picked ? "is-picked" : ""}`}
                  aria-pressed={picked}
                  onClick={() => onPick(g, fetchedFor)}
                >
                  <span className={`online-game__outcome online-game__outcome--${outcome ?? "none"}`} aria-label={outcome ?? "no result"}>
                    {outcome ? OUTCOME_LABEL[outcome] : "–"}
                  </span>
                  <span className="online-game__text">
                    <strong>
                      vs {opponent.name}
                      {opponent.rating !== null && <span className="muted"> ({opponent.rating})</span>}
                    </strong>
                    <span className="muted small">
                      {[g.chess960 ? "Chess960" : null, g.speed, g.timeControl, g.rated ? "rated" : "casual", playedOn(g.endedAt)]
                        .filter(Boolean)
                        .join(" · ")}
                    </span>
                  </span>
                  <span className="online-game__side small muted">as {g.userSide ?? "?"}</span>
                </button>
                {picked && pickedPanel}
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
