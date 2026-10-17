import type { Classification } from "../api/generated/Classification";
import { CLASSIFICATIONS } from "../lib/format";

export function ClassificationBadge({ value, size = "s" }: { value: Classification | null; size?: "s" | "m" }) {
  if (!value) return null;
  const meta = CLASSIFICATIONS[value];
  return (
    <span
      className={`badge badge--${size}`}
      style={{ background: meta.color }}
      title={meta.label}
      aria-label={meta.label}
      role="img"
    >
      {meta.symbol}
    </span>
  );
}
