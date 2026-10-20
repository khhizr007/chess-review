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
  // Positional ideas: development and castling are routine; the rest are plans worth praising.
  forces_doubled_pawns: { one: "a trade that doubles pawns", many: "trades that double pawns", family: "structure" },
  forces_isolated_pawn: { one: "a trade that isolates a pawn", many: "trades that isolate pawns", family: "structure" },
  strikes_at_center: { one: "a strike at the centre", many: "strikes at the centre", family: "centre" },
  central_break: { one: "a central break", many: "central breaks", family: "centre" },
  puts_pawn_in_center: { one: "a pawn in the centre", many: "pawns in the centre", family: "centre", routine: true },
  attacks_pawn_chain: { one: "an attack on a pawn chain", many: "attacks on pawn chains" },
  prepares_central_push: { one: "a central push prepared", many: "central pushes prepared", family: "centre", routine: true },
  rook_on_seventh: { one: "a rook on the seventh", many: "rooks on the seventh" },
  outpost: { one: "an outpost", many: "outposts" },
  takes_open_file: { one: "an open file", many: "open files" },
  battery: { one: "a battery", many: "batteries" },
  centralizes_knight: { one: "a centralised knight", many: "centralised knights", family: "activity" },
  moves_to_active_square: { one: "a more active square", many: "more active squares", family: "activity" },
  bishop_gains_scope: { one: "a longer diagonal", many: "longer diagonals", family: "activity" },
  castles_same_side: { one: "castling", many: "castling moves", family: "castling", routine: true },
  castles_opposite_side: { one: "castling", many: "castling moves", family: "castling", routine: true },
  castles_kingside: { one: "castling", many: "castling moves", family: "castling", routine: true },
  castles_queenside: { one: "castling", many: "castling moves", family: "castling", routine: true },
  connects_rooks: { one: "connected rooks", many: "rook connections", routine: true },
  fianchetto: { one: "a fianchetto", many: "fianchettos", family: "development", routine: true },
  prepares_to_castle_queenside: { one: "a way cleared to castle", many: "ways cleared to castle", family: "castling", routine: true },
  prepares_to_castle_kingside: { one: "a way cleared to castle", many: "ways cleared to castle", family: "castling", routine: true },
  develops_center: { one: "a developing move", many: "developing moves", family: "development", routine: true },
  develops_opening: { one: "a developing move", many: "developing moves", family: "development", routine: true },
  develops_middlegame: { one: "a piece brought into play", many: "pieces brought into play", family: "development", routine: true },
  prepares_bishop_development: { one: "a diagonal opened", many: "diagonals opened", family: "development", routine: true },
  pushes_passed_pawn: { one: "a passed pawn push", many: "passed pawn pushes", family: "passed pawn" },
  pushes_passed_pawn_endgame: { one: "a passed pawn push", many: "passed pawn pushes", family: "passed pawn" },
  makes_passed_pawn: { one: "a new passed pawn", many: "new passed pawns", family: "passed pawn" },
  king_activity: { one: "an active king", many: "king walks" },
  creates_luft: { one: "an escape square for the king", many: "escape squares for the king", routine: true },
  // Positional errors.
  loses_castling: {
    one: "a lost right to castle",
    many: "lost rights to castle",
    error: { one: "Gave up castling", many: (n) => `Gave up castling ${n} times` },
    hint: "Your king still wants to castle.",
  },
  doubled_pawns: {
    one: "doubled pawns",
    many: "doubled pawns",
    family: "weak pawns",
    error: { one: "Doubled your pawns", many: (n) => `Doubled your pawns ${n} times` },
    hint: "Mind your pawn structure.",
  },
  isolated_pawn: {
    one: "an isolated pawn",
    many: "isolated pawns",
    family: "weak pawns",
    error: { one: "Isolated a pawn", many: (n) => `Isolated pawns ${n} times` },
    hint: "Mind your pawn structure.",
  },
  backward_pawn: {
    one: "a backward pawn",
    many: "backward pawns",
    family: "weak pawns",
    error: { one: "Left a backward pawn", many: (n) => `Left backward pawns ${n} times` },
    hint: "Mind your pawn structure.",
  },
  weakens_king: {
    one: "a weakened king",
    many: "weakened kings",
    family: "king safety",
    error: { one: "Weakened your king", many: (n) => `Weakened your king ${n} times` },
    hint: "Keep the pawns in front of your king where they are.",
  },
  moves_pawn_in_front_of_king: {
    one: "a loosened king shelter",
    many: "loosened king shelters",
    family: "king safety",
    error: { one: "Loosened your king's shelter", many: (n) => `Loosened your king's shelter ${n} times` },
    hint: "Keep the pawns in front of your king where they are.",
  },
  too_early_queen: {
    one: "an early queen move",
    many: "early queen moves",
    family: "opening",
    error: { one: "Brought the queen out early", many: (n) => `Brought the queen out early ${n} times` },
    hint: "Develop the knights and bishops first.",
  },
  moves_same_piece: {
    one: "a piece moved twice",
    many: "pieces moved twice",
    family: "opening",
    error: { one: "Moved a piece twice in the opening", many: (n) => `Moved a piece twice in the opening ${n} times` },
    hint: "Develop the knights and bishops first.",
  },
  moves_to_passive_square: {
    one: "a passive square",
    many: "passive squares",
    error: { one: "Put a piece on a passive square", many: (n) => `Put pieces on passive squares ${n} times` },
    hint: "Find your pieces squares where they do more.",
  },
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
