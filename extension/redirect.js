// Chess.com's Game Review page was redirected here (background.js). Open the app instead, always
// in a new tab: when Chess.com navigated the game's own tab (its same-tab mode), that tab goes back
// to the game.
import { appReviewUrl, loadSettings } from "./shared.js";

const params = new URLSearchParams(location.search);

try {
  const url = appReviewUrl(await loadSettings(), params.get("type"), params.get("id") ?? "");
  // A tab opened just for the review has one history entry; a tab that was showing the game has more.
  if (history.length > 1) {
    const tab = await chrome.tabs.getCurrent();
    await chrome.tabs.create(tab ? { url, index: tab.index + 1, openerTabId: tab.id } : { url });
    history.back();
  } else {
    location.replace(url);
  }
} catch (err) {
  document.getElementById("status").textContent = `Could not open the review: ${err instanceof Error ? err.message : err}`;
}
