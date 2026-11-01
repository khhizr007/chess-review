import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PlyReview } from "../api/generated/PlyReview";
import type { CoachMessage } from "../lib/coach";
import { CoachActions, CoachBubble, MoveVerdict, ShowLineMoves, type FragmentMarking } from "./Coach";

afterEach(cleanup);

const plain = (text: string) => ({ text, arrows: [], squares: [] });
const qb3 = { text: "Qb3", arrows: [{ from: "f3", to: "b3", kind: "move" as const }], squares: [] };
const pawns = { text: "Black's pawns on b7 and f7", arrows: [], squares: ["b7", "f7"] };

const fen = "rn1qkb1r/ppp2ppp/5n2/4p3/2B1P3/5Q2/PPP2PPP/RNB1K2R w KQkq - 2 7";
const ply = {
  ply: 12,
  move_number: 6,
  mover: "black",
  san: "Nf6",
  eval_after: { kind: "cp", cp: 222 },
  pgn_comment: null,
  coach: {
    sentences: [{ fragments: [plain("After Nf6, "), qb3, plain(" forks "), pawns, plain(".")], event: 0 }],
    show_lines: [
      {
        label: "Show the fork",
        start_fen: fen,
        moves: [
          { san: "Qb3", uci: "f3b3", fen: "a" },
          { san: "Bc5", uci: "f8c5", fen: "b" },
          { san: "Bxf7+", uci: "c4f7", fen: "c" },
        ],
        narration: ["Qb3 forks.", "Bc5.", "Bxf7+ takes."],
        event: 0,
      },
    ],
    events: [],
  },
} as unknown as PlyReview;

const message: CoachMessage = {
  classification: "mistake",
  verdict: "is a mistake",
  advice: [],
  sentences: ply.coach.sentences,
  facts: [],
  swing: null,
};

function marking(over: Partial<FragmentMarking> = {}): FragmentMarking {
  return { active: null, pinned: null, onHover: vi.fn(), onToggle: vi.fn(), ...over };
}

describe("MoveVerdict", () => {
  it("makes marked fragments focusable buttons that report hover, focus and taps", () => {
    const m = marking();
    render(<MoveVerdict ply={ply} message={message} marking={m} />);
    const btn = screen.getByRole("button", { name: "Black's pawns on b7 and f7" });
    fireEvent.focus(btn);
    expect(m.onHover).toHaveBeenLastCalledWith({ sentence: 0, fragment: 3 });
    fireEvent.blur(btn);
    expect(m.onHover).toHaveBeenLastCalledWith(null);
    fireEvent.click(btn);
    expect(m.onToggle).toHaveBeenCalledWith({ sentence: 0, fragment: 3 });
    // Plain fragments stay text.
    expect(screen.queryByRole("button", { name: /forks/ })).toBeNull();
  });

  it("shows which fragment is active and pinned", () => {
    const ref = { sentence: 0, fragment: 1 };
    render(<MoveVerdict ply={ply} message={message} marking={marking({ active: ref, pinned: ref })} />);
    const btn = screen.getByRole("button", { name: "Qb3" });
    expect(btn.className).toContain("is-active");
    expect(btn.getAttribute("aria-pressed")).toBe("true");
  });

  it("offers a button per show-line, and none without a handler", () => {
    const onShowLine = vi.fn();
    const { unmount } = render(<MoveVerdict ply={ply} message={message} onShowLine={onShowLine} />);
    fireEvent.click(screen.getByRole("button", { name: "Show the fork" }));
    expect(onShowLine).toHaveBeenCalledWith(0);
    unmount();
    render(<MoveVerdict ply={ply} message={message} />);
    expect(screen.queryByRole("button", { name: "Show the fork" })).toBeNull();
  });

  it("works for a move without a coach note", () => {
    const bare = { ...ply, coach: undefined } as unknown as PlyReview;
    render(<MoveVerdict ply={bare} message={{ ...message, sentences: [], advice: ["Qf6 was better."] }} onShowLine={() => {}} />);
    expect(screen.getByText("Qf6 was better.")).toBeTruthy();
  });
});

describe("ShowLineMoves", () => {
  it("numbers the line from its start position and selects a move", () => {
    const onSelect = vi.fn();
    render(<ShowLineMoves line={ply.coach.show_lines[0]!} move={1} onSelect={onSelect} />);
    const buttons = screen.getAllByRole("button");
    expect(buttons.map((b) => b.textContent)).toEqual(["7.Qb3", "Bc5", "8.Bxf7+"]);
    expect(buttons[1]?.getAttribute("aria-current")).toBe("step");
    fireEvent.click(buttons[2]!);
    expect(onSelect).toHaveBeenCalledWith(2);
  });
});

describe("CoachActions", () => {
  it("shows only the icon of an icon-only action and names it for screen readers", () => {
    const onClick = vi.fn();
    render(
      <CoachActions
        actions={[
          { label: "Best", icon: <svg />, iconOnly: true, onClick, hint: "Play the best move, d5" },
          { label: "Resume", icon: <svg />, onClick: () => {} },
        ]}
      />,
    );
    const best = screen.getByRole("button", { name: "Best" });
    expect(best.textContent).toBe("");
    expect(best.title).toBe("Play the best move, d5");
    expect(screen.getByRole("button", { name: "Resume" }).textContent).toBe("Resume");
    fireEvent.click(best);
    expect(onClick).toHaveBeenCalled();
  });
});

describe("CoachBubble", () => {
  it("lights up for Brilliant and Great in their own colours, not for other moves", () => {
    const bubble = (flash: "brilliant" | "great_find" | "best") =>
      render(<CoachBubble flash={flash}>text</CoachBubble>).container.querySelector(".coach__bubble")!.className;
    expect(bubble("brilliant")).toContain("coach__bubble--brilliant");
    expect(bubble("great_find")).toContain("coach__bubble--great");
    expect(bubble("best")).not.toMatch(/--brilliant|--great/);
  });
});
