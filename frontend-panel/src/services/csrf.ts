/** Same-origin cookie-authenticated API mutations use a readable host-only double-submit token. */
const tokenPattern = /^[A-Za-z0-9_-]{32,128}$/;
let bootstrap: Promise<void> | undefined;

function readToken(): string | undefined {
	const name = location.protocol === "https:" ? "__Host-ag_csrf" : "ag_csrf";
	const values = document.cookie
		.split(";")
		.map((value) => value.trim())
		.filter((value) => value.startsWith(`${name}=`))
		.map((value) => value.slice(name.length + 1));
	if (values.length !== 1 || !tokenPattern.test(values[0] ?? ""))
		return undefined;
	return values[0];
}

export async function csrfHeaders(
	method: string,
	input = "/api/v1/",
): Promise<Record<string, string>> {
	if (["GET", "HEAD", "OPTIONS"].includes(method.toUpperCase())) return {};
	const url = new URL(input, location.href);
	if (url.origin !== location.origin || !url.pathname.startsWith("/api/v1/"))
		return {};
	let token = readToken();
	if (!token) {
		bootstrap ??= fetch("/api/v1/setup/status", {
			credentials: "same-origin",
			// Preserve the same-host scheme during pre-setup HTTPS loopback bootstrap.
			referrerPolicy: "same-origin",
			cache: "no-store",
			headers: { Accept: "application/json" },
		})
			.then((response) => {
				if (!response.ok) throw new Error("CSRF bootstrap failed");
			})
			.finally(() => {
				bootstrap = undefined;
			});
		await bootstrap;
		token = readToken();
	}
	if (!token) throw new Error("CSRF token unavailable");
	return { "X-AsterGate-CSRF": token };
}
