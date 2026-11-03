import { ApiErrorCode, isApiSuccessEnvelope } from "@/types/api-helpers";
import { csrfHeaders } from "./csrf";
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

function canRefresh(error: unknown): error is HttpError {
	return (
		error instanceof HttpError &&
		error.status === 401 &&
		(error.code === ApiErrorCode.SessionRequired ||
			error.code === ApiErrorCode.AccessExpired)
	);
}

async function refreshSession(expectedGeneration: number, original: HttpError) {
	if (generation !== expectedGeneration) throw original;
	// Strict reuse policy needs an actual cross-tab mutex. A localStorage lease is not atomic.
	// Unsupported browsers safely require sign-in instead of racing shared refresh credentials.
	if (!navigator.locks) throw original;
	await navigator.locks.request(SESSION_LOCK, async () => {
		if (generation !== expectedGeneration) throw original;
		// A preceding tab may already have rotated the shared cookies while we waited.
		try {
			const profile = await requestJson("/api/v1/account", {
				credentials: "same-origin",
				cache: "no-store",
			});
			if (!isApiSuccessEnvelope(profile))
				throw new Error("Invalid API response envelope");
			return;
		} catch (error) {
			if (!canRefresh(error)) throw error;
		}
		if (generation !== expectedGeneration) throw original;
		const result = await requestJson("/api/v1/auth/refresh", {
			method: "POST",
			credentials: "same-origin",
			cache: "no-store",
			headers: {
				"X-AsterGate-Request": "1",
				...(await csrfHeaders("POST", "/api/v1/auth/refresh")),
			},
		});
		if (!isApiSuccessEnvelope(result))
			throw new Error("Invalid API response envelope");
	});
}

/** Replay only a request rejected by the authentication extractor, at most once. */
export async function requestWithSession<T>(
	path: string,
	init: () => Promise<RequestInit>,
): Promise<T> {
	const expectedGeneration = generation;
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
		flight ??= refreshSession(expectedGeneration, error).finally(() => {
			flight = undefined;
		});
		await flight;
		if (generation !== expectedGeneration) throw error;
		return requestJson<T>(`/api/v1${path}`, await init());
	}
}
