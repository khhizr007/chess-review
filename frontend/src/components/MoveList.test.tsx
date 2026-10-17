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

  it("shows only matching moves when filtered", () => {
    const plies = [ply(1, "white", 1), ply(2, "black", 1, { classification: "blunder", ep_lost: 0.4 })];
    render(<MoveList plies={plies} current={0} filter="attention" onSelect={() => {}} />);
    expect(screen.queryByText("S1")).toBeNull();
    expect(screen.getByText("S2")).toBeTruthy();
    expect(screen.getByText(/Blunder · −40%/)).toBeTruthy();
  });
});
