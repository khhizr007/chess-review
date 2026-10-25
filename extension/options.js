import { loadSettings, normaliseAppUrl } from "./shared.js";

const form = document.getElementById("form");
const fields = {
  enabled: document.getElementById("enabled"),
  appUrl: document.getElementById("appUrl"),
  username: document.getElementById("username"),
};
const status = document.getElementById("status");

const settings = await loadSettings();
fields.enabled.checked = settings.enabled;
fields.appUrl.value = settings.appUrl;
fields.username.value = settings.username;

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  const appUrl = normaliseAppUrl(fields.appUrl.value);
  if (!appUrl) {
    status.textContent = "The app address must start with http:// or https://.";
    return;
  }
  await chrome.storage.sync.set({ enabled: fields.enabled.checked, appUrl, username: fields.username.value.trim() });
  fields.appUrl.value = appUrl;
  status.textContent = "Saved.";
});
