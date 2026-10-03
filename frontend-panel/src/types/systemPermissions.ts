import type { components } from "./api";

export type SystemPermission = components["schemas"]["SystemPermissionDto"];
export const SystemPermission = {
	AccountsRead: "astergate.system.accounts.read",
	AccountsCreate: "astergate.system.accounts.create",
	AccountsSuspend: "astergate.system.accounts.suspend",
	AccountsRestore: "astergate.system.accounts.restore",
	AccountsDelete: "astergate.system.accounts.delete",
	SettingsRead: "astergate.system.settings.read",
	SettingsUpdate: "astergate.system.settings.update",
} as const satisfies Record<string, SystemPermission>;

type AssertNever<T extends never> = T;
export type SystemPermissionCoverage = AssertNever<
	Exclude<
		SystemPermission,
		(typeof SystemPermission)[keyof typeof SystemPermission]
	>
>;

export function hasSystemPermission(
	account: { system_permissions: readonly SystemPermission[] } | null,
	permission: SystemPermission,
) {
	return account?.system_permissions.includes(permission) ?? false;
}
