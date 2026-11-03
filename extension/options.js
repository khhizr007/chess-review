import { APP_URL, loadSettings, normaliseAppUrl } from "./shared.js";

const USERNAME = /^[A-Za-z0-9_-]{2,30}$/;
const SITE_HINT = "Only change this if you run your own copy of KnightReview.";
const form = document.getElementById("form");
const fields = {
  enabled: document.getElementById("enabled"),
  appUrl: document.getElementById("appUrl"),
  username: document.getElementById("username"),
};
const status = document.getElementById("status");
const appStatus = document.getElementById("appStatus");
const advanced = document.getElementById("advanced");
let statusTimer;

function say(element, text, tone) {
  element.textContent = text;
  const base = element === status ? "status" : "hint";
  element.className = tone ? `${base} status--${tone}` : base;
}

/** Whether a custom review site answers (an opaque request: no page data is read). */
async function checkSite(url) {
  if (url === APP_URL) {
    say(appStatus, SITE_HINT);
    return;
  }
  say(appStatus, "Checking the site…");
  try {
    await fetch(url, { mode: "no-cors", cache: "no-store", signal: AbortSignal.timeout(5000) });
    say(appStatus, "Site reachable.", "ok");
  } catch {
    say(appStatus, "Can't reach this site. Check the address and try again.", "error");
  }
}

const settings = await loadSettings();
fields.enabled.checked = settings.enabled;
fields.appUrl.value = settings.appUrl;
fields.username.value = settings.username;
// Keep a custom site visible so it is never changed by surprise.
advanced.open = settings.appUrl !== APP_URL;
void checkSite(settings.appUrl);

document.getElementById("resetUrl").addEventListener("click", () => {
  fields.appUrl.value = APP_URL;
  say(appStatus, SITE_HINT);
});

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  clearTimeout(statusTimer);
  const appUrl = normaliseAppUrl(fields.appUrl.value);
  if (!appUrl) {
    advanced.open = true;
    say(status, "Enter a site address starting with https://.", "error");
    fields.appUrl.focus();
    return;
  }
  const username = fields.username.value.trim();
  if (username && !USERNAME.test(username)) {
    say(status, "Usernames are 2–30 letters, numbers, _ or -.", "error");
    fields.username.focus();
    return;
  }
  await chrome.storage.sync.set({ enabled: fields.enabled.checked, username });
  // The default is not stored, so users follow it if it ever moves.
  if (appUrl === APP_URL) await chrome.storage.sync.remove("appUrl");
  else await chrome.storage.sync.set({ appUrl });
  fields.appUrl.value = appUrl;
  say(status, "Settings saved.", "ok");
  statusTimer = setTimeout(() => say(status, ""), 3000);
  void checkSite(appUrl);
});
