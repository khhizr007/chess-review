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

  it("marks moves with an icon as Chess.com does", () => {
    const plies = [
      ply(1, "white", 1, { classification: "book" }),
      ply(2, "black", 1, { classification: "book" }),
      ply(3, "white", 2),
      ply(4, "black", 2, { classification: "inaccuracy" }),
      ply(5, "white", 3, { classification: "miss" }),
      ply(6, "black", 3, { classification: "blunder" }),
      ply(7, "white", 4, { classification: "best", is_key_moment: true }),
      ply(8, "black", 4, { classification: "best" }),
    ];
    render(<MoveList plies={plies} current={0} filter="all" onSelect={() => {}} />);
    const icon = (n: number) => screen.getByText(`S${n}`).closest("button")?.querySelector("svg")?.getAttribute("aria-label") ?? null;
    // Only the last book move, the blunder and the best move at a key moment.
    expect([1, 2, 3, 4, 5, 6, 7, 8].map(icon)).toEqual([null, "Book", null, null, null, "Blunder", "Best", null]);
  });

  it("ends with the result, leaving Black's cell after the last move empty", () => {
    const plies = [ply(1, "white", 1), ply(2, "black", 1), ply(3, "white", 2)];
    render(<MoveList plies={plies} current={3} filter="all" onSelect={() => {}} result="1-0" />);
    const rows = screen.getAllByRole("row");
    expect(rows).toHaveLength(3);
    expect(rows[1]?.textContent).toBe("2.S3");
    expect(rows[2]?.textContent).toBe("1-0");
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

describe("MoveList sideline classifications", () => {
  it("marks each judged sideline move with its icon", () => {
    const NONE = { check: false, checkmate: false, stalemate: false, capture: false, castle: false, promotion: false, en_passant: false };
    const mv = (san: string) => ({ san, uci: san, fen: "", from: "a1", to: "a2", flags: NONE });
    render(
      <MoveList
        plies={[ply(1, "white", 1), ply(2, "black", 1)]}
        current={1}
        filter="all"
        onSelect={() => {}}
        sideline={{
          line: { fromPly: 1, moves: [mv("c5"), mv("d4")], cursor: 1 },
          branchFen: "x b KQkq - 0 1",
          onSelect: () => {},
          classes: ["mistake", null],
        }}
      />,
    );
    expect(screen.getByRole("img", { name: "Mistake" })).toBeTruthy();
    expect(document.querySelector(`[data-sideline="1"]`)?.getAttribute("title")).toBe("Mistake");
    expect(document.querySelector(`[data-sideline="2"]`)?.getAttribute("title")).toBeNull();
  });
});
