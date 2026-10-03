import { defineConfig, devices } from "playwright/test";

export default defineConfig({
	testDir: "./e2e",
	testMatch: "**/*.spec.ts",
	timeout: 90_000,
	expect: {
		timeout: 5_000,
	},
	use: {
		channel: process.env.AG_BROWSER_CHANNEL,
		baseURL: "http://127.0.0.1:4173",
		trace: "retain-on-failure",
	},
	webServer: {
		command: "bun run dev:accounts",
		url: "http://127.0.0.1:4173/health/ready",
		reuseExistingServer: !process.env.CI,
		timeout: 180_000,
	},
	projects: [
		{
			name: "chromium",
			use: { ...devices["Desktop Chrome"] },
		},
	],
});
