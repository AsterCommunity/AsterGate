import { describe, expect, it } from "vitest";
import { appPaths, backendPathDenylist, backendPaths } from "./routePaths";

describe("route paths", () => {
	it("keeps account paths distinct from backend API paths", () => {
		const paths = Object.values(appPaths);
		expect(new Set(paths).size).toBe(paths.length);
		for (const path of paths)
			expect(backendPathDenylist.some((pattern) => pattern.test(path))).toBe(
				false,
			);
	});

	it.each([
		backendPaths.api,
		backendPaths.health,
		backendPaths.readiness,
		backendPaths.metrics,
		backendPaths.openapi,
		backendPaths.swaggerUi,
	])("keeps backend path %s out of the SPA fallback", (path) => {
		expect(backendPathDenylist.some((pattern) => pattern.test(path))).toBe(
			true,
		);
	});
});
