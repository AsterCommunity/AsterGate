import { afterEach, describe, expect, expectTypeOf, it, vi } from "vitest";
import type { OperationData, OperationJsonResponse } from "@/types/api";
import { accountRequest, accountService } from "./accountService";
import type { systemService } from "./systemService";

vi.mock("./csrf", () => ({
	csrfHeaders: async () => ({ "X-AsterGate-CSRF": "test-token" }),
}));

describe("product response consumption", () => {
	afterEach(() => {
		vi.unstubAllGlobals();
	});
	it.each([
		null,
		[],
		{},
		{ code: "success", data: true },
		{ code: "success", msg: "" },
		{ code: "success", msg: "", data: true, error: null },
		{ code: "rate_limited", msg: "wait", data: true },
	])("rejects invalid successful REST envelopes %j", async (payload) => {
		vi.stubGlobal(
			"fetch",
			vi.fn(async () => new Response(JSON.stringify(payload))),
		);
		await expect(accountService.me()).rejects.toThrow(
			"Invalid API response envelope",
		);
	});
	it("preserves explicit null and empty-object data while requiring the field", async () => {
		for (const data of [null, {}]) {
			vi.stubGlobal(
				"fetch",
				vi.fn(
					async () =>
						new Response(JSON.stringify({ code: "success", msg: "", data })),
				),
			);
			await expect(
				accountRequest<{ data?: null | object }>("/account"),
			).resolves.toEqual({ code: "success", msg: "", data });
		}
	});
	it("preserves boolean acknowledgements and attaches same-origin write headers", async () => {
		const fetch = vi.fn(
			async (_input: RequestInfo | URL, _init?: RequestInit) =>
				new Response(JSON.stringify({ code: "success", msg: "", data: true })),
		);
		vi.stubGlobal("fetch", fetch);
		await expect(accountService.logout()).resolves.toMatchObject({
			data: true,
		});
		const init = fetch.mock.calls[0]?.[1];
		expect(init).toMatchObject({
			method: "POST",
			credentials: "same-origin",
			cache: "no-store",
		});
		expect(new Headers(init?.headers).get("X-AsterGate-CSRF")).toBe(
			"test-token",
		);
		expect(new Headers(init?.headers).get("X-AsterGate-Request")).toBe("1");
	});
	it("keeps operation DTOs distinct and requires runtime-validated success data", () => {
		expectTypeOf<
			Awaited<ReturnType<typeof accountService.me>>["data"]
		>().toEqualTypeOf<OperationData<"profile">>();
		expectTypeOf<
			Awaited<ReturnType<typeof accountService.sessions>>["data"]
		>().toEqualTypeOf<OperationData<"sessions">>();
		expectTypeOf<
			Awaited<ReturnType<typeof systemService.settings>>["data"]
		>().toEqualTypeOf<OperationData<"settings">>();
		expectTypeOf<OperationJsonResponse<"login", 401>>().not.toEqualTypeOf<
			OperationJsonResponse<"login", 200>
		>();
	});
	it("preserves null in generated operation data helpers", () => {
		expectTypeOf<OperationData<"login", 401>>().toEqualTypeOf<null>();
		expectTypeOf<
			Exclude<OperationData<"manage_list">["next_cursor"], undefined>
		>().toEqualTypeOf<string | null>();
	});
});
