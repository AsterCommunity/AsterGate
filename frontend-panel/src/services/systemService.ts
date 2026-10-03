import type { components } from "@/types/api";
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
		accountRequest<Schema["PublicFrontendConfigResponse"]>(
			"/public/frontend-config",
		),
	setup: (input: SetupInput) =>
		accountRequest<Schema["AccountResponse"]>("/setup", "POST", input),
	settings: () =>
		accountRequest<Schema["SettingsResponse"]>("/management/settings"),
	updateSettings: (input: SettingsInput) =>
		accountRequest<Schema["SettingsResponse"]>(
			"/management/settings",
			"PUT",
			input,
		),
};
