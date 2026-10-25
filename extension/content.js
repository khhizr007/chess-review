// Runs on chess.com pages. It only looks at what was clicked: never the board, the moves or the
// clocks. A click on a Game Review link or button opens the game in the app in a new tab instead.
// Catching the click (not only the page load, see background.js) matters because Chess.com shows an
// upgrade dialog instead of opening any page once a free account's daily reviews are used up.
(() => {
  // Same pattern as REVIEW_URL in shared.js (test.mjs checks they agree).
  const REVIEW_URL = new RegExp(
    "^https://(?:[a-z0-9-]+\\.)?chess\\.com/(?:[a-z]{2}(?:[-_][a-z]{2})?/)?analysis/game/(live|daily)/([0-9]+)(?:/review)?/?(?:[?#].*)?$",
    "i",
  );
  // Same pattern as OTHER_TAB in shared.js.
  const OTHER_TAB = new RegExp("[?&]tab=(?:analysis|games|explorer|tablebase|collection-settings)(?:[&#]|$)", "i");
  // A finished game's own page, `/game/{live|daily}/{id}` (optionally behind a language prefix).
  const GAME_PAGE = /^\/(?:[a-z]{2}(?:[-_][a-z]{2})?\/)?game\/(live|daily)\/([0-9]+)\/?$/i;

  let enabled = true;
  chrome.storage.sync.get({ enabled: true }, (s) => (enabled = s.enabled !== false));
  chrome.storage.onChanged.addListener((changes, area) => {
    if (area === "sync" && changes.enabled) enabled = changes.enabled.newValue !== false;
  });

  /** The game a click asks Chess.com to review, or null. */
  function reviewTarget(event) {
    for (const node of event.composedPath()) {
      if (!(node instanceof Element)) continue;
      if (node instanceof HTMLAnchorElement && node.href) {
        const m = REVIEW_URL.exec(node.href);
        if (m && !OTHER_TAB.test(node.href)) return { type: m[1].toLowerCase(), id: m[2] };
      }
      // Buttons without a link (the sidebar's "Game Review" on a finished game's page).
      if (node.matches("button, [role=button], a")) {
        const label = (node.getAttribute("aria-label") || node.textContent || "").trim();
        const page = GAME_PAGE.exec(location.pathname);
        if (label === "Game Review" && page) return { type: page[1].toLowerCase(), id: page[2] };
        return null;
      }
    }
    return null;
  }

  window.addEventListener(
    "click",
    (event) => {
      if (!enabled || event.button !== 0) return;
      // After the extension is reloaded or removed, this old copy can no longer reach it.
      if (!chrome.runtime?.id) return;
      const target = reviewTarget(event);
      if (!target) return;
      // Capture phase on window runs before Chess.com's own handlers, so its review never starts.
      event.preventDefault();
      event.stopImmediatePropagation();
      chrome.runtime.sendMessage({ kind: "open-review", ...target });
    },
    true,
  );
})();
