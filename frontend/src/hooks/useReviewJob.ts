import { useCallback, useEffect, useRef, useState } from "react";
import { ApiError, api } from "../api/client";
import type { CreateReviewRequest } from "../api/generated/CreateReviewRequest";
import type { ReviewResult } from "../api/generated/ReviewResult";
import type { ReviewStatus } from "../api/generated/ReviewStatus";

export type JobView =
  | { kind: "idle" }
  | { kind: "submitting" }
  | { kind: "running"; status: ReviewStatus }
  | { kind: "loading"; id: string }
  | { kind: "done"; id: string; result: ReviewResult }
  | { kind: "cancelled" }
  | { kind: "error"; message: string };

const POLL_MS = 500;

function message(err: unknown): string {
  if (err instanceof ApiError) return err.message;
  if (err instanceof Error) return err.message;
  return String(err);
}

/** Drives one review job: submit, poll progress, fetch the result, cancel. */
export function useReviewJob() {
  const [view, setView] = useState<JobView>({ kind: "idle" });
  const abort = useRef<AbortController | null>(null);
  const currentId = useRef<string | null>(null);

  const stopPolling = () => {
    abort.current?.abort();
    abort.current = null;
  };

  useEffect(() => stopPolling, []);

  const follow = useCallback(async (id: string) => {
    stopPolling();
    const controller = new AbortController();
    abort.current = controller;
    currentId.current = id;
    try {
      for (;;) {
        const status = await api.reviewStatus(id, controller.signal);
        if (status.state === "completed") {
          setView({ kind: "loading", id });
          const result = await api.reviewResult(id, controller.signal);
          setView({ kind: "done", id, result });
          return;
        }
        if (status.state === "failed") {
          setView({ kind: "error", message: status.error ?? "The review failed." });
          return;
        }
        if (status.state === "cancelled") {
          setView({ kind: "cancelled" });
          return;
        }
        setView({ kind: "running", status });
        await new Promise((resolve) => setTimeout(resolve, POLL_MS));
        if (controller.signal.aborted) return;
      }
    } catch (err) {
      if (controller.signal.aborted) return;
      if (err instanceof ApiError && err.status === 404) {
        setView({ kind: "error", message: "This review is no longer available. Please run it again." });
      } else {
        setView({ kind: "error", message: message(err) });
      }
    }
  }, []);

  const start = useCallback(
    async (req: CreateReviewRequest) => {
      setView({ kind: "submitting" });
      try {
        const status = await api.createReview(req);
        setView({ kind: "running", status });
        await follow(status.id);
      } catch (err) {
        setView({ kind: "error", message: message(err) });
      }
    },
    [follow],
  );

  const cancel = useCallback(async () => {
    const id = currentId.current;
    if (!id) return;
    try {
      await api.cancelReview(id);
    } catch (err) {
      setView({ kind: "error", message: message(err) });
    }
  }, []);

  const reset = useCallback(() => {
    stopPolling();
    currentId.current = null;
    setView({ kind: "idle" });
  }, []);

  return { view, start, follow, cancel, reset };
}
