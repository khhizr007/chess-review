# OpenChess Game Review: privacy policy

Last updated: 2 November 2026

OpenChess Game Review is a browser extension that opens your finished Chess.com games in
[OpenChess](https://openchess.online/) for review.

## What the extension stores

- Your Chess.com username, if you enter one.
- Whether the extension is switched on.
- The review site address, if you change it under Advanced.

These settings are kept in your browser's extension storage (`chrome.storage.sync`). If you are
signed in to Chrome with sync turned on, Chrome syncs them to your other devices. They are never
sent to us.

## What the extension reads

On chess.com pages, the extension only checks whether a click was on a Game Review link or button.
It does not read your games, moves, messages, account details or browsing history, and it does not
use cookies.

## What is sent, and to whom

When you click Review, the extension opens a new tab at openchess.online with the game's ID and
your username (if set) in the address. OpenChess then downloads that game from Chess.com's public
API (`api.chess.com`) and analyses it on the OpenChess servers. See the site for how it handles
reviews.

The extension contains no analytics, ads or trackers. It does not sell or share data with anyone.

## Contact

Questions: open an issue at https://github.com/khhizr007/chess-review/issues
