import { expect, type Page, test } from "playwright/test";
import type { Settings } from "../src/services/systemService";

const password = "correct horse battery staple";
const nextPassword = "replacement password for browser";
const finalPassword = "recovered password for browser";

async function mailLink(page: Page, email: string, purpose: string) {
	let link = "";
	await expect
		.poll(
			async () => {
				const response = await page.request.get(
					`http://127.0.0.1:4025/?recipient=${encodeURIComponent(email)}`,
				);
				const messages = (await response.json()) as { text: string }[];
				link =
					messages
						.map(
							(m) =>
								m.text.match(
									new RegExp(`http://[^\\s]+/${purpose}#token=[a-f0-9]{64}`),
								)?.[0],
						)
						.filter((value): value is string => Boolean(value))
						.at(-1) ?? "";
				return link;
			},
			{ timeout: 20_000 },
		)
		.not.toBe("");
	return link;
}

async function signIn(page: Page, username: string, secret = password) {
	await page.goto("/sign-in");
	await page.getByLabel("Username or email").fill(username);
	await page.getByLabel("Password", { exact: true }).fill(secret);
	await page.getByRole("button", { name: "Sign in", exact: true }).click();
	await expect(
		page.getByRole("heading", { name: "Profile", exact: true }),
	).toBeVisible();
}

async function createAccount(page: Page) {
	const username = `user${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
	const email = `${username}@example.test`;
	await page.goto("/sign-up");
	await page.getByLabel("Display name").fill("Account test user");
	await page.getByLabel("Username", { exact: true }).fill(username);
	await page.getByLabel("Email", { exact: true }).fill(email);
	await page.getByLabel("New password").fill(password);
	await page.getByRole("button", { name: "Create account" }).click();
	await expect(
		page.getByRole("heading", { name: "Check your email" }),
	).toBeVisible();
	await page.goto(await mailLink(page, email, "verify-email"));
	await expect(page).not.toHaveURL(/#token=/);
	await page.getByRole("button", { name: "Verify email" }).click();
	await expect(page.getByRole("heading", { name: "Completed" })).toBeVisible();
	await signIn(page, username);
	return { username, email };
}

test.beforeEach(async ({ page }, testInfo) => {
	await page.addInitScript(() =>
		window.localStorage.setItem("astergate.language", "en-US"),
	);
	const config = await (
		await page.request.get("/api/v1/public/frontend-config")
	).json();
	if (
		config.data.account.setup_state === "needs_setup" &&
		!testInfo.title.startsWith("initial setup")
	) {
		await page.request.post("/api/v1/setup", {
			headers: { "X-AsterGate-Request": "1" },
			data: {
				site_urls: ["http://127.0.0.1:4173"],
				initial_account: {
					username: "instance-operator",
					email: "operator@example.test",
					display_name: "Instance operator",
					password,
				},
				registration: "open",
				email_verification: "required",
				mail: {
					host: "127.0.0.1",
					port: 4026,
					from_address: "accounts@example.test",
					security: "loopback_plaintext",
				},
			},
		});
		await page.context().clearCookies();
	}
});

test("initial setup skips SMTP and creates a working operator; settings can add SMTP later", async ({
	page,
}, testInfo) => {
	await page.goto("/");
	await expect(
		page.getByRole("heading", { name: "Initialize AsterGate" }),
	).toBeVisible();
	await expect(page.getByLabel("Setup credential")).toHaveCount(0);
	await page.getByLabel("Display name").fill("Instance operator");
	await page.getByLabel("Username", { exact: true }).fill("instance-operator");
	await page.getByLabel("Email", { exact: true }).fill("operator@example.test");
	await page.getByLabel("New password").fill(password);
	await page.getByLabel("Allow account registration").check();
	await page.setViewportSize({ width: 375, height: 812 });
	expect(
		await page.evaluate(
			() => document.documentElement.scrollWidth <= innerWidth,
		),
	).toBe(true);
	await page.screenshot({
		path: testInfo.outputPath("setup-mobile.png"),
		fullPage: true,
	});
	await page.getByRole("button", { name: "Complete setup" }).click();
	await expect(
		page.getByRole("heading", { name: "Profile", exact: true }),
	).toBeVisible();
	await expect(page.getByText("Unverified", { exact: true })).toBeVisible();
	await page.goto("/setup");
	await expect(
		page.getByRole("heading", { name: "Profile", exact: true }),
	).toBeVisible();
	await page.getByRole("button", { name: "Sign out", exact: true }).click();
	await page.getByRole("link", { name: "Create account" }).click();
	await page.getByLabel("Display name").fill("No email user");
	await page.getByLabel("Username", { exact: true }).fill("no-email-user");
	await page.getByLabel("Email", { exact: true }).fill("no-email@example.test");
	await page.getByLabel("New password").fill(password);
	await page.getByRole("button", { name: "Create account" }).click();
	await expect(
		page.getByRole("heading", { name: "Account created" }),
	).toBeVisible();
	await signIn(page, "no-email-user");
	await expect(page.getByText("Unverified", { exact: true })).toBeVisible();
	await page.getByRole("button", { name: "Sign out", exact: true }).click();
	await signIn(page, "instance-operator");
	await page.goto("/instance-settings");
	await page.getByLabel("Configure SMTP").check();
	await page.getByLabel("SMTP host").fill("127.0.0.1");
	await page.getByLabel("SMTP port").fill("4026");
	await page
		.getByLabel("Transport security")
		.selectOption("loopback_plaintext");
	await page.getByLabel("Sender email").fill("accounts@example.test");
	await page.getByLabel("Require email verification for new accounts").check();
	await page.getByLabel("Current password").fill(password);
	await page.getByRole("button", { name: "Save changes" }).click();
	await expect(page.getByLabel("SMTP host")).toHaveValue("127.0.0.1");
	await expect
		.poll(
			async () =>
				(
					await (
						await page.request.get("/api/v1/public/frontend-config")
					).json()
				).data.account.mail_enabled,
		)
		.toBe(true);
	await page.setViewportSize({ width: 1440, height: 900 });
	await page.screenshot({
		path: testInfo.outputPath("instance-settings-desktop.png"),
		fullPage: true,
	});
});

test("real registration, SMTP verification, profile, sessions, password change, recovery and deletion", async ({
	page,
}, testInfo) => {
	const { username, email } = await createAccount(page);
	await page.setViewportSize({ width: 1440, height: 900 });
	await page.getByLabel("Display name").fill("Updated account name");
	await page.getByRole("button", { name: "Save changes" }).click();
	await expect(
		page.getByRole("heading", { name: "Updated account name" }),
	).toBeVisible();
	await page.screenshot({
		path: testInfo.outputPath("account-desktop.png"),
		fullPage: true,
	});

	await page.setViewportSize({ width: 375, height: 812 });
	await page.getByRole("button", { name: "Language" }).click();
	await expect(
		page.getByRole("heading", { name: "账号资料", exact: true }),
	).toBeVisible();
	expect(
		await page.evaluate(
			() => document.documentElement.scrollWidth <= window.innerWidth,
		),
	).toBe(true);
	await page.screenshot({
		path: testInfo.outputPath("account-mobile.png"),
		fullPage: true,
	});
	await page.getByRole("button", { name: "语言" }).click();

	await page.goto("/sessions");
	await expect(page.getByText("Current", { exact: true })).toBeVisible();
	await page.route("**/api/v1/account/sessions", (route) =>
		route.fulfill({
			status: 503,
			contentType: "application/json",
			body: '{"code":"service_unavailable"}',
		}),
	);
	await page.reload();
	await expect(page.getByRole("alert")).toContainText(
		"Unable to load sessions",
	);
	await page.unroute("**/api/v1/account/sessions");
	await page.getByRole("button", { name: "Refresh", exact: true }).click();
	await expect(page.getByText("Current", { exact: true })).toBeVisible();

	await page.goto("/security");
	const passwordForm = page
		.locator("form")
		.filter({ has: page.getByLabel("New password") });
	await passwordForm.getByLabel("Current password").fill(password);
	await passwordForm.getByLabel("New password").fill(nextPassword);
	await passwordForm.getByRole("button", { name: "Change password" }).click();
	await expect(
		page.getByRole("heading", { name: "Sign in", exact: true }),
	).toBeVisible();
	await signIn(page, username, nextPassword);
	await page.getByRole("button", { name: "Sign out", exact: true }).click();
	await page.getByRole("link", { name: "Forgot password?" }).click();
	await expect(page).toHaveURL(/\/recover$/);
	await expect(
		page.getByRole("heading", { name: "Recover account", exact: true }),
	).toBeVisible();
	await page.getByLabel("Username or email").fill(email);
	await expect(page.getByLabel("Username or email")).toHaveValue(email);
	await page.getByRole("button", { name: "Send email" }).click();
	await expect(page.getByRole("status")).toContainText(
		"If this account is eligible",
	);
	await page.goto(await mailLink(page, email, "reset-password"));
	await page.getByLabel("New password").fill(finalPassword);
	await page.getByRole("button", { name: "Reset password" }).click();
	await expect(page.getByRole("heading", { name: "Completed" })).toBeVisible();
	await signIn(page, username, finalPassword);

	await page.goto("/security");
	await page
		.getByRole("button", { name: "Delete account", exact: true })
		.click();
	const deletion = page.locator("form").filter({
		has: page.getByRole("button", { name: "Permanently delete account" }),
	});
	await deletion.getByLabel("Current password").fill(finalPassword);
	await deletion
		.getByRole("button", { name: "Permanently delete account" })
		.click();
	await expect(page.getByRole("status")).toContainText(
		"Your account has been deleted",
	);
	await page.goto("/missing-route");
	await expect(
		page.getByRole("heading", { name: "Page not found" }),
	).toBeVisible();
});

test("email replacement stays bound to the requesting account across a fresh browser login", async ({
	page,
	browser,
}) => {
	const { username } = await createAccount(page);
	const email = `new-${username}@example.test`;
	await page.goto("/security");
	const emailForm = page
		.locator("form")
		.filter({ has: page.getByLabel("New email address") });
	await emailForm.getByLabel("New email address").fill(email);
	await emailForm.getByLabel("Current password").fill(password);
	await emailForm.getByRole("button", { name: "Send confirmation" }).click();
	await expect(page.getByRole("status")).toContainText("confirmation link");
	const context = await browser.newContext({
		baseURL: "http://127.0.0.1:4173",
	});
	const fresh = await context.newPage();
	await fresh.addInitScript(() =>
		window.localStorage.setItem("astergate.language", "en-US"),
	);
	await fresh.goto(await mailLink(page, email, "confirm-email"));
	await fresh.getByLabel("Username or email").fill(username);
	await fresh.getByLabel("Password", { exact: true }).fill(password);
	await fresh.getByRole("button", { name: "Sign in", exact: true }).click();
	await fresh.getByRole("button", { name: "Confirm email change" }).click();
	await expect(fresh.getByRole("heading", { name: "Completed" })).toBeVisible();
	await page.goto("/");
	await expect(
		page.getByRole("heading", { name: "Sign in", exact: true }),
	).toBeVisible();
	await signIn(fresh, email);
	await expect(fresh.getByLabel("Email", { exact: true })).toHaveValue(email);
	await context.close();
});

test("account operator suspension, restoration and erasure with confirmation", async ({
	page,
}) => {
	const { username } = await createAccount(page);
	await page.getByRole("button", { name: "Sign out", exact: true }).click();
	await signIn(page, "instance-operator");
	await page.goto("/admin");
	const row = page.getByRole("row").filter({ hasText: username });
	await row.getByRole("button", { name: "Suspend account" }).click();
	await page.getByRole("dialog").getByLabel("Current password").fill(password);
	await page
		.getByRole("dialog")
		.getByRole("button", { name: "Confirm", exact: true })
		.click();
	await expect(row.getByText("Suspended", { exact: true })).toBeVisible();
	await row.getByRole("button", { name: "Restore account" }).click();
	await page.getByRole("dialog").getByLabel("Current password").fill(password);
	await page
		.getByRole("dialog")
		.getByRole("button", { name: "Confirm", exact: true })
		.click();
	await expect(row.getByText("Active", { exact: true })).toBeVisible();
	await row.getByRole("button", { name: "Delete account" }).click();
	await page.getByRole("dialog").getByLabel("Current password").fill(password);
	await page
		.getByRole("dialog")
		.getByRole("button", { name: "Confirm", exact: true })
		.click();
	await expect(row).toHaveCount(0);
	await page.getByRole("button", { name: "Sign out", exact: true }).click();
	await page.goto("/");
	await expect(
		page.getByRole("heading", { name: "Sign in", exact: true }),
	).toBeVisible();
});

test("read-only system permissions show data while disabling every write control", async ({
	page,
}) => {
	await signIn(page, "instance-operator");
	const errors: string[] = [];
	page.on("pageerror", (error) => errors.push(error.message));
	await page.route("**/api/v1/account", async (route) => {
		const response = await route.fetch();
		const body = await response.json();
		body.data.system_permissions = [
			"astergate.system.accounts.read",
			"astergate.system.settings.read",
		];
		await route.fulfill({ response, json: body });
	});
	await page.goto("/admin");
	await expect(
		page.getByRole("button", { name: "Create account", exact: true }),
	).toBeDisabled();
	await expect(page.getByRole("table")).toBeVisible();
	for (const name of ["Suspend account", "Restore account", "Delete account"]) {
		for (const button of await page
			.getByRole("button", { name, exact: true })
			.all())
			await expect(button).toBeDisabled();
	}
	await page.goto("/instance-settings");
	await expect(page.getByLabel("Allow account registration")).toBeDisabled();
	await expect(
		page.getByRole("button", { name: "Save changes" }),
	).toBeDisabled();
	expect(errors).toEqual([]);
});

test("create-only system permission does not request or reveal the account directory", async ({
	page,
}) => {
	await signIn(page, "instance-operator");
	await page.route("**/api/v1/account", async (route) => {
		const response = await route.fetch();
		const body = await response.json();
		body.data.system_permissions = ["astergate.system.accounts.create"];
		await route.fulfill({ response, json: body });
	});
	let reads = 0;
	page.on("request", (request) => {
		if (
			request.method() === "GET" &&
			new URL(request.url()).pathname === "/api/v1/management/accounts"
		)
			reads++;
	});
	await page.goto("/admin");
	await expect(
		page.getByRole("button", { name: "Create account", exact: true }),
	).toBeEnabled();
	await page
		.getByRole("button", { name: "Create account", exact: true })
		.click();
	await expect(page.getByLabel("Initial password")).toBeVisible();
	expect(reads).toBe(0);
	await expect(
		page.getByText("@instance-operator", { exact: true }),
	).toHaveCount(0);
});

test("site branding is editable and alternate hosts keep independent sessions and primary email links", async ({
	page,
}, testInfo) => {
	await signIn(page, "instance-operator");
	await page.goto("/instance-settings");
	const title = "Gate".repeat(30);
	await page.getByLabel("Site name", { exact: true }).fill(title);
	await page.getByLabel("Site description").fill("Shared account portal");
	await page.getByRole("button", { name: "Add site", exact: true }).click();
	await page
		.getByLabel("Public site 2", { exact: true })
		.fill("http://localhost:4173");
	await page
		.getByRole("radio", { name: "Primary site for email links" })
		.nth(1)
		.click();
	await expect(page.getByLabel("Public site 1", { exact: true })).toHaveValue(
		"http://localhost:4173",
	);
	await page.getByLabel("Current password").fill(password);
	await page.getByRole("button", { name: "Save changes", exact: true }).click();
	await expect(page).toHaveTitle(title);
	await expect(page.getByLabel("Public site 1", { exact: true })).toHaveValue(
		"http://localhost:4173",
	);
	await expect(page.locator('meta[name="description"]')).toHaveAttribute(
		"content",
		"Shared account portal",
	);
	const manifest = await (
		await page.request.get("/manifest.webmanifest")
	).json();
	expect(manifest.name).toBe(title);
	for (const viewport of [
		{ width: 1440, height: 900 },
		{ width: 375, height: 812 },
	]) {
		await page.setViewportSize(viewport);
		const overflow = await page.evaluate(() =>
			Array.from(document.querySelectorAll("body *"))
				.filter(
					(element) =>
						element.getBoundingClientRect().right > innerWidth + 1 &&
						getComputedStyle(element).position !== "absolute" &&
						!element.closest("nav"),
				)
				.map((element) => ({
					tag: element.tagName,
					class: element.className,
					right: element.getBoundingClientRect().right,
				})),
		);
		expect(
			await page.evaluate(
				() => document.documentElement.scrollWidth <= innerWidth,
			),
			JSON.stringify(overflow),
		).toBe(true);
		await page.screenshot({
			path: testInfo.outputPath(`site-settings-${viewport.width}.png`),
			fullPage: true,
		});
	}
	const secondary = await page.context().newPage();
	await secondary.goto("http://localhost:4173/sign-in");
	await expect(secondary).toHaveTitle(title);
	await expect(
		secondary.getByRole("heading", { name: "Sign in", exact: true }),
	).toBeVisible();
	await secondary.getByLabel("Username or email").fill("instance-operator");
	await secondary.getByLabel("Password", { exact: true }).fill(password);
	await secondary.getByRole("button", { name: "Sign in", exact: true }).click();
	await expect(
		secondary.getByRole("heading", { name: "Profile", exact: true }),
	).toBeVisible();
	const cookies = await page.context().cookies();
	const primaryCookie = cookies.find(
		(cookie) => cookie.name === "ag_session" && cookie.domain === "127.0.0.1",
	);
	const secondaryCookie = cookies.find(
		(cookie) => cookie.name === "ag_session" && cookie.domain === "localhost",
	);
	expect(primaryCookie).toBeDefined();
	expect(secondaryCookie).toBeDefined();
	expect(primaryCookie?.value).not.toBe(secondaryCookie?.value);
	const username = `site${Date.now().toString(36)}`;
	const email = `${username}@example.test`;
	const registered = await secondary.request.post(
		"http://localhost:4173/api/v1/auth/register",
		{
			headers: { "X-AsterGate-Request": "1" },
			data: { username, email, display_name: "Multi-site user", password },
		},
	);
	expect(registered.status()).toBe(201);
	expect(await mailLink(page, email, "verify-email")).toMatch(
		/^http:\/\/localhost:4173\//,
	);
	await page
		.getByRole("button", { name: "Remove site 1", exact: true })
		.click();
	await page.getByLabel("Current password").fill(password);
	await page.getByRole("button", { name: "Save changes", exact: true }).click();
	await expect
		.poll(
			async () =>
				(
					(await (await page.request.get("/api/v1/management/settings")).json())
						.data as Settings
				).site_urls,
		)
		.toEqual(["http://127.0.0.1:4173"]);
	expect(
		(
			await secondary.request.get("http://localhost:4173/api/v1/account")
		).status(),
	).toBe(403);
	await page.getByLabel("Site name", { exact: true }).fill("");
	await page.getByLabel("Site description").fill("");
	await page.getByLabel("Current password").fill(password);
	await page.getByRole("button", { name: "Save changes", exact: true }).click();
	await expect(page).toHaveTitle("AsterGate");
	await expect(page.getByLabel("Site name", { exact: true })).toHaveValue(
		"AsterGate",
	);
	await expect(page.getByLabel("Site description")).toHaveValue(
		"Account and identity management",
	);
	await secondary.close();
});
