import type {
	ApiSuccessResponse,
	components,
	OperationJsonResponse,
} from "@/types/api";
import { isApiSuccessEnvelope } from "@/types/api-helpers";
import { csrfHeaders } from "./csrf";
import {
	requestWithSession,
	sessionChanged,
	transitionSession,
} from "./sessionRefresh";

type Schema = components["schemas"];
export type Account = Schema["AccountDto"];
export type Session = Schema["SessionDto"];
export type RegisterInput = Schema["RegisterRequest"];
export type LifecycleAction = Schema["LifecycleActionDto"];

/**
 * Sends same-origin Account REST requests with current CSRF headers and a validated success envelope.
 * Login/setup/logout share the refresh lock; security mutations fence retries from earlier identities.
 */
export async function accountRequest<T extends { data?: unknown }>(
	path: string,
	method = "GET",
	body?: unknown,
): Promise<ApiSuccessResponse<T>> {
	if (
		["/account/password", "/account/email/confirm"].includes(path) ||
		(method === "DELETE" &&
			(path === "/account" || path === "/account/sessions"))
	)
		sessionChanged();
	const send = () =>
		requestWithSession<T>(path, async () => ({
			method,
			credentials: "same-origin",
			cache: "no-store",
			headers: {
				"X-AsterGate-Request": "1",
				...(await csrfHeaders(method, `/api/v1${path}`)),
				...(body === undefined ? {} : { "Content-Type": "application/json" }),
			},
			...(body === undefined ? {} : { body: JSON.stringify(body) }),
		}));
	const response = ["/auth/login", "/auth/logout", "/setup"].includes(path)
		? await transitionSession(send)
		: await send();
	if (!isApiSuccessEnvelope(response)) {
		throw new Error("Invalid API response envelope");
	}
	return response as ApiSuccessResponse<T>;
}

export const accountService = {
	me: () => accountRequest<OperationJsonResponse<"profile">>("/account"),
	login: (identifier: string, password: string) =>
		accountRequest<OperationJsonResponse<"login">>("/auth/login", "POST", {
			identifier,
			password,
		}),
	register: (input: RegisterInput) =>
		accountRequest<OperationJsonResponse<"register", 201>>(
			"/auth/register",
			"POST",
			input,
		),
	logout: () =>
		accountRequest<OperationJsonResponse<"logout">>("/auth/logout", "POST"),
	updateProfile: (display_name: string) =>
		accountRequest<OperationJsonResponse<"update_profile">>(
			"/account",
			"PATCH",
			{
				display_name,
			},
		),
	sessions: () =>
		accountRequest<OperationJsonResponse<"sessions">>("/account/sessions"),
	revokeSession: (id: string) =>
		accountRequest<OperationJsonResponse<"revoke_session">>(
			`/account/sessions/${id}`,
			"DELETE",
		),
	revokeAll: () =>
		accountRequest<OperationJsonResponse<"revoke_all">>(
			"/account/sessions",
			"DELETE",
		),
	changePassword: (current_password: string, new_password: string) =>
		accountRequest<OperationJsonResponse<"change_password">>(
			"/account/password",
			"POST",
			{
				current_password,
				new_password,
			},
		),
	changeEmail: (email: string, password: string) =>
		accountRequest<OperationJsonResponse<"request_email_change">>(
			"/account/email",
			"POST",
			{
				email,
				password,
			},
		),
	requestVerification: (identifier: string) =>
		accountRequest<OperationJsonResponse<"request_verification">>(
			"/auth/verification",
			"POST",
			{
				identifier,
			},
		),
	requestRecovery: (identifier: string) =>
		accountRequest<OperationJsonResponse<"request_recovery">>(
			"/auth/recovery",
			"POST",
			{
				identifier,
			},
		),
	verify: (token: string) =>
		accountRequest<OperationJsonResponse<"verify_email">>(
			"/auth/verify-email",
			"POST",
			{
				token,
			},
		),
	reset: (token: string, password: string) =>
		accountRequest<OperationJsonResponse<"reset_password">>(
			"/auth/reset-password",
			"POST",
			{
				token,
				password,
			},
		),
	confirmEmail: (token: string) =>
		accountRequest<OperationJsonResponse<"confirm_email_change">>(
			"/account/email/confirm",
			"POST",
			{ token },
		),
	remove: (password: string) =>
		accountRequest<OperationJsonResponse<"delete_account">>(
			"/account",
			"DELETE",
			{
				password,
			},
		),
	export: () =>
		accountRequest<OperationJsonResponse<"export">>("/account/export"),
	manageList: (after?: string) =>
		accountRequest<OperationJsonResponse<"manage_list">>(
			`/management/accounts${after ? `?after=${encodeURIComponent(after)}` : ""}`,
			"GET",
			undefined,
		),
	provision: (input: RegisterInput) =>
		accountRequest<OperationJsonResponse<"provision", 201>>(
			"/management/accounts",
			"POST",
			input,
		),
	lifecycle: (id: string, action: LifecycleAction, current_password?: string) =>
		accountRequest<OperationJsonResponse<"manage_lifecycle">>(
			`/management/accounts/${id}/lifecycle`,
			"POST",
			{ action, current_password },
		),
};
