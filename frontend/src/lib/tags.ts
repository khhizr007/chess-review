import type { Tag } from "../api/generated/Tag";

/** How the UI names a coach tag outside the backend's sentences (hints, takeaways). */
export interface TagWords {
  /** Singular with its article: "a fork". */
  one: string;
  /** Plural without a count: "forks". */
  many: string;
  /** Tags of the same family are counted together in takeaways (the mates). */
  family?: string;
  /** For tags that describe the mover's own error (BASE events): "left a piece hanging". */
  error?: { one: string; many: (n: number) => string };
  /** Retry hint for the error tags, said without naming the move. */
  hint?: string;
  /** What a good move does as a matter of course (a recapture, a defence): not praised in takeaways. */
  routine?: boolean;
}

const MATE: TagWords = { one: "a mating attack", many: "mating attacks", family: "mate" };

// Partial on purpose: the backend may add tags before the UI names them; `tagWords` falls back.
const TAG_WORDS: Partial<Record<Tag, TagWords>> = {
  back_rank_mate: { ...MATE, one: "a back-rank mate" },
  checkmate: { ...MATE, one: "a checkmate" },
  forced_mate: { ...MATE, one: "a forced mate" },
  free_piece: { one: "a free piece", many: "free pieces" },
  wins_trade: { one: "a trade that wins material", many: "trades that win material" },
  trapped_piece: { one: "a trapped piece", many: "trapped pieces" },
  double_check: { one: "a double check", many: "double checks" },
  royal_fork: { one: "a fork of the king", many: "forks", family: "fork" },
  fork: { one: "a fork", many: "forks", family: "fork" },
  discovered_check: { one: "a discovered check", many: "discovered checks", family: "discovery" },
  discovered_attack: { one: "a discovered attack", many: "discovered attacks", family: "discovery" },
  skewer: { one: "a skewer", many: "skewers" },
  exploits_pin: { one: "a pin", many: "pins", family: "pin" },
  pin: { one: "a pin", many: "pins", family: "pin" },
  removes_defender: { one: "a way to remove a defender", many: "defender removals" },
  overloaded: { one: "an overloaded defender", many: "overloaded defenders" },
  attraction: { one: "a decoy", many: "decoys" },
  promotion: { one: "a promotion", many: "promotions" },
  moves_pinned_piece: {
    one: "a pinned piece moving",
    many: "pinned pieces moving",
    error: { one: "Moved a pinned piece", many: (n) => `Moved pinned pieces ${n} times` },
    hint: "One of your pieces is pinned. Mind what it is shielding.",
  },
  did_not_recapture: {
    one: "a recapture",
    many: "recaptures",
    error: { one: "Didn't take back", many: (n) => `Didn't take back ${n} times` },
    hint: "Material is out of balance. Can you restore it?",
  },
  ignores_threat: {
    one: "a threat",
    many: "threats",
    error: { one: "Ignored a threat", many: (n) => `Ignored ${n} threats` },
    hint: "Something of yours is under attack.",
  },
  removes_own_defender: {
    one: "a defender leaving",
    many: "defenders leaving",
    error: { one: "Moved away a defender", many: (n) => `Moved away ${n} defenders` },
    hint: "Keep an eye on what your pieces are guarding.",
  },
  bad_trade: {
    one: "a bad trade",
    many: "bad trades",
    error: { one: "Made a bad trade", many: (n) => `Made ${n} bad trades` },
    hint: "Count the material before you trade.",
  },
  hangs_piece: {
    one: "a hanging piece",
    many: "hanging pieces",
    family: "hangs",
    error: { one: "Left a piece hanging", many: (n) => `Left ${n} pieces hanging` },
    hint: "Check that all your pieces stay protected.",
  },
  hangs_pawn: {
    one: "a hanging pawn",
    many: "hanging pawns",
    error: { one: "Left a pawn hanging", many: (n) => `Left ${n} pawns hanging` },
    hint: "Check that your pawns stay protected.",
  },
  loses_trade: {
    one: "a losing trade",
    many: "losing trades",
    family: "trade",
    error: { one: "Lost a trade", many: (n) => `Lost ${n} trades` },
    hint: "Count the attackers and defenders before you capture.",
  },
  recapture: { one: "a recapture", many: "recaptures", routine: true },
  equal_trade: { one: "an even trade", many: "even trades", routine: true },
  blocks_check: { one: "a way to block the check", many: "check blocks", family: "check", routine: true },
  evades_check: { one: "a way out of check", many: "ways out of check", family: "check", routine: true },
  defends_piece: { one: "a way to defend a piece", many: "defences", routine: true },
  escapes_threat: { one: "a way to save a piece", many: "escapes", routine: true },
  counter_threat: { one: "a counter-attack", many: "counter-attacks", routine: true },
  kicks: { one: "a pawn move that chases a piece", many: "pawn moves that chase pieces", family: "tempo", routine: true },
  gains_tempo: { one: "an attack that gains time", many: "attacks that gain time", family: "tempo", routine: true },
  offers_trade: { one: "a trade offer", many: "trade offers", routine: true },
  sacrifice: { one: "a sacrifice", many: "sacrifices", family: "sacrifice" },
  offers_piece: { one: "a sacrifice", many: "sacrifices", family: "sacrifice" },
  only_move: { one: "the only good move", many: "only good moves" },
  slide: {
    one: "a slide in the position",
    many: "slides in the position",
    error: { one: "Let the position slip", many: (n) => `Let the position slip ${n} times` },
    hint: "Look for the move that keeps your position together.",
  },
  material_gain: { one: "a way to win material", many: "chances to win material" },
};

const FALLBACK: TagWords = { one: "a tactic", many: "tactics" };

export function tagWords(tag: Tag): TagWords {
  return TAG_WORDS[tag] ?? FALLBACK;
}

/** Key under which takeaways count a tag (its family, or the tag itself). */
export function tagFamily(tag: Tag): string {
  return tagWords(tag).family ?? tag;
}
