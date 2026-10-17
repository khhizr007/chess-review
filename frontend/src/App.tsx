import { useEffect } from "react";
import { ImportPanel } from "./components/ImportPanel";
import { ReviewScreen } from "./components/ReviewScreen";
import { useReviewJob } from "./hooks/useReviewJob";

/** Review id kept in the URL hash so a refresh reopens the same review. */
function hashId(): string | null {
  const m = /^#review=([0-9a-f-]{36})$/i.exec(window.location.hash);
  return m?.[1] ?? null;
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
      <header className="topbar">
        <span className="brand">
          <span className="brand__mark" aria-hidden="true" />
          Game Review
        </span>
      </header>
      <main className="main">
        {(view.kind === "idle" || view.kind === "submitting") && (
          <ImportPanel onStart={(req) => void start(req)} busy={view.kind === "submitting"} />
        )}

        {(view.kind === "running" || view.kind === "loading") && (
          <section className="card progress-card" aria-live="polite">
            <h1>Reviewing…</h1>
            {view.kind === "running" ? (
              <>
                <progress
                  max={Math.max(1, view.status.progress.total)}
                  value={view.status.progress.done}
                  aria-label="Review progress"
                />
                <p className="muted">
                  {view.status.state === "queued"
                    ? "Waiting for a free engine…"
                    : `Analysed ${view.status.progress.done} of ${view.status.progress.total} positions`}
                </p>
                <button type="button" className="button" onClick={() => void cancel()}>
                  Cancel
                </button>
              </>
            ) : (
              <p className="muted">Loading results…</p>
            )}
          </section>
        )}

        {view.kind === "done" && <ReviewScreen review={view.result} onNewGame={startOver} />}

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
