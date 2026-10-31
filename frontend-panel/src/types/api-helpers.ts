import type { components } from "./api";

export type ApiErrorCode = components["schemas"]["ApiErrorCode"];
export const ApiErrorCode = {
	Success: "success",
	ValidationFailed: "validation.failed",
	CredentialsFailed: "auth.credentials_failed",
	SessionRequired: "auth.session_required",
	Forbidden: "auth.forbidden",
	IdentifierConflict: "account.identifier_conflict",
	SettingsConflict: "settings.revision_conflict",
	NotFound: "account.not_found",
	InvalidTransition: "account.invalid_transition",
	ChallengeInvalid: "auth.challenge_invalid",
	RateLimited: "rate_limited",
	MailNotConfigured: "mail.not_configured",
	ServiceUnavailable: "service.unavailable",
	SetupRequired: "setup.required",
	AlreadyInitialized: "setup.already_initialized",
	LastOperator: "account.last_operator",
	EndpointNotFound: "endpoint.not_found",
} as const satisfies Record<string, ApiErrorCode>;

type AssertNever<T extends never> = T;
export type ApiErrorCodeCoverage = AssertNever<
	Exclude<ApiErrorCode, (typeof ApiErrorCode)[keyof typeof ApiErrorCode]>
>;
const values = new Set<string>(Object.values(ApiErrorCode));
export function isApiErrorCode(value: unknown): value is ApiErrorCode {
	return typeof value === "string" && values.has(value);
}
export function readApiErrorCode(payload: unknown): ApiErrorCode | null {
	if (typeof payload !== "object" || payload === null || !("code" in payload))
		return null;
	return isApiErrorCode(payload.code) ? payload.code : null;
}

export const errorTranslationKeys = {
	success: "success",
	"validation.failed": "invalid_input",
	"auth.credentials_failed": "invalid_credentials",
	"auth.session_required": "authentication_required",
	"auth.forbidden": "access_denied",
	"account.identifier_conflict": "account_conflict",
	"settings.revision_conflict": "settings_conflict",
	"account.not_found": "resource_not_found",
	"account.invalid_transition": "invalid_transition",
	"auth.challenge_invalid": "invalid_challenge",
	rate_limited: "rate_limited",
	"mail.not_configured": "mail_unavailable",
	"service.unavailable": "service_unavailable",
	"setup.required": "setup_required",
	"setup.already_initialized": "already_initialized",
	"account.last_operator": "last_operator",
	"endpoint.not_found": "resource_not_found",
} as const satisfies Record<ApiErrorCode, string>;

/** REST shape validation shared by product services and HTTP error consumption. */
export function isJsonRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function isApiSuccessEnvelope(
	payload: unknown,
): payload is { code: "success"; msg: string; data: unknown } {
	return (
		isJsonRecord(payload) &&
		payload.code === ApiErrorCode.Success &&
		typeof payload.msg === "string" &&
		Object.hasOwn(payload, "data") &&
		!Object.hasOwn(payload, "error")
	);
}

export function isApiFailureEnvelope(payload: unknown): payload is {
	code: string;
	msg: string;
	error?: { retryable: boolean; diagnostic?: unknown };
} {
	if (
		!isJsonRecord(payload) ||
		typeof payload.code !== "string" ||
		payload.code.length === 0 ||
		payload.code === ApiErrorCode.Success ||
		typeof payload.msg !== "string" ||
		Object.hasOwn(payload, "data")
	)
		return false;
	return (
		!Object.hasOwn(payload, "error") ||
		(isJsonRecord(payload.error) &&
			typeof payload.error.retryable === "boolean")
	);
}
