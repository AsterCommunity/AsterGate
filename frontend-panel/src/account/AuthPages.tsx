import { type FormEvent, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { LuArrowLeft, LuArrowRight, LuMailCheck } from "react-icons/lu";
import { Link, Navigate, useLocation, useNavigate } from "react-router-dom";
import { accountService } from "@/services/accountService";
import { useAccount } from "./AccountContext";
import { Field, Notice, SiteName, Submit, useAction } from "./ui";

export function SignInPage() {
	const { t } = useTranslation("account");
	const { account, policy, setAccount, notice, setNotice } = useAccount();
	const [identifier, setIdentifier] = useState("");
	const [password, setPassword] = useState("");
	const action = useAction();
	const location = useLocation();
	if (account) return <Navigate to="/" replace />;
	function submit(e: FormEvent) {
		e.preventDefault();
		void action.run(async () => {
			const result = await accountService.login(identifier, password);
			setPassword("");
			setNotice(null);
			setAccount(result.data);
		});
	}
	return (
		<main className="mx-auto my-11 w-[calc(100%-40px)] max-w-[400px] sm:my-[72px] [&_a]:text-emerald-800">
			<SiteName />
			<h1>{t("signIn")}</h1>
			<form onSubmit={submit} className="grid gap-5">
				<Field
					label={t("identifier")}
					name="identifier"
					autoComplete="username"
					required
					maxLength={254}
					value={identifier}
					onChange={(e) => setIdentifier(e.target.value)}
				/>
				<Field
					label={t("password")}
					name="password"
					type="password"
					autoComplete="current-password"
					required
					maxLength={128}
					value={password}
					onChange={(e) => setPassword(e.target.value)}
				/>
				<Notice
					message={
						action.message ||
						(notice ? t(notice) : "") ||
						(typeof location.state?.notice === "string"
							? location.state.notice
							: "")
					}
					error={action.error}
				/>
				<Submit busy={action.busy}>
					{t("signIn")}
					<LuArrowRight />
				</Submit>
			</form>
			<div className="mt-[22px] flex flex-wrap justify-between gap-4 text-[13px]">
				{policy?.mail_enabled && (
					<Link to="/recover">{t("forgotPassword")}</Link>
				)}
				{policy?.registration_enabled && (
					<Link to="/sign-up">{t("createAccount")}</Link>
				)}
			</div>
			{policy?.mail_enabled && (
				<Link
					className="mt-5 inline-block text-[13px]"
					to="/resend-verification"
				>
					{t("resendVerification")}
				</Link>
			)}
		</main>
	);
}

export function SignUpPage() {
	const { t } = useTranslation("account");
	const { account, policy } = useAccount();
	const [input, setInput] = useState({
		username: "",
		email: "",
		display_name: "",
		password: "",
	});
	const [done, setDone] = useState(false);
	const [needsVerification, setNeedsVerification] = useState(false);
	const action = useAction();
	if (account) return <Navigate to="/" replace />;
	if (!policy?.registration_enabled) return <Navigate to="/sign-in" replace />;
	if (done)
		return (
			<main className="mx-auto my-11 w-[calc(100%-40px)] max-w-[400px] sm:my-[72px] [&_a]:text-emerald-800">
				<LuMailCheck className="mb-6 size-10 text-emerald-800" />
				<h1>{t(needsVerification ? "checkEmail" : "accountCreated")}</h1>
				{policy?.mail_enabled && <p>{t("verificationSent")}</p>}
				<Link to="/sign-in">{t("backToSignIn")}</Link>
			</main>
		);
	const update = (field: keyof typeof input, value: string) =>
		setInput((old) => ({ ...old, [field]: value }));
	return (
		<main className="mx-auto my-11 w-[calc(100%-40px)] max-w-[400px] sm:my-[72px] [&_a]:text-emerald-800">
			<Link
				className="mb-[30px] inline-flex items-center gap-2 text-[13px]"
				to="/sign-in"
			>
				<LuArrowLeft />
				{t("signIn")}
			</Link>
			<h1>{t("createAccount")}</h1>
			<form
				onSubmit={(e) => {
					e.preventDefault();
					void action.run(async () => {
						const result = await accountService.register(input);
						setNeedsVerification(result.data.status === "pending_verification");
						setInput({
							username: "",
							email: "",
							display_name: "",
							password: "",
						});
						setDone(true);
					});
				}}
				className="grid gap-5"
			>
				<Field
					label={t("displayName")}
					name="display_name"
					autoComplete="name"
					required
					maxLength={128}
					value={input.display_name}
					onChange={(e) => update("display_name", e.target.value)}
				/>
				<Field
					label={t("username")}
					name="username"
					autoComplete="username"
					required
					minLength={3}
					maxLength={32}
					pattern={"[A-Za-z0-9_\\-]+"}
					value={input.username}
					onChange={(e) => update("username", e.target.value)}
				/>
				<Field
					label={t("email")}
					name="email"
					type="email"
					autoComplete="email"
					required
					maxLength={254}
					value={input.email}
					onChange={(e) => update("email", e.target.value)}
				/>
				<Field
					label={t("newPassword")}
					name="password"
					type="password"
					autoComplete="new-password"
					required
					minLength={12}
					maxLength={128}
					value={input.password}
					onChange={(e) => update("password", e.target.value)}
				/>
				<Notice message={action.message} error={action.error} />
				<Submit busy={action.busy}>{t("createAccount")}</Submit>
			</form>
		</main>
	);
}

export function RequestMailPage({
	verification = false,
}: {
	verification?: boolean;
}) {
	const { t } = useTranslation("account");
	const [identifier, setIdentifier] = useState("");
	const action = useAction();
	return (
		<main className="mx-auto my-11 w-[calc(100%-40px)] max-w-[400px] sm:my-[72px] [&_a]:text-emerald-800">
			<Link
				className="mb-[30px] inline-flex items-center gap-2 text-[13px]"
				to="/sign-in"
			>
				<LuArrowLeft />
				{t("signIn")}
			</Link>
			<h1>{t(verification ? "resendVerification" : "recoverAccount")}</h1>
			<form
				className="grid gap-5"
				onSubmit={(e) => {
					e.preventDefault();
					void action.run(async () => {
						await (verification
							? accountService.requestVerification(identifier)
							: accountService.requestRecovery(identifier));
					}, t("mailRequested"));
				}}
			>
				<Field
					label={t("identifier")}
					name="identifier"
					autoComplete="username"
					required
					maxLength={254}
					value={identifier}
					onChange={(e) => setIdentifier(e.target.value)}
				/>
				<Notice message={action.message} error={action.error} />
				<Submit busy={action.busy}>{t("sendEmail")}</Submit>
			</form>
		</main>
	);
}

export function ChallengePage({
	kind,
}: {
	kind: "verify" | "reset" | "email";
}) {
	const { t } = useTranslation("account");
	const { account, setAccount } = useAccount();
	const [token] = useState(
		() => new URLSearchParams(window.location.hash.slice(1)).get("token") ?? "",
	);
	const [password, setPassword] = useState("");
	const [done, setDone] = useState(false);
	useEffect(() => {
		window.history.replaceState(
			window.history.state,
			"",
			window.location.pathname,
		);
	}, []);
	const action = useAction();
	const navigate = useNavigate();
	// Tokens stay in memory and never enter query strings, browser storage, or request logs.
	const title =
		kind === "verify"
			? "verifyEmail"
			: kind === "reset"
				? "resetPassword"
				: "confirmEmail";
	if (kind === "email" && !account && !done) return <SignInPage />;
	return (
		<main className="mx-auto my-11 w-[calc(100%-40px)] max-w-[400px] sm:my-[72px] [&_a]:text-emerald-800">
			<h1>{t(done ? "completed" : title)}</h1>
			{done ? (
				<>
					<Notice
						message={t(kind === "verify" ? "emailVerified" : "signInAgain")}
					/>
					<Link to="/sign-in">{t("signIn")}</Link>
				</>
			) : !token ? (
				<>
					<Notice error message={t("errors.invalid_challenge")} />
					<Link to={kind === "reset" ? "/recover" : "/resend-verification"}>
						{t("requestNewLink")}
					</Link>
				</>
			) : kind === "email" && !account ? (
				<>
					<p>{t("signInToConfirm")}</p>
					<Link to="/sign-in">{t("signIn")}</Link>
				</>
			) : (
				<form
					className="grid gap-5"
					onSubmit={(e) => {
						e.preventDefault();
						void action.run(async () => {
							if (kind === "verify") await accountService.verify(token);
							else if (kind === "reset")
								await accountService.reset(token, password);
							else await accountService.confirmEmail(token);
							window.history.replaceState(null, "", window.location.pathname);
							setPassword("");
							if (kind !== "verify") setAccount(null);
							setDone(true);
						});
					}}
				>
					{kind === "reset" && (
						<Field
							label={t("newPassword")}
							name="password"
							type="password"
							autoComplete="new-password"
							required
							minLength={12}
							maxLength={128}
							value={password}
							onChange={(e) => setPassword(e.target.value)}
						/>
					)}
					<Notice message={action.message} error={action.error} />
					<Submit busy={action.busy}>{t(title)}</Submit>
					{action.error && (
						<button
							type="button"
							className="inline-flex min-h-[42px] cursor-pointer items-center justify-center gap-2 rounded-md border border-neutral-300 px-4 py-2 text-sm font-medium leading-snug break-words hover:bg-neutral-50 [&>svg]:size-[17px] [&>svg]:shrink-0 border-transparent! bg-transparent! hover:bg-neutral-50!"
							onClick={() =>
								navigate(kind === "reset" ? "/recover" : "/resend-verification")
							}
						>
							{t("requestNewLink")}
						</button>
					)}
				</form>
			)}
		</main>
	);
}
