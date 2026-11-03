import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
	requestWithSession,
	sessionChanged,
	transitionSession,
} from "./sessionRefresh";

vi.mock("./csrf", () => ({
	csrfHeaders: async () => ({ "X-AsterGate-CSRF": "csrf" }),
}));

const success = () =>
	new Response(JSON.stringify({ code: "success", msg: "", data: true }));
const failure = (code = "auth.access_expired", status = 401) =>
	new Response(JSON.stringify({ code, msg: "rejected" }), { status });
const init = async () => ({ credentials: "same-origin" as const });

describe("bounded browser refresh", () => {
	beforeEach(() => {
		sessionChanged();
		vi.stubGlobal("navigator", {
			locks: { request: vi.fn(async (_name, action) => action()) },
		});
	});
	afterEach(() => {
		vi.unstubAllGlobals();
	});

	it("shares one refresh across simultaneous failures and replays each request once", async () => {
		let rotated = false;
		const fetch = vi.fn(async (url: string) => {
			if (url === "/api/v1/auth/refresh") {
				rotated = true;
				return success();
			}
			return rotated ? success() : failure();
		});
		vi.stubGlobal("fetch", fetch);
		await Promise.all([
			requestWithSession("/account", init),
			requestWithSession("/management/settings", init),
		]);
		expect(
			fetch.mock.calls.filter(([url]) => url === "/api/v1/auth/refresh"),
		).toHaveLength(1);
		expect(
			fetch.mock.calls.filter(([url]) => url === "/api/v1/management/settings"),
		).toHaveLength(2);
	});

	it("uses cookies already rotated by another tab without refreshing again", async () => {
		let count = 0;
		const fetch = vi.fn(async () => (++count === 1 ? failure() : success()));
		vi.stubGlobal("fetch", fetch);
		await requestWithSession("/account", init);
		expect(fetch.mock.calls).toHaveLength(3);
		expect(navigator.locks.request).toHaveBeenCalledTimes(1);
	});

	it.each([
		["/auth/login", "auth.credentials_failed", 401],
		["/account/password", "auth.credentials_failed", 401],
		["/management/settings", "auth.forbidden", 403],
		["/account", "service.unavailable", 503],
		["/account", "rate_limited", 429],
		["/auth/refresh", "auth.session_required", 401],
		["/public/frontend-config", "auth.session_required", 401],
	])("does not refresh %s on %s", async (path, code, status) => {
		const fetch = vi.fn(async () => failure(code, status));
		vi.stubGlobal("fetch", fetch);
		await expect(requestWithSession(path, init)).rejects.toMatchObject({
			status,
		});
		expect(fetch).toHaveBeenCalledTimes(1);
	});

	it("stops on refresh rejection and does not clear cookies or replay the mutation", async () => {
		const fetch = vi.fn(async (url: string) =>
			url === "/api/v1/auth/refresh"
				? failure("auth.refresh_reused")
				: failure(),
		);
		vi.stubGlobal("fetch", fetch);
		await expect(
			requestWithSession("/account/password", init),
		).rejects.toMatchObject({ code: "auth.refresh_reused" });
		expect(fetch).toHaveBeenCalledTimes(3);
	});

	it("stops after a second authentication rejection", async () => {
		const fetch = vi.fn(async (url: string) =>
			url === "/api/v1/auth/refresh" ? success() : failure(),
		);
		vi.stubGlobal("fetch", fetch);
		await expect(requestWithSession("/account", init)).rejects.toMatchObject({
			status: 401,
		});
		expect(fetch).toHaveBeenCalledTimes(4);
	});

	it("does not replay an old mutation across login/logout", async () => {
		const fetch = vi.fn(async (url: string) => {
			if (url === "/api/v1/auth/refresh") {
				sessionChanged();
				return success();
			}
			return failure();
		});
		vi.stubGlobal("fetch", fetch);
		await expect(
			requestWithSession("/account/password", init),
		).rejects.toMatchObject({ status: 401 });
		expect(fetch).toHaveBeenCalledTimes(3);
	});

	it("requires sign-in when a browser cannot coordinate shared cookies", async () => {
		vi.stubGlobal("navigator", {});
		const fetch = vi.fn(async () => failure());
		vi.stubGlobal("fetch", fetch);
		await expect(requestWithSession("/account", init)).rejects.toMatchObject({
			status: 401,
		});
		expect(fetch).toHaveBeenCalledTimes(1);
	});

	it("serializes login behind an in-flight refresh response and fences its old mutation", async () => {
		let releaseRefresh!: () => void;
		let refreshStarted!: () => void;
		const started = new Promise<void>((resolve) => {
			refreshStarted = resolve;
		});
		const response = new Promise<void>((resolve) => {
			releaseRefresh = resolve;
		});
		let tail = Promise.resolve<unknown>(undefined);
		vi.stubGlobal("navigator", {
			locks: {
				request: vi.fn((_name, action) => {
					const result = tail.then(action);
					tail = result.catch(() => undefined);
					return result;
				}),
			},
		});
		const events: string[] = [];
		vi.stubGlobal(
			"fetch",
			vi.fn(async (url: string) => {
				if (url === "/api/v1/auth/refresh") {
					refreshStarted();
					await response;
					events.push("refresh-cookie");
					return success();
				}
				return failure();
			}),
		);
		const mutation = requestWithSession("/account", init).catch(
			(error) => error,
		);
		await started;
		const login = transitionSession(async () => {
			events.push("login-cookie");
		});
		expect(events).toEqual([]);
		releaseRefresh();
		await login;
		expect(await mutation).toMatchObject({ status: 401 });
		expect(events).toEqual(["refresh-cookie", "login-cookie"]);
	});
});
