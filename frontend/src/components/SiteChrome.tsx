import { useEffect, useState } from "react";

const REPO = "khhizr007/chess-review";
const REPO_URL = `https://github.com/${REPO}`;
const CACHE_KEY = "chess-review:github-stars";
/** GitHub allows 60 unauthenticated API calls an hour; one an hour per visitor is plenty. */
const CACHE_MS = 60 * 60 * 1000;

function cachedStars(): number | null {
  try {
    const saved: unknown = JSON.parse(localStorage.getItem(CACHE_KEY) ?? "null");
    if (saved && typeof saved === "object" && "stars" in saved && "at" in saved) {
      const { stars, at } = saved as { stars: unknown; at: unknown };
      if (typeof stars === "number" && typeof at === "number" && Date.now() - at < CACHE_MS) return stars;
    }
  } catch {
    // No storage: ask GitHub.
  }
  return null;
}

/** The repo's star count from GitHub's public API; null while unknown or if GitHub can't be reached. */
function useGitHubStars(): number | null {
  const [stars, setStars] = useState<number | null>(cachedStars);

  useEffect(() => {
    if (stars !== null) return;
    const controller = new AbortController();
    fetch(`https://api.github.com/repos/${REPO}`, { signal: controller.signal, headers: { Accept: "application/vnd.github+json" } })
      .then((res) => (res.ok ? (res.json() as Promise<{ stargazers_count?: unknown }>) : null))
      .then((body) => {
        const count = body?.stargazers_count;
        if (typeof count !== "number") return;
        setStars(count);
        try {
          localStorage.setItem(CACHE_KEY, JSON.stringify({ stars: count, at: Date.now() }));
        } catch {
          // Just not cached.
        }
      })
      .catch(() => {
        // The button still links to the repo without a count.
      });
    return () => controller.abort();
  }, [stars]);

  return stars;
}

function GitHubMark() {
  return (
    <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true" fill="currentColor">
      <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z" />
    </svg>
  );
}

function compact(n: number): string {
  return n < 1000 ? String(n) : `${(n / 1000).toFixed(n < 10000 ? 1 : 0).replace(/\.0$/, "")}k`;
}

/** "Star on GitHub" with the live count, for the top bar. */
export function GitHubStars() {
  const stars = useGitHubStars();
  return (
    <a className="gh-stars" href={REPO_URL} target="_blank" rel="noreferrer">
      <span className="gh-stars__label">
        <GitHubMark />
        Star
      </span>
      {stars !== null && (
        <span className="gh-stars__count" aria-label={`${stars} stars on GitHub`}>
          {compact(stars)}
        </span>
      )}
    </a>
  );
}

export function SiteFooter() {
  return (
    <footer className="site-footer">
      <div className="site-footer__inner">
        <p className="site-footer__line">
          Free and open source under the{" "}
          <a href={`${REPO_URL}/blob/main/LICENSE`} target="_blank" rel="noreferrer">
            GPL-3.0
          </a>
          . Analysis by{" "}
          <a href="https://stockfishchess.org" target="_blank" rel="noreferrer">
            Stockfish
          </a>
          .
        </p>
        <nav className="site-footer__links" aria-label="Project">
          <a href={REPO_URL} target="_blank" rel="noreferrer">
            Source
          </a>
          <a href={`${REPO_URL}/issues`} target="_blank" rel="noreferrer">
            Report a bug
          </a>
        </nav>
        <p className="site-footer__fine">Not affiliated with Chess.com or Lichess.</p>
      </div>
    </footer>
  );
}
