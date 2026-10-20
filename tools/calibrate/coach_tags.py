"""Mapping between our coach vocabulary and Chess.com's board-event tag names (Phase 0).

Our tags are `Tag` in backend/crates/review-server/src/coach/detect/mod.rs (serialised in
snake_case); our circumstances are `Circ` in coach/circ.rs. Chess.com's tag names are the generic
chess terms of their `AnalysisType` enum, written in the CamelCase their coach text keys use
(`HANGS_PIECE` -> `HangsPiece`); see docs/coach-analysis.md §3.4. No Chess.com text is in here.

Families group tags that explain a move the same way, following the version-1 groups of
docs/coach-analysis.md §6.4.
"""

from __future__ import annotations

FAMILIES = (
    "mate",
    "free material",
    "double attack",
    "line tactic",
    "defender tactic",
    "promotion",
    "bad-move base",
    "defence",
    "positional",
    # Chess.com families with no counterpart in our version-1 vocabulary.
    "sacrifice",
    "trade",
    "evaluation",
    "game",
    "other",
)

# Our tag -> (family, the Chess.com tags that say the same thing). The first listed tag is the
# closest one; any listed tag counts as the same tag.
OUR_TAGS: dict[str, tuple[str, tuple[str, ...]]] = {
    # 1. Mates.
    "back_rank_mate": ("mate", ("BackRankMate",)),
    "checkmate": ("mate", ("Checkmate", "LadderMate", "KingQueenMate", "KingRookMate",
                            "DoubleCheckMate", "StillMate", "BackRankMate")),
    "forced_mate": ("mate", ("ForcesMate", "MateThreat", "MaintainedMate", "NmpMateInXThreat",
                             "SlowerMate", "KeymomentGoodFasterMate", "KeymomentBadFasterMate")),
    # 2. Free material.
    "free_piece": ("free material", ("FreePiece", "FreePieceFromDistance", "QuasiFreePiece")),
    "trapped_piece": ("free material", ("AttacksTrappedPiece",)),
    # 3. Double attacks.
    "double_check": ("double attack", ("DoubleCheck",)),
    "royal_fork": ("double attack", ("RoyalFork", "Fork", "ForkWithMate")),
    "fork": ("double attack", ("Fork", "ForkWithMate", "PseudoFork", "MultiAttack", "RoyalFork")),
    "discovered_check": ("double attack", ("DiscoveredCheck", "DiscoveredCheckAttack",
                                           "DiscoveredAttackCheck", "DiscoveredCheckCapture")),
    "discovered_attack": ("double attack", ("DiscoveredAttack", "DiscoveredAttackCapture",
                                            "DiscoveredAttackCheck", "DiscoveredCheckAttack")),
    # 4. Line tactics.
    "skewer": ("line tactic", ("Skewer",)),
    "exploits_pin": ("line tactic", ("ExploitsPinnedDefender", "ThreatensPinned",
                                     "PressuresPinnedPiece")),
    "pin": ("line tactic", ("Pin", "RelativePin", "PinsDefender", "PinsAttacker")),
    # 5. Defender tactics.
    "removes_defender": ("defender tactic", ("RemovesDefender", "CapturesMateDefender",
                                             "RemovesMateDefender", "KicksDefender",
                                             "KicksMateDefender", "ChecksAwayDefender")),
    "overloaded": ("defender tactic", ("ExploitsOverloadedDefender",)),
    "attraction": ("defender tactic", ("Attraction", "Decoy")),
    # 6. Promotion.
    "promotion": ("promotion", ("Promotion", "Underpromotion")),
    # 7. Bad-move bases.
    "moves_pinned_piece": ("bad-move base", ("MovesPinnedPiece",)),
    "did_not_recapture": ("bad-move base", ("DidNotRecapture",)),
    # (Their IgnoresThreat is a sacrifice tag for Brilliant moves, not this.)
    "ignores_threat": ("bad-move base", ("IgnoresAttackedPiece", "IgnoresAttackedPawn")),
    "removes_own_defender": ("bad-move base", ("RemovesOwnDefender",)),
    "bad_trade": ("bad-move base", ("InferiorTrade", "BadTrade", "BadCaptureChoice")),
    "hangs_piece": ("bad-move base", ("HangsPiece",)),
    "hangs_pawn": ("bad-move base", ("HangsPawn",)),
    # 8. Defence (phase 2).
    "defends_piece": ("defence", ("DefendsPiece", "ProtectsPiece", "BlocksAttacker")),
    "escapes_threat": ("defence", ("EscapesThreat", "Retreat", "ObviousRetreat")),
    "blocks_check": ("defence", ("BlocksChecker",)),
    "counter_threat": ("defence", ("CounterThreat",)),
    # Fallback: material won through no named tactic.
    "material_gain": ("free material", ("WinsMaterial", "WinsMaterialNotFree", "LosesMaterial",
                                        "PickUpPieceInTactic", "LosesMaterialLosingPosition",
                                        "LosesMaterialWinningPosition",
                                        "MissesMaterialLosingPosition",
                                        "MissesMaterialWinningPosition")),
}

# Chess.com tags placed in a family by hand; the rest fall back to the range of their priority
# index (CC_RANGES), which follows the groups of docs/coach-analysis.md §3.4.
CC_FAMILY: dict[str, str] = {
    "MateThreat": "mate",
    "NmpMateInXThreat": "mate",
    "CreatesKingFlightSquare": "mate",
    "KeymomentBadFasterMate": "mate",
    "KeymomentGoodFasterMate": "mate",
    "NoLongerMate": "mate",
    "ControlsOpponentPromotionSquare": "promotion",
    "KingReachesSquareOfPawn": "promotion",
    "NmpMutualZugzwang": "positional",
    "Interesting": "other",
    "Promotion": "promotion",
    "Underpromotion": "promotion",
    "UnderpromotionMate": "promotion",
    "WinsMaterial": "free material",
    "WinsMaterialNotFree": "free material",
    "PressuresPinnedPiece": "line tactic",
    "Pin": "line tactic",
    "RelativePin": "line tactic",
    "PinsDefender": "line tactic",
    "PinsAttacker": "line tactic",
    "Attraction": "defender tactic",
    "Decoy": "defender tactic",
    "ForcesDoubledCastlePawns": "positional",
    "ForcesDoubledPawns": "positional",
    "ForcesIsolatedPawn": "positional",
    "ForcesLosesCastlingRights": "positional",
    "WeakenedCastledKing": "positional",
    "OpensUpWeakKing": "positional",
    "DiscoveredCheckCapture": "double attack",
    "ObviousRecapture": "trade",
    "ObviousRetreat": "defence",
    "ExchangeSacrifice": "sacrifice",
    "Desperado": "trade",
    "CaptureThreat": "defence",
    "CaptureThreatFrom": "defence",
    "CaptureThreatTo": "defence",
    "KingsideAttack": "positional",
}
for _tags, _fam in (
    (("LosesMaterialLosingPosition", "LosesMaterialWinningPosition", "MissesMaterialLosingPosition",
      "MissesMaterialWinningPosition", "PickUpPieceInTactic", "FreePieceFromDistance", "FreePiece",
      "QuasiFreePiece", "AttacksTrappedPiece", "TempoBeforeFreePiece"), "free material"),
    (("DiscoveredAttackCapture", "DiscoveredCheckAttack", "DiscoveredAttackCheck", "ForkWithMate",
      "RoyalFork", "Fork", "PseudoFork", "MultiAttack", "DoubleCheck", "DiscoveredCheck",
      "DiscoveredAttack"), "double attack"),
    (("ExploitsPinnedDefender", "Skewer", "ThreatensPinned"), "line tactic"),
    (("ExploitsOverloadedDefender", "KicksDefender", "RemovesDefender", "CaptureDefendsExposedThreat",
      "ChecksAwayDefender", "CapturesMateDefender", "KicksMateDefender", "RemovesMateDefender"),
     "defender tactic"),
    (("RemovesOwnDefender", "MovesPinnedPiece", "DidNotRecapture", "IgnoresAttackedPiece",
      "IgnoresAttackedPawn", "BadCaptureChoice", "HangsPiece", "HangsPawn", "InferiorTrade",
      "LosesMaterial", "MinorsForRookPawn", "QueenForRookMinorBad", "QueenForTwoRooksBad",
      "BadTrade", "MouseSlip", "TradeToLostPawnEndgame", "BadTradeToDrawnPawnEndgame"),
     "bad-move base"),
):
    for _t in _tags:
        CC_FAMILY[_t] = _fam

# (first index, last index, family) over Chess.com's priority order.
CC_RANGES = (
    (0, 13, "other"),
    (14, 25, "sacrifice"),
    (26, 44, "mate"),
    (45, 78, "other"),
    (79, 104, "trade"),
    (105, 139, "defence"),
    (140, 209, "positional"),
    (210, 222, "game"),
    (223, 267, "positional"),
    (268, 277, "evaluation"),
    (278, 281, "other"),
)

# Their circumstance tokens -> ours. Qualifiers that only change the wording (clock remarks, the
# Brilliant/Great framing, "Any") are dropped before comparing chains.
CIRCUMSTANCES = {
    "Permits": "permits",
    "Misses": "misses",
    "MissesSame": "misses_same",
    "LeadsTo": "leads_to",
    "WinsMaterial": "wins_material",
    "Threatens": "threatens",
    "Prevents": "prevents",
    "Forces": "forces",
}
# Our circumstances compared as theirs: MISSES_SAME is worded as a plain miss.
OUR_CIRC_AS_THEIRS = {"base": None, "misses_same": "misses"}


def our_family(tag: str) -> str:
    return OUR_TAGS[tag][0]


def cc_family(tag: str, index: int | None) -> str:
    if tag in CC_FAMILY:
        return CC_FAMILY[tag]
    for our_tag, (fam, cc_tags) in OUR_TAGS.items():
        if tag in cc_tags:
            return fam
    if index is not None:
        for lo, hi, fam in CC_RANGES:
            if lo <= index <= hi:
                return fam
    return "other"


def same_tag(our_tag: str, cc_tag: str) -> bool:
    return cc_tag in OUR_TAGS.get(our_tag, ("", ()))[1]


# Chess.com's material tags carry their circumstance in the tag name: LosesMaterial is said about
# the move itself (no prefix) but means "allows the opponent to win material through no named
# tactic", i.e. our permits > ... > material_gain; MissesMaterial* likewise means misses.
CC_IMPLIED_CHAIN = {
    "LosesMaterial": ("permits",),
    "LosesMaterialLosingPosition": ("permits",),
    "LosesMaterialWinningPosition": ("permits",),
    "MissesMaterialLosingPosition": ("misses",),
    "MissesMaterialWinningPosition": ("misses",),
}


# Steps a tag already implies, dropped before chains are compared: a free piece always wins
# material, and the bare material tags say neither "wins material" nor "eventually".
OUR_IMPLIED_STEPS = {
    "material_gain": ("leads_to", "wins_material"),
    "free_piece": ("wins_material",),
    "trapped_piece": ("wins_material",),
}
CC_IMPLIED_STEPS = {
    "WinsMaterial": ("leads_to", "wins_material"),
    "WinsMaterialNotFree": ("leads_to", "wins_material"),
    "FreePiece": ("wins_material",),
    "QuasiFreePiece": ("wins_material",),
    "FreePieceFromDistance": ("wins_material",),
    "AttacksTrappedPiece": ("wins_material",),
}


# Chess.com says a threat with a tag of its own (the move creates a threat to capture, or a mate
# threat); we say it as THREATENS (or PREVENTS) in front of the tactic or mate that is threatened.
CC_CAPTURE_THREAT = ("CaptureThreat", "CaptureThreatTo", "CaptureThreatFrom")
CC_MATE_THREAT = ("MateThreat", "NmpMateInXThreat")
THREAT_FAMILIES = ("free material", "double attack", "line tactic", "defender tactic")


def threat_match(our_tag: str, our_chain_: tuple[str, ...], cc_tag: str,
                 cc_chain_: tuple[str, ...]) -> str | None:
    """'exact' / 'same tag' when our threat event says what their threat tag says, else None."""
    if cc_tag in CC_CAPTURE_THREAT and our_tag in OUR_TAGS and our_family(our_tag) in THREAT_FAMILIES:
        if "threatens" not in our_chain_:
            return None
        rest = tuple(c for c in our_chain_ if c not in ("threatens", "wins_material", "leads_to"))
        return "exact" if rest == cc_chain_ else "same tag"
    if cc_tag in CC_MATE_THREAT and our_tag in OUR_TAGS and our_family(our_tag) == "mate":
        want = cc_chain_ if "prevents" in cc_chain_ else (*cc_chain_, "threatens")
        if "threatens" not in our_chain_ and "prevents" not in our_chain_:
            return None
        return "exact" if our_chain_ == want else "same tag"
    return None


def our_chain(chain: list[str], tag: str | None = None) -> tuple[str, ...]:
    """Our chain in the comparison vocabulary: BASE is the empty chain."""
    drop = OUR_IMPLIED_STEPS.get(tag, ())
    out = []
    for c in chain:
        c = OUR_CIRC_AS_THEIRS.get(c, c)
        if c and c not in drop:
            out.append(c)
    return tuple(out)


def cc_chain(chain: list[str], tag: str | None = None) -> tuple[str, ...]:
    drop = CC_IMPLIED_STEPS.get(tag, ())
    out = tuple(CIRCUMSTANCES[c] for c in chain if c in CIRCUMSTANCES and CIRCUMSTANCES[c] not in drop)
    if not out and tag in CC_IMPLIED_CHAIN:
        return CC_IMPLIED_CHAIN[tag]
    return out


def mapping_table() -> list[tuple[str, str, str]]:
    """(our tag, family, their tags) rows for the write-up."""
    return [(t, fam, ", ".join(cc)) for t, (fam, cc) in OUR_TAGS.items()]
