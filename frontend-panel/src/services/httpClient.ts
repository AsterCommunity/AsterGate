import {
	ApiErrorCode,
	isApiFailureEnvelope,
	readApiErrorCode,
} from "@/types/api-helpers";

type JsonRequestInit = Omit<RequestInit, "headers"> & {
	headers?: HeadersInit;
};

export class HttpError extends Error {
	readonly status: number;
	readonly payload: unknown;
	readonly code: ReturnType<typeof readApiErrorCode>;
	readonly retryable: boolean;
	readonly retryAfterSeconds: number | null;

	constructor(
		status: number,
		payload: unknown,
		retryAfter: string | null = null,
	) {
		super(`HTTP request failed with status ${status}`);
		this.name = "HttpError";
		this.status = status;
		this.payload = payload;
		const envelope = isApiFailureEnvelope(payload) ? payload : null;
		this.code = readApiErrorCode(envelope);
		this.retryable = envelope?.error?.retryable === true;
		this.retryAfterSeconds = parseRetryAfter(retryAfter);
	}

	/** Identifies terminal session failures after the caller's bounded refresh opportunity. */
	get requiresSignIn() {
		return (
			this.status === 401 &&
			[
				ApiErrorCode.SessionRequired,
				ApiErrorCode.AccessExpired,
				ApiErrorCode.RefreshStale,
				ApiErrorCode.RefreshReused,
			].some((code) => code === this.code)
		);
	}
}

function parseRetryAfter(value: string | null): number | null {
	if (value === null || value.trim() === "") return null;
	const trimmed = value.trim();
	if (/^\d+$/.test(trimmed)) {
		const seconds = Number(trimmed);
		return Number.isSafeInteger(seconds) ? seconds : null;
	}
	// Accept the HTTP-date form without treating a malformed numeric value as a date.
	if (!/^[A-Za-z]{3}, /.test(trimmed)) return null;
	const date = Date.parse(trimmed);
	return Number.isNaN(date)
		? null
		: Math.max(0, Math.ceil((date - Date.now()) / 1000));
}

export async function requestJson<TResponse>(
	input: RequestInfo | URL,
	init: JsonRequestInit = {},
): Promise<TResponse> {
	const headers = new Headers(init.headers);
	if (!headers.has("Accept")) headers.set("Accept", "application/json");
	const response = await fetch(input, { ...init, headers });
	const text = await response.text();
	let payload: unknown = null;
	if (text) {
		try {
			payload = JSON.parse(text);
		} catch {
			if (response.ok) throw new Error("Invalid JSON response");
		}
	} else if (response.ok) {
		throw new Error("Invalid JSON response");
	}
	if (!response.ok) {
		throw new HttpError(
			response.status,
			payload,
			response.headers.get("Retry-After"),
		);
	}
	return payload as TResponse;
}
