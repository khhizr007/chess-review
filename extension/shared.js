// Shared by the service worker, the redirect page and the options page (content.js keeps its own
// copy of REVIEW_URL, since content scripts cannot import modules; test.mjs checks the two agree).

/**
 * A Chess.com Game Review page: `/analysis/game/{live|daily}/{id}/review` or `...?tab=review`,
 * optionally behind a language prefix (`/es/...`) and on any chess.com subdomain. These are the two
 * URL forms Chess.com's own client builds for its Game Review button (routes `web_analysis_ceac_tab`
 * and `web_analysis_ceac` with `tab: "review"`, play package 2026.10.8). Plain analysis pages
 * (no `review`) are left alone. Groups: 1 = game type, 2 = game id.
 */
export const REVIEW_URL =
  "^https://(?:[a-z0-9-]+\\.)?chess\\.com/(?:[a-z]{2}(?:[-_][a-z]{2})?/)?analysis/game/(live|daily)/([0-9]+)(?:/review/?(?:[?#].*)?|\\?(?:[^#]*&)?tab=review(?:[&#].*)?)$";

export const DEFAULTS = { appUrl: "http://localhost:5173/", username: "", enabled: true };

const USERNAME = /^[A-Za-z0-9_-]{2,30}$/;

/** The settings, with defaults for anything unset or invalid. */
export async function loadSettings() {
  const stored = await chrome.storage.sync.get(DEFAULTS);
  return {
    appUrl: normaliseAppUrl(stored.appUrl) ?? DEFAULTS.appUrl,
    username: USERNAME.test(stored.username) ? stored.username : "",
    enabled: stored.enabled !== false,
  };
}

/** An http(s) URL without its hash, or null. */
export function normaliseAppUrl(value) {
  try {
    const url = new URL(String(value).trim());
    if (url.protocol !== "http:" && url.protocol !== "https:") return null;
    url.hash = "";
    return url.href;
  } catch {
    return null;
  }
}

/** The app page that reviews Chess.com game `id` (see frontend/src/lib/handoff.ts). */
export function appReviewUrl(settings, type, id) {
  if (type !== "live" && type !== "daily") throw new Error(`bad game type ${type}`);
  if (!/^[0-9]+$/.test(id)) throw new Error(`bad game id ${id}`);
  const user = USERNAME.test(settings.username) ? `&user=${settings.username}` : "";
  return `${settings.appUrl}#chesscom=${type}/${id}${user}`;
}
