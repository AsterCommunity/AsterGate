// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { csrfHeaders } from "./csrf";

const token = "a".repeat(43);
afterEach(() => {
	vi.restoreAllMocks();
	vi.unstubAllGlobals();
});

describe("CSRF request headers", () => {
	it("does not read cookies or bootstrap safe and foreign requests", async () => {
		const cookies = vi.spyOn(document, "cookie", "get");
		const fetchMock = vi.fn();
		vi.stubGlobal("fetch", fetchMock);
		expect(await csrfHeaders("GET", "/api/v1/account")).toEqual({});
		expect(
			await csrfHeaders("POST", "https://other.example/api/v1/auth/login"),
		).toEqual({});
		expect(await csrfHeaders("POST", "/elsewhere")).toEqual({});
		expect(cookies).not.toHaveBeenCalled();
		expect(fetchMock).not.toHaveBeenCalled();
	});

	it("uses the host cookie for same-origin mutations", async () => {
		vi.spyOn(document, "cookie", "get").mockReturnValue(`ag_csrf=${token}`);
		expect(await csrfHeaders("POST", "/api/v1/auth/login")).toEqual({
			"X-AsterGate-CSRF": token,
		});
	});

	it("coalesces bootstrap while the first request is still pending", async () => {
		let cookie = "";
		vi.spyOn(document, "cookie", "get").mockImplementation(() => cookie);
		let finish: (response: { ok: boolean }) => void = () => {};
		const fetchMock = vi.fn(
			() =>
				new Promise<{ ok: boolean }>((resolve) => {
					finish = resolve;
				}),
		);
		vi.stubGlobal("fetch", fetchMock);
		const first = csrfHeaders("POST");
		const second = csrfHeaders("DELETE");
		expect(fetchMock).toHaveBeenCalledTimes(1);
		cookie = `ag_csrf=${token}`;
		finish({ ok: true });
		expect(await Promise.all([first, second])).toEqual(
			Array(2).fill({ "X-AsterGate-CSRF": token }),
		);
		expect(fetchMock).toHaveBeenCalledWith(
			"/api/v1/setup/status",
			expect.objectContaining({
				credentials: "same-origin",
				cache: "no-store",
			}),
		);
	});

	it("selects the HTTPS host cookie and reads rotated values on every mutation", async () => {
		vi.stubGlobal("location", {
			protocol: "https:",
			href: "https://gate.example.test/",
			origin: "https://gate.example.test",
		});
		const cookie = vi
			.spyOn(document, "cookie", "get")
			.mockReturnValue(`ag_csrf=wrong; __Host-ag_csrf=${token}`);
		expect(await csrfHeaders("POST")).toEqual({ "X-AsterGate-CSRF": token });
		cookie.mockReturnValue(`__Host-ag_csrf=${"b".repeat(43)}`);
		expect(await csrfHeaders("POST")).toEqual({
			"X-AsterGate-CSRF": "b".repeat(43),
		});
	});

	it.each([
		"ag_csrf=short",
		`ag_csrf=${token}; ag_csrf=${token}`,
	])("fails closed for malformed or ambiguous cookies: %s", async (cookie) => {
		vi.spyOn(document, "cookie", "get").mockReturnValue(cookie);
		vi.stubGlobal(
			"fetch",
			vi.fn(async () => ({ ok: true })),
		);
		await expect(csrfHeaders("POST")).rejects.toThrow("CSRF token unavailable");
	});
});
