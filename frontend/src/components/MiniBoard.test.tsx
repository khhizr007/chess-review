import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { MiniBoard } from "./MiniBoard";

afterEach(cleanup);

const OPERA = "1Q2kb1r/p2n1ppp/4q3/4p1B1/4P3/8/PPP2PPP/2KR4 b k - 1 16";

describe("MiniBoard", () => {
  it("draws 64 squares with a1 dark and h1 light, even with an empty rank", () => {
    const { container } = render(<MiniBoard fen={OPERA} label="Opera" />);
    const squares = container.querySelectorAll(".mini-board__sq");
    expect(squares).toHaveLength(64);
    // White at the bottom: a1 is square 57, h1 is 64, a8 is 1.
    expect(squares[56]?.classList.contains("is-dark")).toBe(true);
    expect(squares[63]?.classList.contains("is-light")).toBe(true);
    expect(squares[0]?.classList.contains("is-light")).toBe(true);
  });

  it("puts the badge on its square", () => {
    const { container } = render(<MiniBoard fen={OPERA} label="Opera" badge={{ square: "b8", value: "brilliant" }} />);
    expect(container.querySelectorAll(".mini-board__sq")[1]?.classList.contains("has-badge")).toBe(true);
  });
});
