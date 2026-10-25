// Service worker: keeps the redirect rule in step with the settings and opens the app for clicks
// that content.js caught on Chess.com.
import { OTHER_TAB, REVIEW_URL, appReviewUrl, loadSettings } from "./shared.js";

const RULE_ID = 1;
const OTHER_TAB_RULE_ID = 2;

/**
 * Any Game Review page load (new tab, same tab, typed or bookmarked) goes to redirect.html,
 * which opens the app. This catches the navigations content.js cannot see, such as
 * Chess.com's own `window.open(reviewUrl)` and `location.href = reviewUrl`.
 */
async function syncRules() {
  const { enabled } = await loadSettings();
  const addRules = enabled
    ? [
        {
          id: RULE_ID,
          priority: 1,
          action: {
            type: "redirect",
            redirect: { regexSubstitution: `${chrome.runtime.getURL("redirect.html")}?type=\\1&id=\\2` },
          },
          condition: { regexFilter: REVIEW_URL, isUrlFilterCaseSensitive: false, resourceTypes: ["main_frame"] },
        },
        {
          // The analysis board, explorer, … of the same game stay on Chess.com.
          id: OTHER_TAB_RULE_ID,
          priority: 2,
          action: { type: "allow" },
          condition: { regexFilter: OTHER_TAB, isUrlFilterCaseSensitive: false, requestDomains: ["chess.com"], resourceTypes: ["main_frame"] },
        },
      ]
    : [];
  await chrome.declarativeNetRequest.updateDynamicRules({ removeRuleIds: [RULE_ID, OTHER_TAB_RULE_ID], addRules });
}

chrome.runtime.onInstalled.addListener(() => void syncRules());
chrome.runtime.onStartup.addListener(() => void syncRules());
// The toolbar button opens the settings.
chrome.action.onClicked.addListener(() => void chrome.runtime.openOptionsPage());
chrome.storage.onChanged.addListener((_changes, area) => {
  if (area === "sync") void syncRules();
});

chrome.runtime.onMessage.addListener((message, sender) => {
  if (message?.kind !== "open-review") return;
  void (async () => {
    const settings = await loadSettings();
    const url = appReviewUrl(settings, message.type, message.id);
    const tab = sender.tab;
    await chrome.tabs.create(tab ? { url, index: tab.index + 1, openerTabId: tab.id } : { url });
  })();
});
