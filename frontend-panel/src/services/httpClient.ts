type JsonRequestInit = Omit<RequestInit, "headers"> & {
	headers?: HeadersInit;
};

import { readApiErrorCode } from "@/types/api-helpers";

export class HttpError extends Error {
	readonly status: number;
	readonly payload: unknown;
	readonly code: ReturnType<typeof readApiErrorCode>;
	readonly retryable: boolean;

	constructor(status: number, payload: unknown) {
		super(`HTTP request failed with status ${status}`);
		this.name = "HttpError";
		this.status = status;
		this.payload = payload;
		this.code = readApiErrorCode(payload);
		this.retryable =
			typeof payload === "object" &&
			payload !== null &&
			"error" in payload &&
			typeof payload.error === "object" &&
			payload.error !== null &&
			"retryable" in payload.error &&
			payload.error.retryable === true;
	}
}

export async function requestJson<TResponse>(
	input: RequestInfo | URL,
	init: JsonRequestInit = {},
) {
	const response = await fetch(input, {
		...init,
		headers: {
			Accept: "application/json",
			...init.headers,
		},
	});
	const text = await response.text();
	let payload: unknown = null;
	if (text) {
		try {
			payload = JSON.parse(text);
		} catch {
			if (response.ok) throw new Error("Invalid JSON response");
		}
	}

	if (!response.ok) {
		throw new HttpError(response.status, payload);
	}

	return payload as TResponse;
}
