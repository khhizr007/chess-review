import { useEffect, useState } from "react";
import { ApiError, api } from "../api/client";
import type { CreateReviewRequest } from "../api/generated/CreateReviewRequest";
import type { Health } from "../api/generated/Health";
import type { ParseEntry } from "../api/generated/ParseEntry";
import type { Side } from "../api/generated/Side";
import type { Strength } from "../api/generated/Strength";
import { mySide, myUsernames, rememberChoice, rememberUsername } from "../lib/players";
import { SAMPLE_PGN } from "../samples";
import { OnlineImport } from "./OnlineImport";

interface Props {
  onStart: (req: CreateReviewRequest) => void;
  busy: boolean;
}

const STRENGTHS: { id: Strength; label: string; hint: string }[] = [
  { id: "fast", label: "Fast", hint: "depth 12" },
  { id: "standard", label: "Standard", hint: "depth 18" },
  { id: "deep", label: "Deep", hint: "depth 22" },
  { id: "maximum", label: "Maximum", hint: "depth 26" },
];

export function ImportPanel({ onStart, busy }: Props) {
  const [pgn, setPgn] = useState("");
  const [games, setGames] = useState<ParseEntry[] | null>(null);
  const [selected, setSelected] = useState(0);
  const [parseError, setParseError] = useState<string | null>(null);
  const [parsing, setParsing] = useState(false);
  const [strength, setStrength] = useState<Strength>("standard");
  const [multipv, setMultipv] = useState(3);
  const [health, setHealth] = useState<Health | null>(null);
  const [healthError, setHealthError] = useState<string | null>(null);
  const [dragging, setDragging] = useState(false);
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

  return (
    <section className="import" aria-labelledby="import-title">
      <div className="import__intro">
        <span className="import__eyebrow">Engine-graded review</span>
        <h1 id="import-title">Find the moments that decided your game</h1>
        <p>
          Load one of your recent games from Chess.com or Lichess, or paste a PGN. Every move is searched with the
          engine and graded for accuracy, so you can step through your best moves, mistakes and missed chances.
        </p>
      </div>

      {healthError && <div className="notice notice--error">Cannot reach the server: {healthError}</div>}
      {health && !health.engine && (
        <div className="notice notice--error">
          The engine is unavailable: {health.engine_error ?? "not configured"}. Set <code>STOCKFISH_PATH</code> and
          restart the backend.
        </div>
      )}

      <div className="import__grid">
        <div className="import__main">
          <OnlineImport
            currentPgn={pgn}
            onPick={(game, username) => {
              rememberUsername(username);
              setPgn(game.pgn);
            }}
          />
          <div
            className={`card card--pgn ${dragging ? "is-dragging" : ""}`}
            onDragOver={(e) => {
              if (!e.dataTransfer.types.includes("Files")) return;
              e.preventDefault();
              setDragging(true);
            }}
            onDragLeave={(e) => {
              if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setDragging(false);
            }}
            onDrop={(e) => {
              e.preventDefault();
              setDragging(false);
              void onFile(e.dataTransfer.files[0]);
            }}
          >
            <div className="card__header">
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
            {dragging && (
              <div className="dropzone" aria-hidden="true">
                <strong>Drop the PGN file</strong>
                <span>.pgn or .txt, up to 5 MB</span>
              </div>
            )}
            {parsing && <p className="muted small">Checking PGN…</p>}
            {parseError && <div className="notice notice--error">{parseError}</div>}
          </div>
        </div>

        <div className="card card--settings">
          <h2 className="card__title">Game</h2>
          {!games && <p className="empty-hint">Games found in the PGN will be listed here.</p>}
          {games && (
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
          )}

          {selectedGame && (
            <>
              <h2 className="card__title">This is me</h2>
              <fieldset className="segmented" aria-label="Which side you played">
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

          <h2 className="card__title">Analysis</h2>
          <fieldset className="segmented" aria-label="Strength">
            {STRENGTHS.map((s) => (
              <label key={s.id} className={strength === s.id ? "is-active" : ""}>
                <input
                  type="radio"
                  name="strength"
                  className="visually-hidden"
                  checked={strength === s.id}
                  onChange={() => setStrength(s.id)}
                />
                {s.label}
                <span className="small muted">{s.hint}</span>
              </label>
            ))}
          </fieldset>
          <label className="field">
            <span>Engine lines per position</span>
            <select value={multipv} onChange={(e) => setMultipv(Number(e.target.value))}>
              {[1, 2, 3, 4, 5].map((n) => (
                <option key={n} value={n}>
                  {n}
                </option>
              ))}
            </select>
          </label>
          <button
            type="button"
            className="button button--primary button--block"
            disabled={!canStart}
            onClick={() => {
              if (selectedGame) rememberChoice(selectedGame, side);
              onStart({ pgn, game_index: selected, strength, multipv, ...(side ? { player_side: side } : {}) });
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
      </div>
    </section>
  );
}
