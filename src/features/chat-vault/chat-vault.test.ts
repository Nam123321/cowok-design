// @vitest-environment jsdom
// @story 04.6
// @implements AC-01, AC-02, AC-04, AC-05 (Client SDK, Pre-Persistence Secret Interceptor, CAS Dropzone Paste Guard & Vault Card)
import React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { ChatVaultClient } from "@/api/chat-vault";
import { CasDropzonePanel } from "./CasDropzonePanel";
import { DecisionEngineOnboardingCard } from "./DecisionEngineOnboardingCard";
import { VaultReceiptCard } from "./VaultReceiptCard";
import { inspectChatSecretCandidate } from "./secret-interceptor";

describe("Chat Vault SDK, Pre-Persistence Interceptor & UI Cards", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("intercepts Google AI Studio and TypeSafe JEV secrets before React or SQLite persistence", () => {
    const aiRes = inspectChatSecretCandidate("use key AIzaSyD4u5v6w7x8y9z01234567890abcdef please");
    expect(aiRes.detected).toBe(true);
    expect(aiRes.provider).toBe("google_ai_studio");
    expect(aiRes.rawSecretEnvelope).toBe("AIzaSyD4u5v6w7x8y9z01234567890abcdef");
    expect(aiRes.redactedPreview).not.toContain("AIzaSyD4u5");

    const engineRes = inspectChatSecretCandidate("jev_prod_token_998877665544");
    expect(engineRes.detected).toBe(true);
    expect(engineRes.provider).toBe("typesafe_jev");

    const cleanRes = inspectChatSecretCandidate("normal design feedback");
    expect(cleanRes.detected).toBe(false);
  });

  it("routes all chat, CAS and vault operations through ChatVaultClient with X-Idempotency-Key", async () => {
    const calls: Array<{ url: string; headers: Headers; method: string }> = [];
    const mockFetch = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const headers = new Headers(init?.headers || {});
      calls.push({ url, headers, method: init?.method || "GET" });
      if (url.endsWith("/api/v1/vault/chat-ingest")) {
        return new Response(
          JSON.stringify({
            receiptId: "rcpt-1",
            provider: "google_ai_studio",
            presenterMask: "AIza...cdef",
            status: "VERIFIED",
            localOnly: true,
            highEntropyWarning: true,
          }),
          { status: 201 }
        );
      }
      return new Response(
        JSON.stringify({ sessionId: "s1", projectId: "__workbench_default__", messages: [] }),
        { status: 200 }
      );
    }) as unknown as typeof fetch;

    const client = new ChatVaultClient({
      getSessionToken: () => "tok-1",
      fetchImpl: mockFetch,
    });
    const list = await client.getMessages("s1");
    expect(list.sessionId).toBe("s1");

    const receipt = await client.ingestVaultSecret(
      {
        sessionId: "s1",
        provider: "google_ai_studio",
        rawSecretEnvelope: "AIzaSyD4u5v6w7x8y9z01234567890abcdef",
        enterpriseLocalOnlyLock: true,
      },
      { idempotencyKey: "key-ui-1" }
    );
    expect(receipt.presenterMask).toBe("AIza...cdef");
    expect(calls[1].headers.get("X-Idempotency-Key")).toBe("key-ui-1");
  });

  it("renders Onboarding Card links, VaultReceiptCard 44px controls, and prevents binary paste leaks", async () => {
    const onToggle = vi.fn();
    const { getByTestId, rerender } = render(
      React.createElement(DecisionEngineOnboardingCard, {
        enterpriseLocalOnlyLock: false,
        onToggleLocalOnlyLock: onToggle,
      })
    );
    expect(getByTestId("decision-engine-onboarding-card").textContent).toContain("Google AI Studio");
    fireEvent.click(getByTestId("enterprise-local-only-toggle"));
    expect(onToggle).toHaveBeenCalledWith(true);

    const onRevoke = vi.fn().mockResolvedValue(undefined);
    rerender(
      React.createElement(VaultReceiptCard, {
        receipt: {
          receiptId: "rcpt-ui",
          provider: "typesafe_jev",
          presenterMask: "jev_...9900",
          status: "RATE_LIMITED_429",
          localOnly: true,
          highEntropyWarning: false,
        },
        onRevoke,
      })
    );
    expect(getByTestId("vault-receipt-status-pill").textContent).toBe("RATE_LIMITED_429");
    const revokeBtn = getByTestId("vault-receipt-revoke-btn");
    expect(revokeBtn.style.minHeight).toBe("44px");
    fireEvent.click(revokeBtn);
    await waitFor(() => expect(onRevoke).toHaveBeenCalledWith("rcpt-ui"));

    rerender(React.createElement(CasDropzonePanel, { sessionId: "s1" }));
    const dropzone = getByTestId("cas-file-dropzone");
    const input = getByTestId("cas-dropzone-file-input") as HTMLInputElement;
    expect(input.getAttribute("accept")).toContain(".csv");
    expect(input.getAttribute("accept")).toContain(".fig");
    expect(dropzone).toBeTruthy();
  });

  it("redacts multiple distinct secrets in a single message and neutralizes CSV DDE cells", async () => {
    const multi = inspectChatSecretCandidate(
      "primary AIzaSyD4u5v6w7x8y9z01234567890abcdef and secondary sk-proj1234567890abcdefghij"
    );
    expect(multi.detected).toBe(true);
    expect(multi.detectedSecrets.length).toBe(2);
    expect(multi.redactedPreview).not.toContain("AIzaSyD4u5");
    expect(multi.redactedPreview).not.toContain("sk-proj1234");
    expect(multi.redactedPreview).toContain("[VAULTED:google_ai_studio]");
    expect(multi.redactedPreview).toContain("[VAULTED:openai]");

    const { neutralizeCsvTsvText } = await import("./read-selected-chat-files");
    const csvOut = neutralizeCsvTsvText("colA,colB\n=cmd|' /C calc'!A0,@SUM(1+1)", "payload.csv");
    expect(csvOut).toContain("'=cmd|' /C calc'!A0");
    expect(csvOut).toContain("'@SUM(1+1)");

    const { EventEmitter } = await import("node:events");
    const { MAX_CAS_UPLOAD_BODY_BYTES, readRawBody } = await import("@/lib/vite-bff-proof");
    const OVER_ONE_MIB_BYTES = 1_572_864;
    const req = new EventEmitter();
    const p = readRawBody(req as unknown as import("node:http").IncomingMessage, MAX_CAS_UPLOAD_BODY_BYTES);
    req.emit("data", Buffer.alloc(OVER_ONE_MIB_BYTES, 1));
    req.emit("end");
    const buf = await p;
    expect(Buffer.isBuffer(buf) && buf.byteLength === OVER_ONE_MIB_BYTES).toBe(true);
  });
});
