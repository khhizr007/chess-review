// Run with `node --test extension/` (Node 20+). Checks the URL rules without a browser.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { REVIEW_URL, appReviewUrl, normaliseAppUrl } from "./shared.js";

const review = new RegExp(REVIEW_URL, "i");

test("matches both Game Review URL forms, with language prefixes and subdomains", () => {
  const cases = [
    ["https://www.chess.com/analysis/game/live/185885368440?tab=review", "live", "185885368440"],
    ["https://www.chess.com/analysis/game/live/185885368440/review", "live", "185885368440"],
    ["https://www.chess.com/analysis/game/live/185885368440/review?move=12", "live", "185885368440"],
    ["https://www.chess.com/analysis/game/daily/912?flip=true&tab=review", "daily", "912"],
    ["https://www.chess.com/es/analysis/game/live/7?tab=review&move=3", "live", "7"],
    ["https://chess.com/analysis/game/live/7/review", "live", "7"],
    ["https://schach.chess.com/analysis/game/live/7/review", "live", "7"],
  ];
  for (const [url, type, id] of cases) {
    const m = review.exec(url);
    assert.ok(m, url);
    assert.deepEqual([m[1], m[2]], [type, id], url);
  }
});

test("leaves other pages alone", () => {
  for (const url of [
    "https://www.chess.com/analysis/game/live/7",
    "https://www.chess.com/analysis/game/live/7?tab=analysis",
    "https://www.chess.com/analysis/game/live/7/analysis",
    "https://www.chess.com/analysis/game/live/7?tab=reviews",
    "https://www.chess.com/analysis/game/computer/7/review",
    "https://www.chess.com/game/live/7",
    "https://evilchess.com/analysis/game/live/7/review",
    "https://www.chess.com.evil.example/analysis/game/live/7/review",
    "http://www.chess.com/analysis/game/live/7/review",
  ]) {
    assert.equal(review.exec(url), null, url);
  }
});

test("content.js uses the same pattern as shared.js", () => {
  const source = readFileSync(new URL("./content.js", import.meta.url), "utf8");
  const literal = /new RegExp\(\s*("(?:[^"\\]|\\.)*")/.exec(source)?.[1];
  assert.equal(JSON.parse(literal), REVIEW_URL);
});

test("builds the app link the frontend reads (frontend/src/lib/handoff.ts)", () => {
  const settings = { appUrl: "http://localhost:5173/", username: "khhizr7", enabled: true };
  assert.equal(appReviewUrl(settings, "live", "42"), "http://localhost:5173/#chesscom=live/42&user=khhizr7");
  assert.equal(appReviewUrl({ ...settings, username: "" }, "daily", "1"), "http://localhost:5173/#chesscom=daily/1");
  assert.equal(appReviewUrl({ ...settings, username: "bad name" }, "live", "1"), "http://localhost:5173/#chesscom=live/1");
  assert.throws(() => appReviewUrl(settings, "live", "1&user=x"));
  assert.throws(() => appReviewUrl(settings, "computer", "1"));
});

test("accepts only http(s) app addresses and drops their hash", () => {
  assert.equal(normaliseAppUrl(" http://localhost:5173 "), "http://localhost:5173/");
  assert.equal(normaliseAppUrl("https://example.org/app/#x"), "https://example.org/app/");
  assert.equal(normaliseAppUrl("javascript:alert(1)"), null);
  assert.equal(normaliseAppUrl("not a url"), null);
});
