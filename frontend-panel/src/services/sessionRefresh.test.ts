// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { csrfHeaders, readCsrfToken } from "./csrf";
import { HttpError } from "./httpClient";
import {
	requestWithSession,
	sessionChanged,
	transitionSession,
} from "./sessionRefresh";

const csrf = "a".repeat(43);
const success = () =>
	new Response(JSON.stringify({ code: "success", msg: "", data: true }));
const failure = (code = "auth.access_expired", status = 401) =>
	new Response(JSON.stringify({ code, msg: "rejected" }), { status });
const init = async () => ({ credentials: "same-origin" as const });

function setCsrf(value?: string) {
	// biome-ignore lint/suspicious/noDocumentCookie: Exercise synchronous shared Cookie changes in jsdom.
	document.cookie = value
		? `ag_csrf=${value}; Path=/`
		: "ag_csrf=; Max-Age=0; Path=/";
}

describe("bounded browser refresh", () => {
	beforeEach(() => {
		setCsrf(csrf);
		sessionChanged();
		vi.stubGlobal("navigator", {
			locks: {
				request: vi.fn(async (_name, options, action) => (action ?? options)()),
			},
		});
	});
	afterEach(() => {
		vi.restoreAllMocks();
		vi.useRealTimers();
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
		expect(navigator.locks.request).toHaveBeenCalledTimes(2);
	});

	it("holds the session lock while preparing and sending a successful retry", async () => {
		let locked = false;
		vi.stubGlobal("navigator", {
			locks: {
				request: vi.fn(async (_name, action) => {
					expect(locked).toBe(false);
					locked = true;
					try {
						return await action();
					} finally {
						locked = false;
					}
				}),
			},
		});
		let requests = 0;
		const fetch = vi.fn(async () => {
			requests++;
			if (requests === 1) return failure();
			expect(locked).toBe(true);
			return success();
		});
		vi.stubGlobal("fetch", fetch);
		let preparations = 0;
		await requestWithSession("/account", async () => {
			preparations++;
			await Promise.resolve();
			if (preparations === 2) expect(locked).toBe(true);
			return init();
		});
		expect(preparations).toBe(2);
		expect(fetch).toHaveBeenCalledTimes(3);
		expect(locked).toBe(false);
	});

	it("releases a mutation's shared lock before recovering under the exclusive lock", async () => {
		let mode: string | undefined;
		vi.stubGlobal("navigator", {
			locks: {
				request: vi.fn(async (_name, options, action) => {
					expect(mode).toBeUndefined();
					mode = action ? options.mode : "exclusive";
					try {
						return await (action ?? options)();
					} finally {
						mode = undefined;
					}
				}),
			},
		});
		let requests = 0;
		let rotated = false;
		const fetch = vi.fn(async (url: string) => {
			requests++;
			expect(mode).toBe(requests === 1 ? "shared" : "exclusive");
			if (url === "/api/v1/auth/refresh") {
				rotated = true;
				return success();
			}
			return rotated ? success() : failure();
		});
		vi.stubGlobal("fetch", fetch);
		await requestWithSession("/account/password", async () => ({
			method: "POST",
		}));
		expect(fetch).toHaveBeenCalledTimes(4);
		expect(mode).toBeUndefined();
	});

	it.each([
		"recovery lock",
		"profile response",
		"refresh response",
		"retry lock",
		"retry preparation",
	])("rejects a cookie-only identity change during %s without waiting for a notification", async (stage) => {
		const original = new HttpError(401, {
			code: "auth.access_expired",
			msg: "original request rejected",
		});
		const changeIdentity = () => {
			setCsrf("b".repeat(43));
		};
		let locks = 0;
		vi.stubGlobal("navigator", {
			locks: {
				request: vi.fn(async (_name, options, callback) => {
					if (callback) return callback();
					locks++;
					if (
						(stage === "recovery lock" && locks === 1) ||
						(stage === "retry lock" && locks === 2)
					)
						changeIdentity();
					return options();
				}),
			},
		});
		let requests = 0;
		const fetch = vi.fn(async () => {
			requests++;
			if (requests === 1) throw original;
			if (requests === 2) {
				if (stage === "profile response") changeIdentity();
				return stage === "refresh response" ? failure() : success();
			}
			changeIdentity();
			return success();
		});
		vi.stubGlobal("fetch", fetch);
		let preparations = 0;
		await expect(
			requestWithSession("/account", async () => {
				preparations++;
				await Promise.resolve();
				if (stage === "retry preparation" && preparations === 2)
					changeIdentity();
				return {
					method: "PATCH",
					headers: await csrfHeaders("PATCH", "/api/v1/account"),
				};
			}),
		).rejects.toBe(original);
		expect(readCsrfToken()).not.toBe(csrf);
		expect(fetch).toHaveBeenCalledTimes(
			stage === "recovery lock" ? 1 : stage === "refresh response" ? 3 : 2,
		);
	});

	it.each([
		undefined,
		"b".repeat(43),
	])("preserves the original failure if shared cookies change before it arrives: %s", async (next) => {
		const original = new HttpError(401, {
			code: "auth.access_expired",
			msg: "original request rejected",
		});
		const fetch = vi.fn(async () => {
			setCsrf(next);
			throw original;
		});
		vi.stubGlobal("fetch", fetch);
		await expect(requestWithSession("/account", init)).rejects.toBe(original);
		expect(fetch).toHaveBeenCalledTimes(1);
		expect(navigator.locks.request).not.toHaveBeenCalled();
	});

	it("does not send a mutation prepared asynchronously under a different identity", async () => {
		const fetch = vi.fn();
		vi.stubGlobal("fetch", fetch);
		await expect(
			requestWithSession("/account", async () => {
				await Promise.resolve();
				setCsrf("b".repeat(43));
				return {
					method: "PATCH",
					headers: await csrfHeaders("PATCH", "/api/v1/account"),
				};
			}),
		).rejects.toMatchObject({ status: 401, code: "auth.session_required" });
		expect(fetch).not.toHaveBeenCalled();
	});

	it("checks identity after a first mutation waits for its shared lock", async () => {
		vi.stubGlobal("navigator", {
			locks: {
				request: vi.fn(async (_name, _options, action) => {
					setCsrf("b".repeat(43));
					return action();
				}),
			},
		});
		const fetch = vi.fn();
		vi.stubGlobal("fetch", fetch);
		await expect(
			requestWithSession("/account", async () => ({ method: "PATCH" })),
		).rejects.toMatchObject({ status: 401, code: "auth.session_required" });
		expect(fetch).not.toHaveBeenCalled();
	});

	it("does not adopt a new identity marker bootstrapped while preparing a mutation", async () => {
		setCsrf();
		const fetch = vi.fn();
		vi.stubGlobal("fetch", fetch);
		await expect(
			requestWithSession("/account", async () => {
				setCsrf("b".repeat(43));
				return { method: "PATCH" };
			}),
		).rejects.toMatchObject({ status: 401, code: "auth.session_required" });
		expect(fetch).not.toHaveBeenCalled();
	});

	it("does not recover when the request began without a valid identity marker", async () => {
		setCsrf();
		const fetch = vi.fn(async () => failure());
		vi.stubGlobal("fetch", fetch);
		await expect(requestWithSession("/account", init)).rejects.toMatchObject({
			status: 401,
		});
		expect(fetch).toHaveBeenCalledTimes(1);
		expect(navigator.locks.request).not.toHaveBeenCalled();
	});

	it("times out only the profile probe and releases a queued identity transition", async () => {
		vi.useFakeTimers();
		const timeout = vi
			.spyOn(AbortSignal, "timeout")
			.mockImplementation((milliseconds) => {
				const controller = new AbortController();
				setTimeout(
					() => controller.abort(new DOMException("Timed out", "TimeoutError")),
					milliseconds,
				);
				return controller.signal;
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
		let probeStarted!: () => void;
		const started = new Promise<void>((resolve) => {
			probeStarted = resolve;
		});
		const fetch = vi
			.fn()
			.mockResolvedValueOnce(failure())
			.mockImplementationOnce((_url: string, init: RequestInit) => {
				probeStarted();
				return new Promise((_resolve, reject) => {
					init.signal?.addEventListener("abort", () =>
						reject(init.signal?.reason),
					);
				});
			});
		vi.stubGlobal("fetch", fetch);
		const request = requestWithSession("/account", init).catch(
			(error: unknown) => error,
		);
		await started;
		const action = vi.fn(async () => {});
		const transition = transitionSession(action);
		expect(action).not.toHaveBeenCalled();
		await vi.advanceTimersByTimeAsync(9_999);
		expect(action).not.toHaveBeenCalled();
		await vi.advanceTimersByTimeAsync(1);
		await transition;
		expect(await request).toMatchObject({ name: "TimeoutError" });
		expect(action).toHaveBeenCalledTimes(1);
		expect(timeout).toHaveBeenCalledExactlyOnceWith(10_000);
		expect(fetch).toHaveBeenCalledTimes(2);
	});

	it("does not impose the probe timeout on refresh or replace caller signals on retry", async () => {
		const caller = new AbortController().signal;
		let rotated = false;
		const fetch = vi.fn(async (url: string, init: RequestInit) => {
			if (url === "/api/v1/auth/refresh") {
				expect(init.signal).toBeUndefined();
				rotated = true;
				return success();
			}
			if (url === "/api/v1/account") {
				expect(init.signal).toBeInstanceOf(AbortSignal);
				expect(init.signal).not.toBe(caller);
			} else {
				expect(init.signal).toBe(caller);
			}
			return rotated ? success() : failure();
		});
		vi.stubGlobal("fetch", fetch);
		await requestWithSession("/account/sessions", async () => ({
			signal: caller,
		}));
		expect(fetch).toHaveBeenCalledTimes(4);
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
