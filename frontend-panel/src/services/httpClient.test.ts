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

describe("shared REST errors", () => {
	afterEach(() => {
		vi.unstubAllGlobals();
		vi.useRealTimers();
	});
	it.each([
		"",
		"<html>ok</html>",
	])("rejects invalid successful JSON %j", async (body) => {
		vi.stubGlobal(
			"fetch",
			vi.fn(async () => new Response(body)),
		);
		await expect(requestJson("/api/v1/account")).rejects.toThrow(
			"Invalid JSON response",
		);
	});
	it.each([
		[
			{ code: "auth.session_required", msg: "Sign in" },
			401,
			"auth.session_required",
			true,
			false,
		],
		[
			{ code: "auth.credentials_failed", msg: "Wrong password" },
			401,
			"auth.credentials_failed",
			false,
			false,
		],
		[
			{ code: "future.code", msg: "Future", error: { retryable: true } },
			503,
			null,
			false,
			true,
		],
		[
			{ code: "success", msg: "", error: { retryable: true } },
			500,
			null,
			false,
			false,
		],
		[
			{
				code: "rate_limited",
				msg: "Wait",
				data: null,
				error: { retryable: true },
			},
			429,
			null,
			false,
			false,
		],
	])("validates failure metadata without automatic retry", async (payload, status, code, requiresSignIn, retryable) => {
		const fetch = vi.fn(
			async () =>
				new Response(JSON.stringify(payload), { status: Number(status) }),
		);
		vi.stubGlobal("fetch", fetch);
		await expect(requestJson("/api/v1/account")).rejects.toMatchObject({
			status,
			code,
			requiresSignIn,
			retryable,
			payload,
		});
		expect(fetch).toHaveBeenCalledTimes(1);
	});
	it.each([
		["30", 30],
		["0", 0],
		["-1", null],
		["1.5", null],
		["invalid", null],
		["9007199254740992", null],
		["Thu, 01 Jan 2026 00:00:30 GMT", 30],
		["Wed, 31 Dec 2025 23:59:30 GMT", 0],
	])("reads advisory Retry-After %s", async (retryAfter, seconds) => {
		vi.useFakeTimers();
		vi.setSystemTime(new Date("2026-01-01T00:00:00Z"));
		vi.stubGlobal(
			"fetch",
			vi.fn(
				async () =>
					new Response(
						JSON.stringify({
							code: "rate_limited",
							msg: "Wait",
							error: { retryable: true },
						}),
						{ status: 429, headers: { "Retry-After": String(retryAfter) } },
					),
			),
		);
		await expect(requestJson("/api/v1/account")).rejects.toMatchObject({
			status: 429,
			retryable: true,
			retryAfterSeconds: seconds,
		});
	});
	it("merges all HeadersInit forms and preserves caller Accept", async () => {
		const fetch = vi.fn(
			async (_input: RequestInfo | URL, _init?: RequestInit) =>
				new Response("{}"),
		);
		vi.stubGlobal("fetch", fetch);
		for (const headers of [
			new Headers({ "X-Custom": "yes", Accept: "custom/json" }),
			[
				["X-Custom", "yes"],
				["Accept", "custom/json"],
			] as [string, string][],
			{ "X-Custom": "yes", Accept: "custom/json" },
		]) {
			await requestJson("/health", { headers });
			const init = fetch.mock.calls.at(-1)?.[1] as RequestInit;
			expect(new Headers(init.headers).get("X-Custom")).toBe("yes");
			expect(new Headers(init.headers).get("Accept")).toBe("custom/json");
		}
	});
});
