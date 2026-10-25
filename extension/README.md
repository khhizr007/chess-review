# Chess Review opener (browser extension)

When you click **Game Review** on Chess.com, the game opens in your own Chess Review app in a new
tab instead. Chrome or Edge (Manifest V3, version 118 or later). Not affiliated with Chess.com.

## Install (unpacked)

1. Start the app (`frontend`: `npm run dev`, which serves http://localhost:5173/).
2. Open `chrome://extensions` (or `edge://extensions`) and turn on **Developer mode**.
3. Click **Load unpacked** and pick this `extension` folder.
4. Open the extension's **Details → Extension options**. Check the app address and enter your
   Chess.com username.

## How it works

- `content.js` catches clicks on Chess.com's Game Review links and buttons. It reads only the clicked
  element, never the board, and asks `background.js` to open the app in a new tab. Chess.com's own
  review never starts, so a free account's daily review is not used. That also covers the upgrade
  dialog Chess.com shows instead of a page once those reviews are used up.
- `background.js` also adds a `declarativeNetRequest` redirect rule. Chess.com review pages opened
  any other way (its `window.open`, its same-tab mode) go to `redirect.html`. That page opens the app
  in a new tab and sends a same-tab navigation back to the game.
- The app gets `#chesscom=<live|daily>/<id>&user=<name>` and finds the game in the player's public
  archives (`api.chess.com/pub`, the documented API). See `frontend/src/lib/handoff.ts`.

The URL patterns come from Chess.com's own client (routes `web_analysis_ceac` and
`web_analysis_ceac_tab`, play package 2026.10.8). If Chess.com changes them, update `REVIEW_URL` in
`shared.js` and `content.js` together.

## Test

```
node --test extension/test.mjs
```
