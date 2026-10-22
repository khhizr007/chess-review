import { useEffect } from "react";
import { ImportPanel } from "./components/ImportPanel";
import { ClassificationIconDefs } from "./components/ClassificationIcon";
import { CoachBubble } from "./components/Coach";
import { Logo } from "./components/Logo";
import { PieceDefs } from "./components/pieces";
import { ReviewScreen } from "./components/ReviewScreen";
import { useReviewJob } from "./hooks/useReviewJob";
import { parseReviewHash, reviewHash } from "./lib/hash";

function ProgressBar({ done, total }: { done: number; total: number }) {
  const pct = Math.round((Math.min(done, total) / total) * 100);
  return (
    <div className="progress" role="progressbar" aria-label="Review progress" aria-valuemin={0} aria-valuemax={100} aria-valuenow={pct}>
      <div className="progress__track">
        <div className="progress__fill" style={{ width: `${pct}%` }} />
      </div>
      <span className="progress__pct">{pct}%</span>
    </div>
  );
}

/** The move a reloaded page showed in this review, from the URL hash. */
function hashMove(id: string): number | null {
  const hash = parseReviewHash(window.location.hash);
  return hash?.id === id ? hash.move : null;
}

export function App() {
  const { view, start, follow, cancel, reset } = useReviewJob();

  useEffect(() => {
    const id = parseReviewHash(window.location.hash)?.id;
    if (id) void follow(id);
  }, [follow]);

  useEffect(() => {
    // A refresh reopens the same review. The review screen adds the move it shows.
    if (view.kind === "done" && parseReviewHash(window.location.hash)?.id !== view.id) {
      window.history.replaceState(null, "", reviewHash(view.id, null));
    }
  }, [view]);

  const startOver = () => {
    window.history.replaceState(null, "", window.location.pathname);
    reset();
  };

  return (
    <div className="app">
      <PieceDefs />
      <ClassificationIconDefs />
      {view.kind !== "done" && (
        <header className="topbar">
          <Logo />
        </header>
      )}
      <main className={view.kind === "done" ? "main main--review" : "main"}>
        {(view.kind === "idle" || view.kind === "submitting") && (
          <ImportPanel onStart={(req) => void start(req)} busy={view.kind === "submitting"} />
        )}

        {(view.kind === "running" || view.kind === "loading") && (
          <section className="card progress-card" aria-live="polite">
            <CoachBubble>
              <div className="coach__head">
                <h1 className="coach__title">Analyzing your game…</h1>
              </div>
              <div className="coach__body">
                <p>
                  {view.kind === "loading"
                    ? "Almost there. Putting the review together."
                    : view.status.state === "queued"
                      ? "Waiting for a free engine."
                      : `Checked ${view.status.progress.done} of ${view.status.progress.total} positions.`}
                </p>
              </div>
            </CoachBubble>
            <ProgressBar
              done={view.kind === "running" ? view.status.progress.done : 1}
              total={view.kind === "running" ? Math.max(1, view.status.progress.total) : 1}
            />
            {view.kind === "running" && (
              <button type="button" className="button" onClick={() => void cancel()}>
                Cancel
              </button>
            )}
          </section>
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
    </div>
  );
}
