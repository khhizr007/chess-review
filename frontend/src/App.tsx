import { useEffect } from "react";
import { ImportPanel } from "./components/ImportPanel";
import { ClassificationIconDefs } from "./components/ClassificationIcon";
import { CoachBubble } from "./components/Coach";
import { Logo } from "./components/Logo";
import { PieceDefs } from "./components/pieces";
import { ReviewScreen } from "./components/ReviewScreen";
import { useReviewJob } from "./hooks/useReviewJob";

/** Review id kept in the URL hash so a refresh reopens the same review. */
function hashId(): string | null {
  const m = /^#review=([0-9a-f-]{36})$/i.exec(window.location.hash);
  return m?.[1] ?? null;
}

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

export function App() {
  const { view, start, follow, cancel, reset } = useReviewJob();

  useEffect(() => {
    const id = hashId();
    if (id) void follow(id);
  }, [follow]);

  useEffect(() => {
    if (view.kind === "done") window.history.replaceState(null, "", `#review=${view.id}`);
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
