// The landing page's FAQ. index.html carries the same questions and answers as FAQPage JSON-LD
// for search engines; faq.test.ts fails if the two drift apart.

export interface FaqEntry {
  q: string;
  a: string;
}

export const FAQ: FaqEntry[] = [
  {
    q: "Is KnightReview really free?",
    a: "Yes. Every review is free, with no daily limit and no account. The code is open source, so it stays free.",
  },
  {
    q: "Do I need an account?",
    a: "No. Enter your Chess.com or Lichess username, or paste a PGN, and start the review.",
  },
  {
    q: "Does it work with Chess.com and Lichess games?",
    a: "Yes. It loads your recent public games from both sites. You can also paste or open a PGN from anywhere.",
  },
  {
    q: "Can I review a game straight from Chess.com?",
    a: "Yes. Add the free KnightReview extension to Chrome or Edge. When you click Review on a finished Chess.com game, the game opens in KnightReview instead, so your daily Chess.com review is not used.",
  },
  {
    q: "How is a Brilliant move decided?",
    a: "A Brilliant move is the best move, or very close to it, that gives up a piece. Your position must still be good afterwards, and you must not already be winning easily.",
  },
  {
    q: "How is accuracy worked out?",
    a: "Each move is scored by how much of your winning chances it keeps. Accuracy is the average for each player, and is also shown for the opening, middlegame and endgame.",
  },
  {
    q: "Is KnightReview part of Chess.com or Lichess?",
    a: "No. KnightReview is an independent open-source project. It is not affiliated with Chess.com or Lichess.",
  },
];
