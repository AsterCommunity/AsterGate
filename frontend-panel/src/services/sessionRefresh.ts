import { ApiErrorCode, isApiSuccessEnvelope } from "@/types/api-helpers";
import { csrfHeaders, readCsrfToken } from "./csrf";
import { HttpError, requestJson } from "./httpClient";

let flight: Promise<void> | undefined;
let generation = 0;
const SESSION_LOCK = "astergate-session-refresh";
const channel =
	typeof BroadcastChannel === "undefined"
		? undefined
		: new BroadcastChannel("astergate-session-state");
channel?.addEventListener("message", () => {
	generation++;
});

/** Only lifecycle notifications cross tabs. Credentials never enter JS storage or messages. */
export function sessionChanged() {
	generation++;
	channel?.postMessage("changed");
}

/** Serialize cookie identity changes with refresh responses, including across browser tabs. */
export async function transitionSession<T>(
	action: () => Promise<T>,
): Promise<T> {
	sessionChanged();
	const transition = async () => {
		try {
			return await action();
		} finally {
			sessionChanged();
		}
	};
	return navigator.locks
		? navigator.locks.request(SESSION_LOCK, transition)
		: transition();
}

/** Only missing/expired access permits recovery; permission, password and transport failures do not. */
function canRefresh(error: unknown): error is HttpError {
	return (
		error instanceof HttpError &&
		error.status === 401 &&
		(error.code === ApiErrorCode.SessionRequired ||
			error.code === ApiErrorCode.AccessExpired)
	);
}

/** The shared CSRF cookie fences identity changes even before BroadcastChannel delivers them. */
function assertSession(
	expectedGeneration: number,
	expectedCsrf: string | undefined,
	original: HttpError,
) {
	if (
		!expectedCsrf ||
		generation !== expectedGeneration ||
		readCsrfToken() !== expectedCsrf
	)
		throw original;
}

/**
 * Under the cross-tab lock, reuse an already refreshed access Cookie or rotate once.
 * Identity changes abort recovery; browsers without Web Locks require sign-in under Strict policy.
 */
async function refreshSession(
	expectedGeneration: number,
	expectedCsrf: string | undefined,
	original: HttpError,
) {
	assertSession(expectedGeneration, expectedCsrf, original);
	// Strict reuse policy needs an actual cross-tab mutex. A localStorage lease is not atomic.
	// Unsupported browsers safely require sign-in instead of racing shared refresh credentials.
	if (!navigator.locks) throw original;
	await navigator.locks.request(SESSION_LOCK, async () => {
		assertSession(expectedGeneration, expectedCsrf, original);
		// A preceding tab may already have rotated the shared cookies while we waited.
		let needsRefresh = false;
		try {
			const profile = await requestJson("/api/v1/account", {
				credentials: "same-origin",
				cache: "no-store",
			});
			if (!isApiSuccessEnvelope(profile))
				throw new Error("Invalid API response envelope");
		} catch (error) {
			if (!canRefresh(error)) throw error;
			needsRefresh = true;
		}
		assertSession(expectedGeneration, expectedCsrf, original);
		if (!needsRefresh) return;
		const headers = await csrfHeaders("POST", "/api/v1/auth/refresh");
		assertSession(expectedGeneration, expectedCsrf, original);
		const result = await requestJson("/api/v1/auth/refresh", {
			method: "POST",
			credentials: "same-origin",
			cache: "no-store",
			headers: {
				"X-AsterGate-Request": "1",
				...headers,
			},
		});
		if (!isApiSuccessEnvelope(result))
			throw new Error("Invalid API response envelope");
		assertSession(expectedGeneration, expectedCsrf, original);
	});
}

/** Replay only a request rejected by the authentication extractor, at most once. */
export async function requestWithSession<T>(
	path: string,
	init: () => Promise<RequestInit>,
): Promise<T> {
	const expectedGeneration = generation;
	const expectedCsrf = readCsrfToken();
	try {
		return await requestJson<T>(`/api/v1${path}`, await init());
	} catch (error) {
		const protectedPath =
			path === "/account" ||
			path.startsWith("/account/") ||
			path.startsWith("/management/");
		if (
			!protectedPath ||
			!canRefresh(error) ||
			generation !== expectedGeneration
		)
			throw error;
		assertSession(expectedGeneration, expectedCsrf, error);
		flight ??= refreshSession(expectedGeneration, expectedCsrf, error).finally(
			() => {
				flight = undefined;
			},
		);
		await flight;
		assertSession(expectedGeneration, expectedCsrf, error);
		return navigator.locks.request(SESSION_LOCK, async () => {
			assertSession(expectedGeneration, expectedCsrf, error);
			const retryInit = await init();
			assertSession(expectedGeneration, expectedCsrf, error);
			// Keep cookie identity changes serialized through retry preparation and its response.
			return requestJson<T>(`/api/v1${path}`, retryInit);
		});
	}
}
