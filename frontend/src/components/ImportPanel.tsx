import { useEffect, useState, type DragEvent } from "react";
import { ApiError, api } from "../api/client";
import type { Classification } from "../api/generated/Classification";
import type { CreateReviewRequest } from "../api/generated/CreateReviewRequest";
import type { GameSummary } from "../api/generated/GameSummary";
import type { Health } from "../api/generated/Health";
import type { ParseEntry } from "../api/generated/ParseEntry";
import type { Side } from "../api/generated/Side";
import { uciSquares } from "../lib/fen";
import { mySide, myUsernames, rememberChoice, rememberUsername } from "../lib/players";
import { SAMPLE_PGN } from "../samples";
import { MiniBoard } from "./MiniBoard";
import { OnlineImport } from "./OnlineImport";

interface Props {
  /** Starts the review; the parsed game lets the review layout show the board while it runs. */
  onStart: (req: CreateReviewRequest, game: GameSummary | null) => void;
  busy: boolean;
}

/** The diagram shown before a game is picked: the Opera Game, Paris 1858, after 16.Qb8+!! */
const FIGURE = {
  fen: "1Q2kb1r/p2n1ppp/4q3/4p1B1/4P3/8/PPP2PPP/2KR4 b k - 1 16",
  lastMove: { from: "b3", to: "b8" },
  badge: { square: "b8", value: "brilliant" as Classification },
  players: "P. Morphy – Duke of Brunswick & Count Isouard",
  where: "Paris Opera, 1858",
  move: "16.Qb8+",
  note: "Morphy gives his queen. After 16…Nxb8, 17.Rd8 is mate.",
};

/** "24…Qxf2#" for the last move of a game. */
function lastMoveLabel(game: GameSummary): string | null {
  const last = game.moves[game.moves.length - 1];
  if (!last) return null;
  const blackFirst = game.start_fen.split(" ")[1] === "b";
  const ply = game.moves.length - 1 + (blackFirst ? 1 : 0);
  const number = Math.floor(ply / 2) + 1;
  return `${number}${ply % 2 === 0 ? "." : "…"}${last.san}`;
}

export function ImportPanel({ onStart, busy }: Props) {
  const [pgn, setPgn] = useState("");
  const [games, setGames] = useState<ParseEntry[] | null>(null);
  const [selected, setSelected] = useState(0);
  const [parseError, setParseError] = useState<string | null>(null);
  const [parsing, setParsing] = useState(false);
  const [health, setHealth] = useState<Health | null>(null);
  const [healthError, setHealthError] = useState<string | null>(null);
  const [dragging, setDragging] = useState(false);
  const [pgnOpen, setPgnOpen] = useState(false);
  // The side the user played, if any: the coach then talks to them.
  const [side, setSide] = useState<Side | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    api
      .health(controller.signal)
      .then(setHealth)
      .catch((err: unknown) => {
        if (!controller.signal.aborted) setHealthError(err instanceof Error ? err.message : String(err));
      });
    return () => controller.abort();
  }, []);

  // Parsing is cheap; re-parse shortly after the text stops changing.
  useEffect(() => {
    if (!pgn.trim()) {
      setGames(null);
      setParseError(null);
      return;
    }
    const timer = setTimeout(async () => {
      setParsing(true);
      try {
        const res = await api.parseGames(pgn);
        setGames(res.games);
        setParseError(null);
        const firstOk = res.games.findIndex((g) => g.game !== null);
        setSelected(firstOk >= 0 ? firstOk : 0);
      } catch (err) {
        setGames(null);
        setParseError(err instanceof ApiError || err instanceof Error ? err.message : String(err));
      } finally {
        setParsing(false);
      }
    }, 350);
    return () => clearTimeout(timer);
  }, [pgn]);

  const onFile = async (file: File | undefined) => {
    if (!file) return;
    setPgnOpen(true);
    if (file.size > 5 * 1024 * 1024) {
      setParseError("The file is larger than 5 MB.");
      return;
    }
    setPgn(await file.text());
  };

  const selectedEntry = games?.[selected];
  const selectedGame = selectedEntry?.game ?? null;

  // A game whose PGN names one of the user's usernames starts on that side.
  useEffect(() => {
    setSide(selectedGame ? mySide(selectedGame, myUsernames()) : null);
  }, [selectedGame]);
  const canStart = !busy && !!health?.engine && !!selectedEntry?.game && selectedEntry.game.ply_count > 0;

  // A PGN file dropped anywhere on the page is read.
  const drop = {
    onDragOver: (e: DragEvent) => {
      if (!e.dataTransfer.types.includes("Files")) return;
      e.preventDefault();
      setDragging(true);
    },
    onDragLeave: (e: DragEvent) => {
      if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setDragging(false);
    },
    onDrop: (e: DragEvent) => {
      e.preventDefault();
      setDragging(false);
      void onFile(e.dataTransfer.files[0]);
    },
  };

  // The diagram shows the picked game's final position, from the user's side; until then, a famous one.
  const lastMove = selectedGame?.moves[selectedGame.moves.length - 1];
  const figure = selectedGame
    ? {
        fen: lastMove?.fen ?? selectedGame.start_fen,
        lastMove: lastMove ? uciSquares(lastMove.uci) : null,
        badge: null,
        players: `${selectedGame.white ?? "White"} – ${selectedGame.black ?? "Black"}`,
        where: [selectedGame.event, selectedGame.date?.replaceAll(".", "-")].filter((s) => s && !s.includes("?")).join(", "),
        move: lastMoveLabel(selectedGame),
        note: selectedGame.result && selectedGame.result !== "*" ? `Final position, ${selectedGame.result}.` : "Final position.",
      }
    : FIGURE;

  return (
    <section className={`landing ${dragging ? "is-dragging" : ""}`} aria-labelledby="import-title" {...drop}>
      <div className="landing__hero">
        <div className="landing__copy">
          <p className="landing__kicker">Game review</p>
          <h1 id="import-title" className="landing__title">
            Review your games and learn from your mistakes, for free.
          </h1>
          <p className="landing__lede">Enter your Chess.com or Lichess username and pick a game. No account needed.</p>

          {healthError && <div className="notice notice--error">Cannot reach the server: {healthError}</div>}
          {health && !health.engine && (
            <div className="notice notice--error">
              The engine is unavailable: {health.engine_error ?? "not configured"}. Set <code>STOCKFISH_PATH</code> and
              restart the backend.
            </div>
          )}

          <OnlineImport
            currentPgn={pgn}
            onPick={(game, username) => {
              rememberUsername(username);
              setPgn(game.pgn);
            }}
          />

          <details className="pgn-drawer" open={pgnOpen} onToggle={(e) => setPgnOpen(e.currentTarget.open)}>
            <summary>Or paste a PGN</summary>
            <div className="pgn-drawer__body">
              <div className="pgn-drawer__head">
                <label htmlFor="pgn-input" className="card__title">
                  PGN
                </label>
                <div className="row gap-s">
                  <label className="button button--ghost">
                    Open file…
                    <input
                      type="file"
                      accept=".pgn,.txt,application/x-chess-pgn,text/plain"
                      className="visually-hidden"
                      onChange={(e) => void onFile(e.target.files?.[0])}
                    />
                  </label>
                  <button type="button" className="button button--ghost" onClick={() => setPgn(SAMPLE_PGN)}>
                    Load sample
                  </button>
                </div>
              </div>
              <textarea
                id="pgn-input"
                className="pgn-input"
                spellCheck={false}
                placeholder={'[Event "…"]\n[White "…"]\n[Black "…"]\n\n1. e4 e5 2. Nf3 …'}
                value={pgn}
                onChange={(e) => setPgn(e.target.value)}
              />
            </div>
          </details>
          {parsing && <p className="muted small">Checking PGN…</p>}
          {parseError && <div className="notice notice--error">{parseError}</div>}

          {games && (
            <div className="setup">
              <h2 className="card__title">Game</h2>
              <ul className="game-list" role="radiogroup" aria-label="Games in the PGN">
                {games.map((entry) => (
                  <li key={entry.index}>
                    <label className={`game-option ${entry.game ? "" : "game-option--error"}`}>
                      <input
                        type="radio"
                        name="game"
                        disabled={!entry.game}
                        checked={selected === entry.index}
                        onChange={() => setSelected(entry.index)}
                      />
                      {entry.game ? (
                        <span>
                          <strong>
                            {entry.game.white ?? "White"} – {entry.game.black ?? "Black"}
                          </strong>
                          <span className="muted small">
                            {[entry.game.event, entry.game.date, entry.game.result, `${entry.game.ply_count} plies`]
                              .filter(Boolean)
                              .join(" · ")}
                          </span>
                        </span>
                      ) : (
                        <span>
                          <strong>Game {entry.index + 1}: cannot be reviewed</strong>
                          <span className="small">{entry.error_message}</span>
                        </span>
                      )}
                    </label>
                  </li>
                ))}
              </ul>

              {selectedGame && (
                <>
                  <h2 className="card__title">This is me</h2>
                  <fieldset className="segmented segmented--sides" aria-label="Which side you played">
                    {(
                      [
                        { id: "white", label: selectedGame.white ?? "White", hint: "White" },
                        { id: "black", label: selectedGame.black ?? "Black", hint: "Black" },
                        { id: null, label: "Neither", hint: "neutral coach" },
                      ] as const
                    ).map((o) => (
                      <label key={o.id ?? "none"} className={side === o.id ? "is-active" : ""}>
                        <input
                          type="radio"
                          name="player-side"
                          className="visually-hidden"
                          checked={side === o.id}
                          onChange={() => setSide(o.id)}
                        />
                        {o.label}
                        <span className="small muted">{o.hint}</span>
                      </label>
                    ))}
                  </fieldset>
                </>
              )}

              <button
                type="button"
                className="button button--primary button--block setup__start"
                disabled={!canStart}
                onClick={() => {
                  if (selectedGame) rememberChoice(selectedGame, side);
                  // Depth 18 and the server's default engine lines, as Chess.com's review offers no choice.
                  onStart({ pgn, game_index: selected, strength: "standard", ...(side ? { player_side: side } : {}) }, selectedGame);
                }}
              >
                {busy ? "Starting…" : "Start review"}
              </button>
              {health?.engine && (
                <p className="engine-status">
                  <span className="engine-status__dot" aria-hidden="true" />
                  {health.engine} ready
                </p>
              )}
            </div>
          )}
        </div>

        <figure className="landing__figure">
          <MiniBoard
            fen={figure.fen}
            flipped={!!selectedGame && side === "black"}
            lastMove={figure.lastMove}
            badge={figure.badge}
            label={`${figure.players}${figure.move ? `, after ${figure.move}` : ""}`}
          />
          <figcaption>
            <span className="landing__figure-players">{figure.players}</span>
            {figure.where && <span className="landing__figure-where">{figure.where}</span>}
            <span className="landing__figure-note">
              {figure.move && <strong>{figure.move}{figure.badge ? "!!" : ""}</strong>} {figure.note}
            </span>
          </figcaption>
        </figure>
      </div>

      {dragging && (
        <div className="dropzone" aria-hidden="true">
          <strong>Drop the PGN file</strong>
          <span>.pgn or .txt, up to 5 MB</span>
        </div>
      )}
    </section>
  );
}
