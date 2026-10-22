import { useEffect, useState } from "react";
import type { GameSummary } from "./api/generated/GameSummary";
import { ImportPanel } from "./components/ImportPanel";
import { ClassificationIconDefs } from "./components/ClassificationIcon";
import { Logo } from "./components/Logo";
import { PieceDefs } from "./components/pieces";
import { ReviewLoading } from "./components/ReviewLoading";
import { ReviewScreen } from "./components/ReviewScreen";
import { useReviewJob } from "./hooks/useReviewJob";

/** Review id kept in the URL hash so a refresh reopens the same review. */
function hashId(): string | null {
  const m = /^#review=([0-9a-f-]{36})$/i.exec(window.location.hash);
  return m?.[1] ?? null;
}

export function App() {
  const { view, start, follow, cancel, reset } = useReviewJob();
  // The game as parsed at import, so the board and players show while the review runs.
  const [game, setGame] = useState<GameSummary | null>(null);
  const reviewLayout = view.kind === "running" || view.kind === "loading" || view.kind === "done";

  useEffect(() => {
    const id = hashId();
    if (id) void follow(id);
  }, [follow]);

  useEffect(() => {
    if (view.kind === "done") window.history.replaceState(null, "", `#review=${view.id}`);
  }, [view]);

  const startOver = () => {
    window.history.replaceState(null, "", window.location.pathname);
    setGame(null);
    reset();
  };

  return (
    <div className="app">
      <PieceDefs />
      <ClassificationIconDefs />
      {!reviewLayout && (
        <header className="topbar">
          <Logo />
        </header>
      )}
      <main className={reviewLayout ? "main main--review" : "main"}>
        {(view.kind === "idle" || view.kind === "submitting") && (
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

        {view.kind === "done" && <ReviewScreen review={view.result} reviewId={view.id} onNewGame={startOver} />}

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
    </div>
  );
}
