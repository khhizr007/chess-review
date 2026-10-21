# Sound

The review plays board and feedback sounds. **When** each sound plays follows the reference client,
recovered with REA on 2026-10-21. The sounds are this project's own. They are synthesised in the
browser (`frontend/src/lib/synth.ts`), and the repository holds no audio files (see `THIRD_PARTY.md`).
The rules are in `frontend/src/lib/sound.ts` and are tested in `sound.test.ts`. The **Sound** switch
under the board turns sound off; the setting is kept in `localStorage` (`review.sound`).

## Sources

- **Web Game Review** (client 2026.10.3-hotfix). Analysed from its bundles: the move-sound chooser in
  the shared eager chunk, and the review and retry handlers in `analysis.js`. Also observed while
  stepping through a signed-in review with `AudioBufferSourceNode.start` hooked. Retry was read from
  code only.
- **Android 4.10.15** (JADX): `CBSoundPlayerImpl`, `soundandhaptics/impl/sound/*` and
  `GameReviewViewModel`. It agrees with the web client except where noted below.

## Move sound

Exactly one sound plays per move. The first rule that matches wins:

| Rule | Sound |
|---|---|
| SAN has `+` or `#` (so a mate counts as a check) | check |
| Promotion, including a capture that promotes | promote |
| Capture, including en passant | capture |
| Castling | castle |
| Anything else | move |

The web client also has a premove rule and an own/opponent split. Neither applies to review: no
"playing as" colour is set there, so quiet moves by both sides play the same sound. Reaching a mate
in review plays only the check sound, with no game-end sound.

## Navigation

| Action | Sound |
|---|---|
| Step forward (→, Next, autoplay) | the move arrived at |
| Click a move or jump to it (End, graph, move list), backwards jumps included | the move arrived at |
| Step back one move (←) | the move **taken back**, not the one landed on |
| Go to the start position (Home) | none |

## Sidelines

The reader's own moves (exploring a sideline, `docs/explore-mode.md`) follow the same rules.
- **Playing a move** plays its move sound.
- **Stepping back one move** plays the move taken back.
- **Stepping forward, or clicking a sideline move,** plays the move arrived at.
- **Brilliant** never sounds in a sideline, because sideline moves are not classified.
- **Playing the game's own move** follows the game and sounds like stepping forward.

This is this app's own choice. The reference client's analysis board was not examined for sound.

## Brilliant

The brilliant sound plays together with the move sound when all of these hold:

- the board arrives at a move classified Brilliant, by stepping forward or by selecting it (never by
  stepping back);
- the move is on the main line, not in a line preview or retry;
- the move was played by the reviewing player.

The reference client checks the last rule against the signed-in username. Here it uses the side
chosen under "This is me" at import, and either side counts when none was chosen.

No other classification has a sound. Great, best, miss, mistake, blunder and the rest are silent on
the web. The Android app gives some of them a haptic only.

## Retry

When a retry is finished, a sound plays for the classification of the move found:

| Retry move | Sound |
|---|---|
| Brilliant, Great, Tricky, Best | correct (two-part rising blip) |
| Excellent | result good (rising three notes) |
| Good, Book, Forced | result ok (up, then down) |
| Inaccuracy, Mistake, Blunder, Miss | result bad (falling three notes) |

The retry move also gets its normal move sound. This app only accepts best or excellent answers, so
"ok" and "bad" are mapped but never reached. A wrong attempt plays an "incorrect" sound. That part is
this app's own choice: the reference client was not observed doing it.

## Not reproduced

- Key Takeaways navigation sound: this app has no takeaway pages after the last move.
- Game start/end, illegal move, premove, low time and draw offer: these are play-only events.
- Engine and coach line previews here play each move's sound. The reference client's line previews
  were not examined.
