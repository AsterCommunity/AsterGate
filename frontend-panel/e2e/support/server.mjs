import { spawn } from "node:child_process";
import { mkdtemp } from "node:fs/promises";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { simpleParser } from "mailparser";
import { SMTPServer } from "smtp-server";

const frontendRoot = fileURLToPath(new URL("../../", import.meta.url));
const repository = path.resolve(frontendRoot, "..");
const frontendPort = Number(process.env.AG_DEV_PORT ?? "4173");
const backendPort = Number(process.env.AG_DEV_BACKEND_PORT ?? "3000");
const smtpPort = Number(process.env.AG_DEV_SMTP_PORT ?? "4026");
const inboxPort = Number(process.env.AG_DEV_INBOX_PORT ?? "4025");
const origin = `http://127.0.0.1:${frontendPort}`;
const directory = await mkdtemp(
	path.join(os.tmpdir(), "astergate-account-dev-"),
);
const messages = [];

const smtp = new SMTPServer({
	secure: false,
	authOptional: true,
	disabledCommands: ["AUTH", "STARTTLS"],
	logger: false,
	async onData(stream, session, callback) {
		try {
			const parsed = await simpleParser(stream);
			messages.push({
				recipients: session.envelope.rcptTo.map((r) => r.address),
				subject: parsed.subject,
				text: parsed.text,
			});
			callback();
		} catch {
			callback(new Error("test mail parsing failed"));
		}
	},
});
smtp.listen(smtpPort, "127.0.0.1");
const inbox = http.createServer((request, response) => {
	response.setHeader("Cache-Control", "no-store");
	response.setHeader("Content-Type", "application/json");
	const url = new URL(request.url ?? "/", `http://127.0.0.1:${inboxPort}`);
	const recipient = url.searchParams.get("recipient");
	response.end(
		JSON.stringify(
			recipient
				? messages.filter((m) => m.recipients.includes(recipient))
				: messages,
		),
	);
});
inbox.listen(inboxPort, "127.0.0.1");

const backend = spawn("cargo", ["run", "--locked", "--features", "openapi"], {
	cwd: repository,
	stdio: "inherit",
	env: {
		...process.env,
		ASTER_CONFIG: path.join(directory, "config.toml"),
		ASTER__SERVER__PORT: String(backendPort),
		ASTER__SERVER__TEMP_DIR: path.join(directory, ".tmp"),
		ASTER__DATABASE__URL: `sqlite://${path.join(directory, "account.db")}?mode=rwc`,
		ASTER__SECURITY__MASTER_KEY: Buffer.alloc(32, 7).toString("hex"),
	},
});
const frontend = spawn(
	"bun",
	[
		"run",
		"dev",
		"--host",
		"127.0.0.1",
		"--port",
		String(frontendPort),
		"--strictPort",
	],
	{
		cwd: frontendRoot,
		stdio: "inherit",
		env: {
			...process.env,
			ASTER_DEV_BACKEND: `http://127.0.0.1:${backendPort}`,
		},
	},
);
let stopping = false;
function stop(code = 0) {
	if (stopping) return;
	stopping = true;
	backend.kill("SIGTERM");
	frontend.kill("SIGTERM");
	smtp.close();
	inbox.close();
	setTimeout(() => {
		backend.kill("SIGKILL");
		frontend.kill("SIGKILL");
		process.exit(code);
	}, 5000).unref();
}
backend.on("exit", (code) => {
	if (!stopping) stop(code ?? 1);
});
frontend.on("exit", (code) => {
	if (!stopping) stop(code ?? 1);
});
process.on("SIGINT", () => stop());
process.on("SIGTERM", () => stop());
console.log(
	`Account development: ${origin}; local test inbox: http://127.0.0.1:${inboxPort}`,
);
