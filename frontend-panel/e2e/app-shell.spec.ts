import { expect, type Page, test } from "playwright/test";
import type { Settings } from "../src/services/systemService";

const password = "correct horse battery staple";
const nextPassword = "replacement password for browser";
const finalPassword = "recovered password for browser";

async function mutationHeaders(page: Page, origin = "http://127.0.0.1:4173") {
	await page.request.get(`${origin}/api/v1/setup/status`);
	const token = (await page.context().cookies(origin)).find(
		(cookie) => cookie.name === "ag_csrf",
	)?.value;
	expect(token).toBeDefined();
	return { "X-AsterGate-Request": "1", "X-AsterGate-CSRF": token ?? "" };
}

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

/** Waits for committed logout and credential clearing before any full-page navigation can cancel it. */
async function signOut(page: Page) {
	await page.getByRole("button", { name: "Sign out", exact: true }).click();
	// A full-page navigation can cancel a queued Web Lock before logout reaches the server.
	await expect(
		page.getByRole("heading", { name: "Sign in", exact: true }),
	).toBeVisible();
	expect(
		(await page.context().cookies()).filter((cookie) =>
			["ag_access", "ag_refresh"].includes(cookie.name),
		),
	).toHaveLength(0);
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
			headers: await mutationHeaders(page),
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
	await signOut(page);
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
	await signOut(page);
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

test("browser enforces CSRF and rotates readable host token beside HttpOnly session", async ({
	page,
}) => {
	await page.goto("/sign-in");
	await expect(
		page.getByRole("button", { name: "Sign in", exact: true }),
	).toBeVisible();
	const results = await page.evaluate(async (password) => {
		const readToken = () =>
			document.cookie
				.split(";")
				.map((value) => value.trim())
				.find((value) => value.startsWith("ag_csrf="))
				?.slice("ag_csrf=".length) ?? "";
		const before = readToken();
		const login = async (token: string | null, secret: string) => {
			const headers: Record<string, string> = {
				"Content-Type": "application/json",
				"X-AsterGate-Request": "1",
			};
			if (token !== null) headers["X-AsterGate-CSRF"] = token;
			const response = await fetch("/api/v1/auth/login", {
				method: "POST",
				credentials: "same-origin",
				headers,
				body: JSON.stringify({
					identifier: "instance-operator",
					password: secret,
				}),
			});
			return {
				status: response.status,
				code: (await response.json()).code,
				cache: response.headers.get("cache-control"),
				referrer: response.headers.get("referrer-policy"),
			};
		};
		const missing = await login(null, password);
		const mismatch = await login("x".repeat(43), password);
		const credentials = await login(before, "incorrect browser password");
		const signedIn = await login(before, password);
		return {
			before,
			after: readToken(),
			missing,
			mismatch,
			credentials,
			signedIn,
			visibleCookies: document.cookie,
		};
	}, password);
	expect(results.before).toMatch(/^[A-Za-z0-9_-]{32,128}$/);
	for (const denied of [results.missing, results.mismatch]) {
		expect(denied).toEqual({
			status: 403,
			code: "auth.forbidden",
			cache: "no-store",
			referrer: "no-referrer",
		});
	}
	expect(results.credentials.status).toBe(401);
	expect(results.credentials.code).toBe("auth.credentials_failed");
	expect(results.signedIn.status).toBe(200);
	expect(results.after).not.toBe(results.before);
	expect(results.visibleCookies).not.toContain("ag_access=");
	const cookies = await page.context().cookies("http://127.0.0.1:4173");
	expect(cookies.find((cookie) => cookie.name === "ag_csrf")).toMatchObject({
		httpOnly: false,
		path: "/",
		sameSite: "Strict",
		domain: "127.0.0.1",
	});
	expect(cookies.find((cookie) => cookie.name === "ag_access")).toMatchObject({
		httpOnly: true,
		path: "/",
		sameSite: "Lax",
		domain: "127.0.0.1",
	});
	const logout = await page.evaluate(async () => {
		const token =
			document.cookie
				.split(";")
				.map((value) => value.trim())
				.find((value) => value.startsWith("ag_csrf="))
				?.slice("ag_csrf=".length) ?? "";
		const response = await fetch("/api/v1/auth/logout", {
			method: "POST",
			credentials: "same-origin",
			headers: { "X-AsterGate-Request": "1", "X-AsterGate-CSRF": token },
		});
		return { status: response.status, visibleCookies: document.cookie };
	});
	expect(logout.status).toBe(200);
	expect(logout.visibleCookies).not.toContain("ag_csrf=");
	expect(
		(await page.context().cookies()).some((cookie) =>
			["ag_csrf", "ag_access", "ag_refresh"].includes(cookie.name),
		),
	).toBe(false);
});

test("JWT cookies refresh once across two tabs and a revoked session cannot refresh", async ({
	page,
}) => {
	await signIn(page, "instance-operator");
	const context = page.context();
	const peer = await context.newPage();
	await peer.goto("/");
	await expect(
		peer.getByRole("heading", { name: "Profile", exact: true }),
	).toBeVisible();
	const original = await context.cookies();
	const refresh = original.find((cookie) => cookie.name === "ag_refresh");
	expect(refresh).toMatchObject({
		httpOnly: true,
		path: "/api/v1/auth",
		sameSite: "Lax",
		domain: "127.0.0.1",
	});
	expect(await page.evaluate(() => document.cookie)).not.toContain(
		"ag_refresh=",
	);
	let refreshRequests = 0;
	context.on("request", (request) => {
		if (
			request.method() === "POST" &&
			request.url().endsWith("/api/v1/auth/refresh")
		)
			refreshRequests++;
	});
	// Losing access through cookie expiry has the same browser transport behavior as removing it.
	await context.clearCookies({ name: "ag_access" });
	await Promise.all([page.reload(), peer.reload()]);
	await expect(
		page.getByRole("heading", { name: "Profile", exact: true }),
	).toBeVisible();
	await expect(
		peer.getByRole("heading", { name: "Profile", exact: true }),
	).toBeVisible();
	expect(refreshRequests).toBe(1);
	const next = (await context.cookies()).find(
		(cookie) => cookie.name === "ag_refresh",
	);
	expect(next?.value).not.toBe(refresh?.value);
	expect(next?.expires).toBeLessThanOrEqual((refresh?.expires ?? 0) + 1);
	const sessions = await (
		await page.request.get("/api/v1/account/sessions")
	).json();
	const current = (sessions.data as { id: string; current: boolean }[]).find(
		(session) => session.current,
	);
	expect(current).toBeDefined();
	const revoked = await page.request.delete(
		`/api/v1/account/sessions/${current?.id}`,
		{ headers: await mutationHeaders(page) },
	);
	expect(revoked.status()).toBe(200);
	await Promise.all([page.reload(), peer.reload()]);
	await expect(
		page.getByRole("heading", { name: "Sign in", exact: true }),
	).toBeVisible();
	await expect(
		peer.getByRole("heading", { name: "Sign in", exact: true }),
	).toBeVisible();
	const storage = await page.evaluate(() =>
		[...Object.values(localStorage), ...Object.values(sessionStorage)].join(
			" ",
		),
	);
	expect(storage).not.toContain(refresh?.value);
	expect(storage).not.toContain(next?.value);
	await peer.close();
});

test("an old profile mutation cannot cross a two-tab login when lifecycle notifications are unavailable", async ({
	page,
}) => {
	await page.addInitScript(() => {
		Object.defineProperty(window, "BroadcastChannel", { value: undefined });
	});
	await createAccount(page);
	const context = page.context();
	const originalCsrf = (await context.cookies()).find(
		(cookie) => cookie.name === "ag_csrf",
	)?.value;
	expect(originalCsrf).toBeDefined();
	let releaseMutation!: () => void;
	const release = new Promise<void>((resolve) => {
		releaseMutation = resolve;
	});
	let mutationStarted!: () => void;
	const started = new Promise<void>((resolve) => {
		mutationStarted = resolve;
	});
	let patches = 0;
	await page.route("**/api/v1/account", async (route) => {
		if (route.request().method() !== "PATCH" || ++patches > 1) {
			await route.continue();
			return;
		}
		mutationStarted();
		await release;
		await route.fulfill({
			status: 401,
			contentType: "application/json",
			body: JSON.stringify({ code: "auth.access_expired", msg: "expired" }),
		});
	});
	const peer = await context.newPage();
	try {
		await page.getByLabel("Display name").fill("Old account's pending edit");
		await page.getByRole("button", { name: "Save changes" }).click();
		await started;
		await peer.goto("/");
		// Queue a real login under the session lock without sending lifecycle notifications.
		// It must wait for the first write's response, then precede that write's recovery.
		const login = peer.evaluate(
			async (secret) =>
				navigator.locks.request("astergate-session-refresh", async () => {
					const csrf = document.cookie
						.split(";")
						.map((cookie) => cookie.trim())
						.find((cookie) => cookie.startsWith("ag_csrf="))
						?.slice("ag_csrf=".length);
					const response = await fetch("/api/v1/auth/login", {
						method: "POST",
						headers: {
							"Content-Type": "application/json",
							"X-AsterGate-Request": "1",
							"X-AsterGate-CSRF": csrf ?? "",
						},
						body: JSON.stringify({
							identifier: "instance-operator",
							password: secret,
						}),
					});
					return { status: response.status, ...(await response.json()) };
				}),
			password,
		);
		await expect
			.poll(() =>
				peer.evaluate(async () => {
					const locks = await navigator.locks.query();
					return (
						locks.held?.some((lock) => lock.mode === "shared") &&
						locks.pending?.some((lock) => lock.mode === "exclusive")
					);
				}),
			)
			.toBe(true);
		releaseMutation();
		const before = await login;
		expect(before.status).toBe(200);
		expect(
			(await context.cookies()).find((cookie) => cookie.name === "ag_csrf")
				?.value,
		).not.toBe(originalCsrf);
		await expect(
			page.getByRole("heading", { name: "Sign in", exact: true }),
		).toBeVisible();
		const after = await (await peer.request.get("/api/v1/account")).json();
		expect(after.data.id).toBe(before.data.id);
		expect(after.data.display_name).toBe(before.data.display_name);
		expect(patches).toBe(1);
	} finally {
		releaseMutation();
		await peer.close();
	}
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
	await signOut(page);
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
	await signOut(page);
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
	await signOut(page);
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
		(cookie) => cookie.name === "ag_access" && cookie.domain === "127.0.0.1",
	);
	const secondaryCookie = cookies.find(
		(cookie) => cookie.name === "ag_access" && cookie.domain === "localhost",
	);
	expect(primaryCookie).toBeDefined();
	expect(secondaryCookie).toBeDefined();
	expect(primaryCookie?.value).not.toBe(secondaryCookie?.value);
	const username = `site${Date.now().toString(36)}`;
	const email = `${username}@example.test`;
	const registered = await secondary.request.post(
		"http://localhost:4173/api/v1/auth/register",
		{
			headers: await mutationHeaders(secondary, "http://localhost:4173"),
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
