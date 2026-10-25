// Shared by the service worker, the redirect page and the options page (content.js keeps its own
// copy of REVIEW_URL, since content scripts cannot import modules; test.mjs checks the two agree).

/**
 * A Chess.com game's review page: `/analysis/game/{live|daily}/{id}`, optionally followed by
 * `/review`, a query (`?tab=review`, `?flip=true`, `?move=12`) or a hash, behind an optional
 * language prefix (`/es/...`) on any chess.com subdomain. Chess.com builds these for its Game Review
 * button (routes `web_analysis_ceac_tab` and `web_analysis_ceac` with `tab: "review"`, play package
 * 2026.10.8) and for the archive's Review button (`web_analysis_ceac` with no tab, which opens
 * Game Review; archive-index bundle). Other tabs are excluded by OTHER_TAB.
 * Groups: 1 = game type, 2 = game id.
 *
 * Games against bots (`computer`) and coach games are not matched: Chess.com's public API does not
 * list them, so the app could not load them.
 */
export const REVIEW_URL =
  "^https://(?:[a-z0-9-]+\\.)?chess\\.com/(?:[a-z]{2}(?:[-_][a-z]{2})?/)?analysis/game/(live|daily)/([0-9]+)(?:/review)?/?(?:[?#].*)?$";

/** An analysis page asked for a tab other than the review (left to Chess.com). */
export const OTHER_TAB = "[?&]tab=(?:analysis|games|explorer|tablebase|collection-settings)(?:[&#]|$)";

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
  if (type !== "live" && type !== "daily") throw new Error("This is not a Chess.com game link.");
  if (!/^[0-9]+$/.test(id)) throw new Error("This is not a Chess.com game link.");
  const user = USERNAME.test(settings.username) ? `&user=${settings.username}` : "";
  return `${settings.appUrl}#chesscom=${type}/${id}${user}`;
}
