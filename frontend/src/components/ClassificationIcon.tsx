import type { ReactNode } from "react";
import type { Classification } from "../api/generated/Classification";
import { CLASSIFICATIONS } from "../lib/format";

const STROKE = { fill: "none", stroke: "#fff", strokeWidth: 2.6, strokeLinecap: "round", strokeLinejoin: "round" } as const;

function text(glyph: string, size: number) {
  return (
    <text x="12" y="12" dy="0.36em" textAnchor="middle" fontSize={size} fontWeight={900} fill="#fff" fontFamily="inherit">
      {glyph}
    </text>
  );
}

function glyph(value: Classification, color: string): ReactNode {
  switch (value) {
    case "brilliant":
      return text("!!", 11.5);
    case "great_find":
      return text("!", 14);
    case "best":
      return <path fill="#fff" d="M12 4.9l2.1 4.3 4.7.7-3.4 3.3.8 4.7-4.2-2.2-4.2 2.2.8-4.7-3.4-3.3 4.7-.7z" />;
    case "excellent":
      return (
        <path
          fill="#fff"
          d="M10 11.2l2.4-4.6c.3-.6 1-.8 1.6-.5.6.3.8.9.6 1.5l-.8 2.6h3.3c.9 0 1.6.8 1.4 1.7l-1.1 4.7c-.2.7-.8 1.2-1.5 1.2H10zM6.2 11.2h2.4v6.6H6.2z"
        />
      );
    case "good":
      return <path {...STROKE} d="M7.6 12.4l3 3 5.8-6" />;
    case "book":
      return (
        <>
          <path fill="#fff" d="M5.6 7.6c2.1-.7 4.4-.4 6.4.9 2-1.3 4.3-1.6 6.4-.9v9.2c-2.1-.7-4.4-.4-6.4.9-2-1.3-4.3-1.6-6.4-.9z" />
          <path stroke={color} strokeWidth={1.2} d="M12 8.6v9" />
        </>
      );
    case "inaccuracy":
      return text("?!", 11.5);
    case "mistake":
      return text("?", 14);
    case "miss":
      return <path {...STROKE} d="M8.6 8.6l6.8 6.8M15.4 8.6l-6.8 6.8" />;
    case "blunder":
      return text("??", 11.5);
    case "forced":
      return <path {...STROKE} d="M7 12h9M12.6 8.4L16.2 12l-3.6 3.6" />;
    case "tricky":
      return text("!?", 11.5);
  }
}

interface Props {
  value: Classification | null;
  /** Rendered size in px. */
  size?: number;
  className?: string;
}

/** Round classification icon, as used in the move list, coach bubble and on the board. */
export function ClassificationIcon({ value, size = 18, className }: Props) {
  if (!value) return null;
  const meta = CLASSIFICATIONS[value];
  return (
    <svg
      className={`cls-icon ${className ?? ""}`}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      role="img"
      aria-label={meta.label}
    >
      <title>{meta.label}</title>
      <circle cx="12" cy="12" r="12" fill={meta.color} />
      <circle cx="12" cy="12" r="12" fill="url(#cls-shade)" />
      {glyph(value, meta.color)}
    </svg>
  );
}

/** Shared gradient that gives every icon the same soft top highlight. Render once. */
export function ClassificationIconDefs() {
  return (
    <svg width="0" height="0" style={{ position: "absolute" }} aria-hidden="true">
      <defs>
        <linearGradient id="cls-shade" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#fff" stopOpacity="0.18" />
          <stop offset="0.55" stopColor="#fff" stopOpacity="0" />
          <stop offset="1" stopColor="#000" stopOpacity="0.16" />
        </linearGradient>
      </defs>
    </svg>
  );
}

/** How a king is marked at the end of the game: mated, or the winner's. */
export type EndMark = "checkmate" | "winner";

const END_MARKS: Record<EndMark, { label: string; color: string }> = {
  checkmate: { label: "Checkmate", color: "var(--c-blunder)" },
  winner: { label: "Winner", color: "var(--c-best)" },
};

/** Round end-of-game badge for a king, drawn as the classification icons are: # for mate, a crown for the winner. */
export function EndMarkIcon({ value, size = 18 }: { value: EndMark; size?: number }) {
  const meta = END_MARKS[value];
  return (
    <svg className="cls-icon" width={size} height={size} viewBox="0 0 24 24" role="img" aria-label={meta.label}>
      <title>{meta.label}</title>
      <circle cx="12" cy="12" r="12" fill={meta.color} />
      <circle cx="12" cy="12" r="12" fill="url(#cls-shade)" />
      {value === "checkmate" ? (
        text("#", 14)
      ) : (
        <path fill="#fff" d="M5.6 8.4l3.5 2.9L12 6.2l2.9 5.1 3.5-2.9-1.2 7.8H6.8zM6.8 17.2h10.4v1.8H6.8z" />
      )}
    </svg>
  );
}
