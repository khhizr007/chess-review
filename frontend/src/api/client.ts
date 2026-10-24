// Typed client for the review API (see docs/api.md). Types are generated
// from the Rust backend by ts-rs into ./generated.
import type { ApiErrorEnvelope } from "./generated/ApiErrorEnvelope";
import type { CreateReviewRequest } from "./generated/CreateReviewRequest";
import type { Defaults } from "./generated/Defaults";
import type { Health } from "./generated/Health";
import type { JudgeMoveRequest } from "./generated/JudgeMoveRequest";
import type { ParseResponse } from "./generated/ParseResponse";
import type { PositionAnalysisResponse } from "./generated/PositionAnalysisResponse";
import type { ReviewResult } from "./generated/ReviewResult";
import type { ReviewStatus } from "./generated/ReviewStatus";
import type { SidelineJudgement } from "./generated/SidelineJudgement";

export class ApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly details: unknown;

  constructor(status: number, code: string, message: string, details?: unknown) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
    this.details = details;
  }
}

async function request<T>(method: string, path: string, body?: unknown, signal?: AbortSignal): Promise<T> {
  let response: Response;
  try {
    response = await fetch(path, {
      method,
      headers: body === undefined ? undefined : { "Content-Type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
      signal,
    });
  } catch (cause) {
    if (cause instanceof DOMException && cause.name === "AbortError") throw cause;
    throw new ApiError(0, "network_error", "Cannot reach the review server. Is the backend running?");
  }
  const text = await response.text();
  const data: unknown = text ? safeJson(text) : null;
  if (!response.ok) {
    const envelope = data as Partial<ApiErrorEnvelope> | null;
    const err = envelope?.error;
    throw new ApiError(
      response.status,
      err?.code ?? "http_error",
      err?.message ?? `Request failed with status ${response.status}`,
      err?.details,
    );
  }
  return data as T;
}

function safeJson(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

export const api = {
  health: (signal?: AbortSignal) => request<Health>("GET", "/api/health", undefined, signal),
  defaults: () => request<Defaults>("GET", "/api/config/defaults"),
  parseGames: (pgn: string) => request<ParseResponse>("POST", "/api/games/parse", { pgn }),
  createReview: (req: CreateReviewRequest) => request<ReviewStatus>("POST", "/api/reviews", req),
  reviewStatus: (id: string, signal?: AbortSignal) =>
    request<ReviewStatus>("GET", `/api/reviews/${encodeURIComponent(id)}`, undefined, signal),
  reviewResult: (id: string, signal?: AbortSignal) =>
    request<ReviewResult>("GET", `/api/reviews/${encodeURIComponent(id)}/result`, undefined, signal),
  cancelReview: (id: string) => request<ReviewStatus>("DELETE", `/api/reviews/${encodeURIComponent(id)}`),
  analyzePosition: (fen: string, multipv = 3) =>
    request<PositionAnalysisResponse>("POST", "/api/positions/analyze", { fen, multipv }),
  /** Classifies a sideline move with the review's rules, from the browser engine's lines. */
  judgeMove: (id: string, req: JudgeMoveRequest, signal?: AbortSignal) =>
    request<SidelineJudgement>("POST", `/api/reviews/${encodeURIComponent(id)}/judge`, req, signal),
};
