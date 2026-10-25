import { loadSettings, normaliseAppUrl } from "./shared.js";

const USERNAME = /^[A-Za-z0-9_-]{2,30}$/;
const form = document.getElementById("form");
const fields = {
  enabled: document.getElementById("enabled"),
  appUrl: document.getElementById("appUrl"),
  username: document.getElementById("username"),
};
const status = document.getElementById("status");
const appStatus = document.getElementById("appStatus");

function say(element, text, tone) {
  element.textContent = text;
  const base = element === status ? "status" : "hint";
  element.className = tone ? `${base} status--${tone}` : base;
}

/** Whether something answers at the app address (an opaque request: no page data is read). */
async function checkApp(url) {
  say(appStatus, "Checking the app…");
  try {
    await fetch(url, { mode: "no-cors", cache: "no-store", signal: AbortSignal.timeout(3000) });
    say(appStatus, "The app is running.", "ok");
  } catch {
    say(appStatus, "Nothing answers at this address. Start the app (npm run dev in frontend).", "error");
  }
}

const settings = await loadSettings();
fields.enabled.checked = settings.enabled;
fields.appUrl.value = settings.appUrl;
fields.username.value = settings.username;
void checkApp(settings.appUrl);

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  const appUrl = normaliseAppUrl(fields.appUrl.value);
  if (!appUrl) {
    say(status, "The app address must start with http:// or https://.", "error");
    fields.appUrl.focus();
    return;
  }
  const username = fields.username.value.trim();
  if (username && !USERNAME.test(username)) {
    say(status, "A Chess.com username has 2–30 letters, digits, _ or -.", "error");
    fields.username.focus();
    return;
  }
  await chrome.storage.sync.set({ enabled: fields.enabled.checked, appUrl, username });
  fields.appUrl.value = appUrl;
  say(status, "Saved.", "ok");
  void checkApp(appUrl);
});
