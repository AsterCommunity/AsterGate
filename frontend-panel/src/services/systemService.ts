import type { components, OperationJsonResponse } from "@/types/api";
import { accountRequest } from "./accountService";

type Schema = components["schemas"];
export type SetupInput = Schema["SetupRequest"];
export type SettingsInput = Schema["SettingsRequest"];
export type Settings = Schema["SettingsDto"];
export type MailInput = Schema["MailSettingsInput"];
export type PublicBranding = Schema["PublicBrandingDto"];
export type AccountPolicy = Schema["PublicAccountPolicyDto"];
export const systemService = {
	frontendConfig: () =>
		accountRequest<OperationJsonResponse<"frontend_config">>(
			"/public/frontend-config",
		),
	setup: (input: SetupInput) =>
		accountRequest<OperationJsonResponse<"initialize", 201>>(
			"/setup",
			"POST",
			input,
		),
	settings: () =>
		accountRequest<OperationJsonResponse<"settings">>("/management/settings"),
	updateSettings: (input: SettingsInput) =>
		accountRequest<OperationJsonResponse<"update_settings">>(
			"/management/settings",
			"PUT",
			input,
		),
};
