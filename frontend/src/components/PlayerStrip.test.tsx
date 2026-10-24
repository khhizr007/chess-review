import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { PlayerStrip } from "./PlayerStrip";

afterEach(cleanup);

const material = { captured: [], lead: 0 };

describe("PlayerStrip", () => {
  it("shows the clock, light with an icon for the side to move", () => {
    const { container } = render(
      <>
        <PlayerStrip side="white" name="khhizr7" elo={835} material={material} toMove={false} clock={597_500} />
        <PlayerStrip side="black" name="siddytopcoder" elo={780} material={material} toMove clock={600_000} />
      </>,
    );
    const white = screen.getByLabelText("White's clock: 9:57");
    const black = screen.getByLabelText("Black's clock: 10:00");
    expect(white.textContent).toBe("9:57");
    expect(white.classList.contains("is-running")).toBe(false);
    expect(white.querySelector(".clock__icon")).toBeNull();
    expect(black.classList.contains("is-running")).toBe(true);
    expect(black.querySelector(".clock__icon")).not.toBeNull();
    // The clock replaces the "To move" pill.
    expect(container.querySelector(".player-strip__turn")).toBeNull();
  });

  it("keeps the strip without clocks", () => {
    const { container } = render(
      <PlayerStrip side="white" name="Alice" elo={null} material={material} toMove clock={null} />,
    );
    expect(container.querySelector(".clock")).toBeNull();
    expect(container.querySelector(".player-strip__turn")).not.toBeNull();
  });
});
