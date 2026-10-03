import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
	LuDownload,
	LuKeyRound,
	LuLogOut,
	LuMail,
	LuMonitor,
	LuRefreshCw,
	LuShieldCheck,
	LuSmartphone,
	LuTrash2,
} from "react-icons/lu";
import { Navigate, useNavigate } from "react-router-dom";
import { accountService, type Session } from "@/services/accountService";
import { useAccount } from "./AccountContext";
import { Field, Notice, Submit, useAction } from "./ui";

export function ProfilePage() {
	const { t } = useTranslation("account");
	const { account, policy, setAccount } = useAccount();
	const [name, setName] = useState(account?.display_name ?? "");
	const action = useAction();
	if (!account) return <Navigate to="/sign-in" replace />;
	return (
		<main className="mx-auto mt-8 mb-15 w-[calc(100%-40px)] max-w-[1000px] sm:mt-11 sm:w-[calc(100%-80px)] [&_a]:text-emerald-800">
			<div className="mb-8 flex items-center justify-between gap-3 sm:gap-5 [&_h1]:mb-0!">
				<div>
					<div className="mb-2.5 text-xs text-neutral-500">
						{t("yourAccount")}
					</div>
					<h1>{t("profile")}</h1>
				</div>
				<span className="inline-flex items-center gap-1 rounded-sm bg-neutral-100 px-2 py-0.5 text-[11px] leading-normal text-neutral-500 whitespace-nowrap bg-emerald-50! text-emerald-800!">
					<LuShieldCheck />
					{t("status.active")}
				</span>
			</div>
			<section className="border-t border-neutral-200 py-7 flex items-center gap-[18px] border-t-0! pt-0! [&_h2]:mb-1! [&_h2]:text-xl! [&_p]:m-0 [&_p]:text-sm [&_p]:text-neutral-500">
				<div
					className="grid size-16 shrink-0 place-items-center rounded-full bg-emerald-50 text-2xl font-semibold text-emerald-800"
					aria-hidden="true"
				>
					{account.display_name.slice(0, 1).toUpperCase()}
				</div>
				<div>
					<h2>{account.display_name}</h2>
					<p>@{account.username}</p>
				</div>
			</section>
			<section className="border-t border-neutral-200 py-7">
				<h2>{t("personalDetails")}</h2>
				<form
					className="grid gap-5 max-w-[460px]"
					onSubmit={(e) => {
						e.preventDefault();
						void action.run(async () => {
							const result = await accountService.updateProfile(name);
							setAccount(result.data);
						}, t("saved"));
					}}
				>
					<Field
						label={t("displayName")}
						name="display_name"
						autoComplete="name"
						required
						maxLength={128}
						value={name}
						onChange={(e) => setName(e.target.value)}
					/>
					<Field label={t("username")} value={account.username} readOnly />
					<Field
						label={t("email")}
						value={account.email}
						readOnly
						type="email"
					/>
					<span
						className={
							account.email_verified
								? "-mt-3 flex items-center gap-1.5 text-xs text-emerald-800"
								: "mb-[22px] max-w-[580px] text-sm text-neutral-500"
						}
					>
						<LuShieldCheck />
						{t(account.email_verified ? "verified" : "unverified")}
					</span>
					{!account.email_verified && policy?.mail_enabled && (
						<button
							type="button"
							className="inline-flex min-h-[42px] cursor-pointer items-center justify-center gap-2 rounded-md border border-neutral-300 px-4 py-2 text-sm font-medium leading-snug break-words hover:bg-neutral-50 [&>svg]:size-[17px] [&>svg]:shrink-0"
							disabled={action.busy}
							onClick={() =>
								void action.run(async () => {
									await accountService.requestVerification(account.username);
								}, t("mailRequested"))
							}
						>
							{t("verifyMyEmail")}
						</button>
					)}
					<Notice message={action.message} error={action.error} />
					<Submit busy={action.busy}>{t("saveChanges")}</Submit>
				</form>
			</section>
			<section className="border-t border-neutral-200 py-7">
				<h2>{t("accountRecord")}</h2>
				<dl className="my-6 grid grid-cols-[80px_minmax(0,1fr)] gap-3.5 text-[13px] sm:grid-cols-[110px_minmax(0,1fr)] [&_dt]:text-neutral-500 [&_dd]:m-0 [&_dd]:break-words">
					<dt>{t("accountId")}</dt>
					<dd className="font-mono text-xs">{account.id}</dd>
					<dt>{t("createdAt")}</dt>
					<dd>{new Date(account.created_at * 1000).toLocaleDateString()}</dd>
				</dl>
				<button
					type="button"
					className="inline-flex min-h-[42px] cursor-pointer items-center justify-center gap-2 rounded-md border border-neutral-300 px-4 py-2 text-sm font-medium leading-snug break-words hover:bg-neutral-50 [&>svg]:size-[17px] [&>svg]:shrink-0"
					disabled={action.busy}
					onClick={() =>
						void action.run(async () => {
							const result = await accountService.export();
							const url = URL.createObjectURL(
								new Blob([JSON.stringify(result.data, null, 2)], {
									type: "application/json",
								}),
							);
							const link = document.createElement("a");
							link.href = url;
							link.download = "astergate-account.json";
							link.click();
							setTimeout(() => URL.revokeObjectURL(url), 1000);
						})
					}
				>
					<LuDownload />
					{t("exportAccount")}
				</button>
			</section>
		</main>
	);
}

export function SecurityPage() {
	const { t } = useTranslation("account");
	const { account, policy, setAccount, setNotice } = useAccount();
	const [current, setCurrent] = useState("");
	const [password, setPassword] = useState("");
	const [email, setEmail] = useState("");
	const [emailPassword, setEmailPassword] = useState("");
	const [deletePassword, setDeletePassword] = useState("");
	const [deleting, setDeleting] = useState(false);
	const passwordAction = useAction();
	const emailAction = useAction();
	const deleteAction = useAction();
	const navigate = useNavigate();
	if (!account) return <Navigate to="/sign-in" replace />;
	return (
		<main className="mx-auto mt-8 mb-15 w-[calc(100%-40px)] max-w-[1000px] sm:mt-11 sm:w-[calc(100%-80px)] [&_a]:text-emerald-800">
			<div className="mb-8 flex items-center justify-between gap-3 sm:gap-5 [&_h1]:mb-0!">
				<div>
					<div className="mb-2.5 text-xs text-neutral-500">
						{t("yourAccount")}
					</div>
					<h1>{t("security")}</h1>
				</div>
				<LuShieldCheck className="size-7 text-neutral-400" />
			</div>
			<section className="border-t border-neutral-200 py-7">
				<h2>
					<LuKeyRound />
					{t("changePassword")}
				</h2>
				<form
					className="grid gap-5 max-w-[460px]"
					onSubmit={(e) => {
						e.preventDefault();
						void passwordAction.run(async () => {
							await accountService.changePassword(current, password);
							setCurrent("");
							setPassword("");
							setNotice("signInAgain");
							setAccount(null);
							navigate("/sign-in", { state: { notice: t("signInAgain") } });
						});
					}}
				>
					<Field
						label={t("currentPassword")}
						type="password"
						autoComplete="current-password"
						required
						maxLength={128}
						value={current}
						onChange={(e) => setCurrent(e.target.value)}
					/>
					<Field
						label={t("newPassword")}
						type="password"
						autoComplete="new-password"
						required
						minLength={12}
						maxLength={128}
						value={password}
						onChange={(e) => setPassword(e.target.value)}
					/>
					<Notice
						message={passwordAction.message}
						error={passwordAction.error}
					/>
					<Submit busy={passwordAction.busy}>{t("changePassword")}</Submit>
				</form>
			</section>
			{policy?.mail_enabled && (
				<section className="border-t border-neutral-200 py-7">
					<h2>
						<LuMail />
						{t("changeEmail")}
					</h2>
					<p className="mb-[22px] max-w-[580px] text-sm text-neutral-500">
						{account.email}
					</p>
					<form
						className="grid gap-5 max-w-[460px]"
						onSubmit={(e) => {
							e.preventDefault();
							void emailAction.run(async () => {
								await accountService.changeEmail(email, emailPassword);
								setEmailPassword("");
							}, t("emailChangeSent"));
						}}
					>
						<Field
							label={t("newEmail")}
							type="email"
							autoComplete="email"
							required
							maxLength={254}
							value={email}
							onChange={(e) => setEmail(e.target.value)}
						/>
						<Field
							label={t("currentPassword")}
							type="password"
							autoComplete="current-password"
							required
							maxLength={128}
							value={emailPassword}
							onChange={(e) => setEmailPassword(e.target.value)}
						/>
						<Notice message={emailAction.message} error={emailAction.error} />
						<Submit busy={emailAction.busy}>{t("sendConfirmation")}</Submit>
					</form>
				</section>
			)}
			<section className="border-t border-neutral-200 py-7 [&_h2]:text-rose-700">
				<h2>{t("deleteAccount")}</h2>
				<p className="mb-[22px] max-w-[580px] text-sm text-neutral-500">
					{t("deleteWarning")}
				</p>
				{!deleting ? (
					<button
						type="button"
						className="inline-flex min-h-[42px] cursor-pointer items-center justify-center gap-2 rounded-md border border-neutral-300 px-4 py-2 text-sm font-medium leading-snug break-words hover:bg-neutral-50 [&>svg]:size-[17px] [&>svg]:shrink-0 border-rose-200! text-rose-700"
						onClick={() => setDeleting(true)}
					>
						<LuTrash2 />
						{t("deleteAccount")}
					</button>
				) : (
					<form
						className="grid gap-5 max-w-[460px]"
						onSubmit={(e) => {
							e.preventDefault();
							void deleteAction.run(async () => {
								await accountService.remove(deletePassword);
								setDeletePassword("");
								setNotice("accountDeleted");
								setAccount(null);
								navigate("/sign-in", {
									state: { notice: t("accountDeleted") },
								});
							});
						}}
					>
						<Field
							label={t("currentPassword")}
							type="password"
							autoComplete="current-password"
							required
							maxLength={128}
							value={deletePassword}
							onChange={(e) => setDeletePassword(e.target.value)}
						/>
						<Notice message={deleteAction.message} error={deleteAction.error} />
						<div className="flex flex-wrap items-center gap-2.5">
							<Submit busy={deleteAction.busy} danger>
								{t("confirmDelete")}
							</Submit>
							<button
								type="button"
								className="inline-flex min-h-[42px] cursor-pointer items-center justify-center gap-2 rounded-md border border-neutral-300 px-4 py-2 text-sm font-medium leading-snug break-words hover:bg-neutral-50 [&>svg]:size-[17px] [&>svg]:shrink-0"
								disabled={deleteAction.busy}
								onClick={() => {
									setDeleting(false);
									setDeletePassword("");
								}}
							>
								{t("cancel")}
							</button>
						</div>
					</form>
				)}
			</section>
		</main>
	);
}

export function SessionsPage() {
	const { t } = useTranslation("account");
	const { account, setAccount } = useAccount();
	const [sessions, setSessions] = useState<Session[]>([]);
	const [failed, setFailed] = useState(false);
	const [loading, setLoading] = useState(true);
	const action = useAction();
	const navigate = useNavigate();
	useEffect(() => {
		let current = true;
		void accountService
			.sessions()
			.then((r) => {
				if (current) setSessions(r.data);
			})
			.catch(() => {
				if (current) setFailed(true);
			})
			.finally(() => {
				if (current) setLoading(false);
			});
		return () => {
			current = false;
		};
	}, []);
	if (!account) return <Navigate to="/sign-in" replace />;
	const refresh = async () => {
		const result = await accountService.sessions();
		setSessions(result.data);
		setFailed(false);
	};
	return (
		<main className="mx-auto mt-8 mb-15 w-[calc(100%-40px)] max-w-[1000px] sm:mt-11 sm:w-[calc(100%-80px)] [&_a]:text-emerald-800">
			<div className="mb-8 flex items-center justify-between gap-3 sm:gap-5 [&_h1]:mb-0!">
				<div>
					<div className="mb-2.5 text-xs text-neutral-500">
						{t("yourAccount")}
					</div>
					<h1>{t("sessions")}</h1>
				</div>
				<button
					className="inline-flex size-[38px] shrink-0 cursor-pointer items-center justify-center rounded-md border border-transparent text-neutral-500 hover:bg-neutral-100 [&>svg]:size-[19px]"
					type="button"
					title={t("refresh")}
					aria-label={t("refresh")}
					disabled={action.busy}
					onClick={() => void action.run(refresh)}
				>
					<LuRefreshCw />
				</button>
			</div>
			<Notice message={action.message} error={action.error} />
			{loading ? (
				<p role="status">{t("loading")}</p>
			) : failed ? (
				<Notice error message={t("errors.session_load_failed")} />
			) : sessions.length === 0 ? (
				<p>{t("noSessions")}</p>
			) : (
				<ul className="m-0 list-none p-0 [&>li]:flex [&>li]:items-center [&>li]:gap-3 [&>li]:border-t [&>li]:border-neutral-200 [&>li]:py-6 sm:[&>li]:gap-[18px]">
					{sessions.map((session) => (
						<li key={session.id}>
							<span className="grid size-11 shrink-0 place-items-center rounded-md bg-neutral-50 text-neutral-500">
								{/Mobile|Android|iPhone/.test(session.user_agent) ? (
									<LuSmartphone />
								) : (
									<LuMonitor />
								)}
							</span>
							<div className="min-w-0 flex-1 [&_h2]:mb-2! [&_h2]:flex-wrap [&_h2]:text-sm! [&_p]:my-1 [&_p]:text-xs [&_p]:text-neutral-500">
								<h2>
									{/Mobile|Android|iPhone/.test(session.user_agent)
										? t("mobileBrowser")
										: t("webBrowser")}
									{session.current && (
										<span className="inline-flex items-center gap-1 rounded-sm bg-neutral-100 px-2 py-0.5 text-[11px] leading-normal text-neutral-500 whitespace-nowrap bg-emerald-50! text-emerald-800!">
											{t("currentSession")}
										</span>
									)}
								</h2>
								<p className="truncate">
									{session.user_agent || t("unknownDevice")}
								</p>
								<p>
									{t("createdAt")}{" "}
									{new Date(session.created_at * 1000).toLocaleString()} ·{" "}
									{t("expiresAt")}{" "}
									{new Date(session.expires_at * 1000).toLocaleString()}
								</p>
							</div>
							<button
								className="inline-flex size-[38px] shrink-0 cursor-pointer items-center justify-center rounded-md border border-transparent text-neutral-500 hover:bg-neutral-100 [&>svg]:size-[19px] text-rose-700!"
								type="button"
								title={t("revokeSession")}
								aria-label={`${t("revokeSession")} ${session.current ? t("currentSession") : session.id}`}
								disabled={action.busy}
								onClick={() =>
									void action.run(async () => {
										await accountService.revokeSession(session.id);
										if (session.current) {
											setAccount(null);
											navigate("/sign-in");
										} else await refresh();
									})
								}
							>
								<LuLogOut />
							</button>
						</li>
					))}
				</ul>
			)}
			<section className="border-t border-neutral-200 py-7">
				<button
					className="inline-flex min-h-[42px] cursor-pointer items-center justify-center gap-2 rounded-md border border-neutral-300 px-4 py-2 text-sm font-medium leading-snug break-words hover:bg-neutral-50 [&>svg]:size-[17px] [&>svg]:shrink-0 border-rose-200! text-rose-700"
					type="button"
					disabled={action.busy}
					onClick={() =>
						void action.run(async () => {
							await accountService.revokeAll();
							setAccount(null);
							navigate("/sign-in");
						})
					}
				>
					<LuLogOut />
					{t("signOutEverywhere")}
				</button>
			</section>
		</main>
	);
}
