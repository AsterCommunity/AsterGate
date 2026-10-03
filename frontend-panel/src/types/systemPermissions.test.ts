import { describe, expect, it } from "vitest";
import { hasSystemPermission, SystemPermission } from "./systemPermissions";

describe("system permissions", () => {
	it("requires the exact operation and never expands read into write", () => {
		const reader = {
			system_permissions: [
				SystemPermission.AccountsRead,
				SystemPermission.SettingsRead,
			],
		};
		expect(hasSystemPermission(reader, SystemPermission.AccountsRead)).toBe(
			true,
		);
		expect(hasSystemPermission(reader, SystemPermission.SettingsRead)).toBe(
			true,
		);
		for (const permission of [
			SystemPermission.AccountsCreate,
			SystemPermission.AccountsSuspend,
			SystemPermission.AccountsRestore,
			SystemPermission.AccountsDelete,
			SystemPermission.SettingsUpdate,
		])
			expect(hasSystemPermission(reader, permission)).toBe(false);
		expect(hasSystemPermission(null, SystemPermission.AccountsRead)).toBe(
			false,
		);
		expect(
			hasSystemPermission(
				{ system_permissions: [] },
				SystemPermission.AccountsRead,
			),
		).toBe(false);
	});
	it("lists only seven distinct registered system operations", () => {
		const permissions = Object.values(SystemPermission);
		expect(new Set(permissions).size).toBe(7);
		for (const permission of permissions)
			expect(permission).toMatch(
				/^astergate\.system\.(accounts|settings)\.(read|create|suspend|restore|delete|update)$/,
			);
	});
});
