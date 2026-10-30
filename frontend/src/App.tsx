import { useEffect, useState } from "react";
import type { GameSummary } from "./api/generated/GameSummary";
import { ImportPanel } from "./components/ImportPanel";
import { ClassificationIconDefs } from "./components/ClassificationIcon";
import { Logo } from "./components/Logo";
import { PieceDefs } from "./components/pieces";
import { ReviewLoading } from "./components/ReviewLoading";
import { ReviewScreen } from "./components/ReviewScreen";
import { GitHubStars, SiteFooter } from "./components/SiteChrome";
import { useReviewJob } from "./hooks/useReviewJob";
import { api } from "./api/client";
import { findHandoffGame, handoffCandidates, parseHandoffHash } from "./lib/handoff";
import { parseReviewHash, reviewHash } from "./lib/hash";

/** The move a reloaded page showed in this review, from the URL hash. */
function hashMove(id: string): number | null {
  const hash = parseReviewHash(window.location.hash);
  return hash?.id === id ? hash.move : null;
}

export function App() {
  const { view, start, follow, cancel, reset } = useReviewJob();
  // The game as parsed at import, so the board and players show while the review runs.
  const [game, setGame] = useState<GameSummary | null>(null);
  // A game opened from the browser extension: null, being looked up, or why it failed.
  const [handoff, setHandoff] = useState<{ state: "finding" } | { state: "error"; message: string } | null>(() =>
    parseHandoffHash(window.location.hash) ? { state: "finding" } : null,
  );
  const reviewLayout = view.kind === "running" || view.kind === "loading" || view.kind === "done";

  useEffect(() => {
    const id = parseReviewHash(window.location.hash)?.id;
    if (id) void follow(id);
  }, [follow]);

  useEffect(() => {
    const link = parseHandoffHash(window.location.hash);
    if (!link) return;
    const controller = new AbortController();
    void (async () => {
      try {
        const found = await findHandoffGame(link, handoffCandidates(link), { signal: controller.signal });
        if (controller.signal.aborted) return;
        if ("error" in found) {
          setHandoff({ state: "error", message: found.error });
          return;
        }
        const { pgn, userSide } = found.game;
        const parsed = await api.parseGames(pgn);
        if (controller.signal.aborted) return;
        window.history.replaceState(null, "", window.location.pathname);
        setHandoff(null);
        setGame(parsed.games[0]?.game ?? null);
        void start({ pgn, game_index: 0, strength: "standard", ...(userSide ? { player_side: userSide } : {}) });
      } catch (err) {
        if (controller.signal.aborted) return;
        setHandoff({ state: "error", message: err instanceof Error ? err.message : String(err) });
      }
    })();
    return () => controller.abort();
  }, [start]);

  useEffect(() => {
    // A refresh reopens the same review. The review screen adds the move it shows.
    if (view.kind === "done" && parseReviewHash(window.location.hash)?.id !== view.id) {
      window.history.replaceState(null, "", reviewHash(view.id, null));
    }
  }, [view]);

  const startOver = () => {
    window.history.replaceState(null, "", window.location.pathname);
    setGame(null);
    setHandoff(null);
    reset();
  };

  return (
    <div className="app">
      <PieceDefs />
      <ClassificationIconDefs />
      {!reviewLayout && (
        <header className="topbar">
          <Logo />
          <GitHubStars />
        </header>
      )}
      <main className={reviewLayout ? "main main--review" : "main"}>
        {handoff && view.kind === "idle" && (
          <section className="card progress-card">
            <h1>{handoff.state === "finding" ? "Opening your Chess.com game" : "Could not open the game"}</h1>
            {handoff.state === "finding" ? (
              <p className="muted">Looking it up in your public Chess.com games.</p>
            ) : (
              <>
                <div className="notice notice--error">{handoff.message}</div>
                <button type="button" className="button button--primary" onClick={startOver}>
                  Back to import
                </button>
              </>
            )}
          </section>
        )}

        {!handoff && (view.kind === "idle" || view.kind === "submitting") && (
          <ImportPanel
            onStart={(req, picked) => {
              setGame(picked);
              void start(req);
            }}
            busy={view.kind === "submitting"}
          />
        )}

        {(view.kind === "running" || view.kind === "loading") && (
          <ReviewLoading
            game={game}
            progress={view.kind === "loading" ? { done: 1, total: 1 } : view.status.state === "queued" ? null : view.status.progress}
            status={
              view.kind === "loading"
                ? "Almost there. Putting the review together."
                : view.status.state === "queued"
                  ? "Waiting for a free engine."
                  : `Checked ${view.status.progress.done} of ${view.status.progress.total} positions.`
            }
            onCancel={view.kind === "running" ? () => void cancel() : undefined}
          />
        )}

        {view.kind === "done" && <ReviewScreen review={view.result} reviewId={view.id} initialPly={hashMove(view.id)} onNewGame={startOver} />}

        {(view.kind === "error" || view.kind === "cancelled") && (
          <section className="card progress-card">
            <h1>{view.kind === "cancelled" ? "Review cancelled" : "Review failed"}</h1>
            {view.kind === "error" && <div className="notice notice--error">{view.message}</div>}
            <button type="button" className="button button--primary" onClick={startOver}>
              Back to import
            </button>
          </section>
        )}
      </main>
      {!reviewLayout && <SiteFooter />}
    </div>
  );
}
