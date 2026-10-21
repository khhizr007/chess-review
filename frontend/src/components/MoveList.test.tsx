import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PlyReview } from "../api/generated/PlyReview";
import { MoveList } from "./MoveList";

afterEach(cleanup);

function ply(n: number, mover: "white" | "black", move_number: number, extra: Partial<PlyReview> = {}): PlyReview {
  return {
    ply: n,
    move_number,
    mover,
    san: `S${n}`,
    classification: "good",
    ep_lost: 0,
    phase: "opening",
    is_key_moment: false,
    is_critical: false,
    ...extra,
  } as PlyReview;
}

describe("MoveList", () => {
  it("lays out moves in numbered rows and handles a Black first move", () => {
    const plies = [ply(1, "black", 5), ply(2, "white", 6), ply(3, "black", 6)];
    const onSelect = vi.fn();
    render(<MoveList plies={plies} current={2} filter="all" onSelect={onSelect} />);
    const rows = screen.getAllByRole("row");
    expect(rows).toHaveLength(2);
    expect(rows[0]?.textContent).toContain("5.");
    expect(rows[0]?.textContent).toContain("…");
    expect(screen.getByText("S2").closest("button")?.getAttribute("aria-current")).toBe("step");
    fireEvent.click(screen.getByText("S3"));
    expect(onSelect).toHaveBeenCalledWith(3);
  });

  it("keeps every move but dims the ones outside the filter", () => {
    const plies = [ply(1, "white", 1), ply(2, "black", 1, { classification: "blunder", ep_lost: 0.4 })];
    render(<MoveList plies={plies} current={0} filter="attention" onSelect={() => {}} />);
    expect(screen.getByText("S1").closest("button")?.className).toContain("is-dim");
    expect(screen.getByText("S2").closest("button")?.className).not.toContain("is-dim");
  });

  it("marks only notable moves with an icon", () => {
    const plies = [ply(1, "white", 1), ply(2, "black", 1, { classification: "blunder" })];
    render(<MoveList plies={plies} current={0} filter="all" onSelect={() => {}} />);
    expect(screen.queryByRole("img", { name: "Good" })).toBeNull();
    expect(screen.getByRole("img", { name: "Blunder" })).toBeTruthy();
  });

  it("writes piece moves in figurine notation without losing the SAN text", () => {
    const plies = [ply(1, "white", 1, { san: "Nf3" })];
    render(<MoveList plies={plies} current={1} filter="all" onSelect={() => {}} />);
    expect(screen.getByRole("button").textContent).toBe("Nf3");
  });
});

describe("MoveList sideline", () => {
  const NONE = { check: false, checkmate: false, stalemate: false, capture: false, castle: false, promotion: false, en_passant: false };
  const mv = (san: string) => ({ san, uci: san, fen: "", from: "a1", to: "a2", flags: NONE });
  // 1. e4 e5 2. Nf3 Nc6, with 1... c5 2. Nf3 explored instead of 1... e5.
  const plies = [ply(1, "white", 1), ply(2, "black", 1), ply(3, "white", 2), ply(4, "black", 2)];
  const branchFen = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1";

  it("lists the sideline under the row of the move it replaces, with move numbers", () => {
    const onPick = vi.fn();
    render(
      <MoveList
        plies={plies}
        current={1}
        filter="all"
        onSelect={() => {}}
        sideline={{ line: { fromPly: 1, moves: [mv("c5"), mv("Nf3")], cursor: 2 }, branchFen, onSelect: onPick }}
      />,
    );
    const rows = screen.getAllByRole("row");
    expect(rows).toHaveLength(3);
    expect(rows[1]?.textContent).toBe("1...c52.Nf3");
    expect(document.querySelector(`[data-sideline="2"]`)?.getAttribute("aria-current")).toBe("step");
    // While the board is in the sideline, no game move is current.
    expect(screen.getByText("S1").closest("button")?.getAttribute("aria-current")).toBeNull();
    fireEvent.click(screen.getByText("c5"));
    expect(onPick).toHaveBeenCalledWith(1);
  });

  it("at the branch position, keeps the game move current", () => {
    render(
      <MoveList
        plies={plies}
        current={1}
        filter="all"
        onSelect={() => {}}
        sideline={{ line: { fromPly: 1, moves: [mv("c5")], cursor: 0 }, branchFen, onSelect: () => {} }}
      />,
    );
    expect(screen.getByText("S1").closest("button")?.getAttribute("aria-current")).toBe("step");
  });
});
