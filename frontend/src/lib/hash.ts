// The URL hash: the review shown, and the move when one is being reviewed, so a reload returns to
// it.

export interface ReviewHash {
  id: string;
  /** The ply shown in the review; null on the report. */
  move: number | null;
}

export function parseReviewHash(hash: string): ReviewHash | null {
  const m = /^#review=([0-9a-f-]{36})(?:&move=(\d+))?$/i.exec(hash);
  if (!m) return null;
  return { id: m[1]!, move: m[2] === undefined ? null : Number(m[2]) };
}

export function reviewHash(id: string, move: number | null): string {
  return move === null ? `#review=${id}` : `#review=${id}&move=${move}`;
}
