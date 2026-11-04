# Chrome Web Store listing

Copy these into the Developer Dashboard. Keep them in sync with `manifest.json`.

## Store listing

**Name:** KnightReview: Free Chess Game Review (from `manifest.json`)

**Summary (132 characters max):** from `manifest.json` `description`.

**Category:** Entertainment (Games is not offered for extensions)

**Language:** English

**Description:**

> Review your Chess.com games and learn from your mistakes, for free.
>
> Click Review on any finished Chess.com game and it opens in KnightReview in a new tab. See where
> the game turned, your best moves and your mistakes, your accuracy and an estimated rating for
> each player.
>
> How it works
> • Install the extension and enter your Chess.com username (optional, but it helps find your games).
> • Play on Chess.com as usual.
> • Click Review, from the game page or your game archive. The review opens in KnightReview.
>
> Reviews opened this way do not use up your Chess.com Game Review allowance.
>
> Good to know
> • Works with live and daily games. Games against Chess.com bots are not in Chess.com's public
>   games list, so they still open on Chess.com.
> • Turn the extension off at any time from its settings to use Chess.com's own review.
> • No ads, no tracking, no account needed.
>
> KnightReview is open source and is not affiliated with, endorsed by or sponsored by Chess.com.
> Chess.com is a trademark of Chess.com, LLC.

**Homepage URL:** https://knightreview.online/

**Support URL:** https://github.com/khhizr007/knightreview/issues

## Graphics (you make these)

| Asset | Size | Required |
| --- | --- | --- |
| Store icon | 128×128 PNG | yes: `icons/icon-128.png` |
| Screenshots | 1280×800 or 640×400, 1–5 | yes, at least 1 |
| Small promo tile | 440×280 | yes |
| Marquee promo tile | 1400×560 | no |

Good screenshots: the Review button on Chess.com, a KnightReview review opened from it, and the
extension settings page.

## Privacy practices tab

**Single purpose:** Opens the user's finished Chess.com games in KnightReview for a free game review
when they click Review on Chess.com.

**Permission justifications:**

- `declarativeNetRequestWithHostAccess`: Redirects Chess.com Game Review page loads
  (`chess.com/analysis/game/live|daily/<id>`) to the extension's page, which opens the same game in
  KnightReview. Only main-frame navigations to that URL pattern are redirected.
- `storage`: Saves the user's settings: on/off, optional Chess.com username, optional review site.
- Host permission `https://*.chess.com/*`: Needed to redirect Game Review pages on chess.com and
  its language subdomains, and for the content script that catches clicks on the Review button.
- Content script on chess.com: Checks only whether a click was on a Game Review link or button,
  then opens that game in KnightReview. It does not read page content.

**Remote code:** No, I am not using remote code. (All code is in the package.)

**Data usage:** Tick **Personally identifiable information** only if you treat the username as such
(it is a public Chess.com username; ticking it is the safe choice). Tick nothing else. Then tick all
three certifications (no selling, no unrelated use, no credit-worthiness use).

**Privacy policy URL:** https://github.com/khhizr007/knightreview/blob/main/extension/PRIVACY.md
(the site has no `/privacy` page; it would return the home page)
