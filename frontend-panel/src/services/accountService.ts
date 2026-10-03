import type { components } from "@/types/api";
import { ApiErrorCode } from "@/types/api-helpers";
import { requestJson } from "./httpClient";

type Schema = components["schemas"];
export type Account = Schema["AccountDto"];
export type Session = Schema["SessionDto"];
export type RegisterInput = Schema["RegisterRequest"];
export type LifecycleAction = Schema["LifecycleActionDto"];

export async function accountRequest<T>(
	path: string,
	method = "GET",
	body?: unknown,
) {
	const response = await requestJson<T>(`/api/v1${path}`, {
		method,
		credentials: "same-origin",
		cache: "no-store",
		headers: {
			"X-AsterGate-Request": "1",
			...(body === undefined ? {} : { "Content-Type": "application/json" }),
		},
		...(body === undefined ? {} : { body: JSON.stringify(body) }),
	});
	if (
		typeof response !== "object" ||
		response === null ||
		!("code" in response) ||
		response.code !== ApiErrorCode.Success ||
		!("data" in response)
	) {
		throw new Error("Invalid API response envelope");
	}
	return response;
}

export const accountService = {
	me: () => accountRequest<Schema["AccountResponse"]>("/account"),
	login: (identifier: string, password: string) =>
		accountRequest<Schema["AccountResponse"]>("/auth/login", "POST", {
			identifier,
			password,
		}),
	register: (input: RegisterInput) =>
		accountRequest<Schema["AccountResponse"]>("/auth/register", "POST", input),
	logout: () =>
		accountRequest<Schema["SuccessResponse"]>("/auth/logout", "POST"),
	updateProfile: (display_name: string) =>
		accountRequest<Schema["AccountResponse"]>("/account", "PATCH", {
			display_name,
		}),
	sessions: () =>
		accountRequest<Schema["SessionsResponse"]>("/account/sessions"),
	revokeSession: (id: string) =>
		accountRequest<Schema["SuccessResponse"]>(
			`/account/sessions/${id}`,
			"DELETE",
		),
	revokeAll: () =>
		accountRequest<Schema["SuccessResponse"]>("/account/sessions", "DELETE"),
	changePassword: (current_password: string, new_password: string) =>
		accountRequest<Schema["SuccessResponse"]>("/account/password", "POST", {
			current_password,
			new_password,
		}),
	changeEmail: (email: string, password: string) =>
		accountRequest<Schema["SuccessResponse"]>("/account/email", "POST", {
			email,
			password,
		}),
	requestVerification: (identifier: string) =>
		accountRequest<Schema["SuccessResponse"]>("/auth/verification", "POST", {
			identifier,
		}),
	requestRecovery: (identifier: string) =>
		accountRequest<Schema["SuccessResponse"]>("/auth/recovery", "POST", {
			identifier,
		}),
	verify: (token: string) =>
		accountRequest<Schema["SuccessResponse"]>("/auth/verify-email", "POST", {
			token,
		}),
	reset: (token: string, password: string) =>
		accountRequest<Schema["SuccessResponse"]>("/auth/reset-password", "POST", {
			token,
			password,
		}),
	confirmEmail: (token: string) =>
		accountRequest<Schema["SuccessResponse"]>(
			"/account/email/confirm",
			"POST",
			{ token },
		),
	remove: (password: string) =>
		accountRequest<Schema["SuccessResponse"]>("/account", "DELETE", {
			password,
		}),
	export: () => accountRequest<Schema["ExportResponse"]>("/account/export"),
	manageList: (after?: string) =>
		accountRequest<Schema["AccountsResponse"]>(
			`/management/accounts${after ? `?after=${encodeURIComponent(after)}` : ""}`,
			"GET",
			undefined,
		),
	provision: (input: RegisterInput) =>
		accountRequest<Schema["AccountResponse"]>(
			"/management/accounts",
			"POST",
			input,
		),
	lifecycle: (id: string, action: LifecycleAction, current_password?: string) =>
		accountRequest<Schema["AccountResponse"]>(
			`/management/accounts/${id}/lifecycle`,
			"POST",
			{ action, current_password },
		),
};
