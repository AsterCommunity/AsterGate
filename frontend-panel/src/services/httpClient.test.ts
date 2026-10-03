import { afterEach, describe, expect, it, vi } from "vitest";
import { requestJson } from "./httpClient";

describe("requestJson", () => {
	afterEach(() => {
		vi.unstubAllGlobals();
	});

	it("returns parsed JSON for successful responses", async () => {
		vi.stubGlobal(
			"fetch",
			vi.fn(async () => new Response(JSON.stringify({ status: "ok" }))),
		);

		await expect(requestJson<{ status: string }>("/health")).resolves.toEqual({
			status: "ok",
		});
	});

	it("throws HttpError with parsed payload for failed responses", async () => {
		vi.stubGlobal(
			"fetch",
			vi.fn(
				async () =>
					new Response(JSON.stringify({ message: "not ready" }), {
						status: 503,
					}),
			),
		);

		await expect(requestJson("/health/ready")).rejects.toMatchObject({
			status: 503,
			payload: { message: "not ready" },
		});
	});
	it("keeps HTTP status for HTML proxy failures", async () => {
		vi.stubGlobal(
			"fetch",
			vi.fn(
				async () => new Response("<html>Bad Gateway</html>", { status: 502 }),
			),
		);
		await expect(requestJson("/api/v1/account")).rejects.toMatchObject({
			status: 502,
			code: null,
			payload: null,
		});
	});
	it("exposes typed codes and retry metadata without classifying wrong passwords as expired sessions", async () => {
		vi.stubGlobal(
			"fetch",
			vi.fn(
				async () =>
					new Response(
						JSON.stringify({
							code: "auth.credentials_failed",
							msg: "Invalid credentials",
							error: { retryable: false },
						}),
						{ status: 401 },
					),
			),
		);
		await expect(requestJson("/api/v1/auth/login")).rejects.toMatchObject({
			code: "auth.credentials_failed",
			retryable: false,
		});
	});
});
