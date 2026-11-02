# OpenChess Game Review (browser extension)

When you click **Review** on a finished Chess.com game, the game opens in
[OpenChess](https://openchess.online/) in a new tab instead. Chrome or Edge (Manifest V3, version
118 or later). Not affiliated with Chess.com.

## Install

From the Chrome Web Store (once published), or unpacked for development:

1. Open `chrome://extensions` (or `edge://extensions`) and turn on **Developer mode**.
2. Click **Load unpacked** and pick this `extension` folder.
3. The settings page opens. Enter your Chess.com username.

Reviews open at https://openchess.online/ by default. To test against a local frontend
(`npm run dev` in `frontend`, http://localhost:5173/), set **Advanced → Review site** in the
extension's settings.

## Release

1. Bump `version` in `manifest.json` (each store upload needs a higher version).
2. `npm run pack` here (or `node pack.mjs`). It runs the tests, then writes
   `dist/openchess-extension-<version>.zip` with only the shipped files.
3. Upload the zip in the Chrome Web Store Developer Dashboard. Listing text, permission
   justifications and the privacy answers are in `STORE.md`; the privacy policy is `PRIVACY.md`.

## How it works

- `content.js` catches clicks on Chess.com's Game Review links and buttons. It reads only the clicked
  element, never the board, and asks `background.js` to open the review site in a new tab.
  Chess.com's own review never starts, so a free account's daily review is not used. That also
  covers the upgrade dialog Chess.com shows instead of a page once those reviews are used up.
- `background.js` also adds a `declarativeNetRequest` redirect rule. Chess.com review pages opened
  any other way (its `window.open`, its same-tab mode) go to `redirect.html`. That page opens the
  review in a new tab and sends a same-tab navigation back to the game.
- The site gets `#chesscom=<live|daily>/<id>&user=<name>` and finds the game in the player's public
  archives (`api.chess.com/pub`, the documented API). See `frontend/src/lib/handoff.ts`.
- On update from a development build, a saved `http://localhost:5173/` address is dropped so the
  default site applies. The default address itself is never stored.

The URL patterns come from Chess.com's own client (routes `web_analysis_ceac` and
`web_analysis_ceac_tab`, play package 2026.10.8; the archive's Review button uses `web_analysis_ceac`
with no tab). If Chess.com changes them, update `REVIEW_URL` and `OTHER_TAB` in `shared.js` and
`content.js` together.

Games against bots and coach games still open on Chess.com: its public API does not list them, so
the site cannot load them.

## Test

```
node --test extension/test.mjs
```
