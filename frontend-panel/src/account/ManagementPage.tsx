import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
	LuArrowRight,
	LuPause,
	LuPlay,
	LuPlus,
	LuRefreshCw,
	LuTrash2,
	LuUsersRound,
} from "react-icons/lu";
import { Navigate } from "react-router-dom";
import {
	type Account,
	accountService,
	type LifecycleAction,
} from "@/services/accountService";
import {
	hasSystemPermission,
	SystemPermission,
} from "@/types/systemPermissions";
import { useAccount } from "./AccountContext";
import { Field, Notice, SiteName, Submit, useAction } from "./ui";

const statusColors = {
	active: "bg-emerald-50! text-emerald-800!",
	pending_verification: "bg-amber-50! text-amber-800!",
	suspended: "bg-rose-50! text-rose-700!",
	deleted: "bg-neutral-100! text-neutral-500!",
};

export function ManagementPage() {
	const { t } = useTranslation("account");
	const { account } = useAccount();
	const canRead = hasSystemPermission(account, SystemPermission.AccountsRead);
	const canCreate = hasSystemPermission(
		account,
		SystemPermission.AccountsCreate,
	);
	const [currentPassword, setCurrentPassword] = useState("");
	const [loadFailed, setLoadFailed] = useState(false);
	const [accounts, setAccounts] = useState<Account[]>([]);
	const [cursor, setCursor] = useState<string | null>(null);
	const [creating, setCreating] = useState(false);
	const [input, setInput] = useState({
		username: "",
		email: "",
		display_name: "",
		password: "",
	});
	const [pending, setPending] = useState<{
		account: Account;
		action: LifecycleAction;
	} | null>(null);
	const dialog = useRef<HTMLDialogElement>(null);
	const action = useAction();
	useEffect(() => {
		if (canRead)
			void accountService
				.manageList()
				.then((r) => {
					setAccounts(r.data.items);
					setCursor(r.data.next_cursor ?? null);
				})
				.catch(() => {
					setLoadFailed(true);
				});
	}, [canRead]);
	useEffect(() => {
		if (pending) dialog.current?.showModal();
		else dialog.current?.close();
	}, [pending]);
	async function refresh() {
		const r = await accountService.manageList();
		setLoadFailed(false);
		setAccounts(r.data.items);
		setCursor(r.data.next_cursor ?? null);
	}
	const update = (field: keyof typeof input, value: string) =>
		setInput((old) => ({ ...old, [field]: value }));
	if (!account) return <Navigate to="/sign-in" replace />;
	if (!canRead && !canCreate) return <Navigate to="/" replace />;
	return (
		<main className="mx-auto mt-8 mb-15 w-[calc(100%-40px)] max-w-[1000px] sm:mt-11 sm:w-[calc(100%-80px)] [&_a]:text-emerald-800">
			<div className="mb-8 flex items-center justify-between gap-3 sm:gap-5 [&_h1]:mb-0!">
				<div>
					<SiteName />
					<h1>{t("accountManagement")}</h1>
				</div>
				<LuUsersRound className="size-7 text-neutral-400" />
			</div>
			{loadFailed && <Notice error message={t("errors.service_unavailable")} />}
			{
				<>
					<div className="mb-6 flex flex-wrap items-center justify-between gap-4">
						<button
							className="inline-flex min-h-[42px] cursor-pointer items-center justify-center gap-2 rounded-md border border-neutral-300 px-4 py-2 text-sm font-medium leading-snug break-words hover:bg-neutral-50 [&>svg]:size-[17px] [&>svg]:shrink-0 border-emerald-800! bg-emerald-800! text-white hover:bg-emerald-900!"
							type="button"
							onClick={() => setCreating(!creating)}
							disabled={!canCreate}
						>
							<LuPlus />
							{t("provisionAccount")}
						</button>
						<div className="flex flex-wrap items-center gap-2.5">
							<button
								className="inline-flex size-[38px] shrink-0 cursor-pointer items-center justify-center rounded-md border border-transparent text-neutral-500 hover:bg-neutral-100 [&>svg]:size-[19px]"
								type="button"
								title={t("refresh")}
								aria-label={t("refresh")}
								disabled={action.busy || !canRead}
								onClick={() => void action.run(refresh)}
							>
								<LuRefreshCw />
							</button>
						</div>
					</div>
					{creating && canCreate && (
						<section className="border-t border-neutral-200 py-7">
							<h2>{t("provisionAccount")}</h2>
							<form
								className="grid gap-5 max-w-[460px]"
								onSubmit={(e) => {
									e.preventDefault();
									void action.run(async () => {
										await accountService.provision(input);
										setInput({
											username: "",
											email: "",
											display_name: "",
											password: "",
										});
										setCreating(false);
										if (canRead) await refresh();
									}, t("verificationSent"));
								}}
							>
								<Field
									label={t("displayName")}
									required
									maxLength={128}
									value={input.display_name}
									onChange={(e) => update("display_name", e.target.value)}
								/>
								<Field
									label={t("username")}
									required
									minLength={3}
									maxLength={32}
									pattern={"[A-Za-z0-9_\\-]+"}
									value={input.username}
									onChange={(e) => update("username", e.target.value)}
								/>
								<Field
									label={t("email")}
									type="email"
									required
									maxLength={254}
									value={input.email}
									onChange={(e) => update("email", e.target.value)}
								/>
								<Field
									label={t("temporaryPassword")}
									type="password"
									autoComplete="new-password"
									required
									minLength={12}
									maxLength={128}
									value={input.password}
									onChange={(e) => update("password", e.target.value)}
								/>
								<div className="flex flex-wrap items-center gap-2.5">
									<Submit busy={action.busy}>{t("provisionAccount")}</Submit>
									<button
										className="inline-flex min-h-[42px] cursor-pointer items-center justify-center gap-2 rounded-md border border-neutral-300 px-4 py-2 text-sm font-medium leading-snug break-words hover:bg-neutral-50 [&>svg]:size-[17px] [&>svg]:shrink-0"
										type="button"
										onClick={() => {
											setCreating(false);
											setInput({
												username: "",
												email: "",
												display_name: "",
												password: "",
											});
										}}
									>
										{t("cancel")}
									</button>
								</div>
							</form>
						</section>
					)}
					<Notice message={action.message} error={action.error} />
					<div className="mt-[18px] overflow-auto">
						<table className="w-full min-w-[620px] border-collapse text-left text-[13px] [&_th]:border-b [&_th]:border-neutral-200 [&_th]:bg-neutral-50 [&_th]:px-4 [&_th]:py-3 [&_th]:text-xs [&_th]:font-medium [&_th]:text-neutral-500 [&_td]:max-w-[270px] [&_td]:border-b [&_td]:border-neutral-200 [&_td]:p-4 [&_td]:break-words [&_strong]:block [&_strong]:font-medium [&_small]:mt-1 [&_small]:block [&_small]:text-[11px] [&_small]:text-neutral-500">
							<thead>
								<tr>
									<th>{t("account")}</th>
									<th>{t("email")}</th>
									<th>{t("statusLabel")}</th>
									<th>{t("actions")}</th>
								</tr>
							</thead>
							<tbody>
								{accounts.map((a) => (
									<tr key={a.id}>
										<td>
											<strong>{a.display_name || t("deletedAccount")}</strong>
											<small>{a.username ? `@${a.username}` : a.id}</small>
										</td>
										<td>{a.email || "-"}</td>
										<td>
											<span
												className={`inline-flex items-center gap-1 rounded-sm bg-neutral-100 px-2 py-0.5 text-[11px] leading-normal text-neutral-500 whitespace-nowrap ${statusColors[a.status]}`}
											>
												{t(`status.${a.status}`)}
											</span>
										</td>
										<td>
											<div className="flex flex-wrap items-center gap-2.5">
												{a.status !== "deleted" && (
													<>
														<button
															type="button"
															className="inline-flex size-[38px] shrink-0 cursor-pointer items-center justify-center rounded-md border border-transparent text-neutral-500 hover:bg-neutral-100 [&>svg]:size-[19px]"
															title={t(
																a.status === "suspended"
																	? "restoreAccount"
																	: "suspendAccount",
															)}
															aria-label={t(
																a.status === "suspended"
																	? "restoreAccount"
																	: "suspendAccount",
															)}
															disabled={
																action.busy ||
																!hasSystemPermission(
																	account,
																	a.status === "suspended"
																		? SystemPermission.AccountsRestore
																		: SystemPermission.AccountsSuspend,
																)
															}
															onClick={() =>
																setPending({
																	account: a,
																	action:
																		a.status === "suspended"
																			? "restore"
																			: "suspend",
																})
															}
														>
															{a.status === "suspended" ? (
																<LuPlay />
															) : (
																<LuPause />
															)}
														</button>
														<button
															type="button"
															className="inline-flex size-[38px] shrink-0 cursor-pointer items-center justify-center rounded-md border border-transparent text-neutral-500 hover:bg-neutral-100 [&>svg]:size-[19px] text-rose-700!"
															title={t("deleteAccount")}
															aria-label={t("deleteAccount")}
															disabled={
																action.busy ||
																!hasSystemPermission(
																	account,
																	SystemPermission.AccountsDelete,
																)
															}
															onClick={() =>
																setPending({ account: a, action: "delete" })
															}
														>
															<LuTrash2 />
														</button>
													</>
												)}
											</div>
										</td>
									</tr>
								))}
							</tbody>
						</table>
						{accounts.length === 0 && (
							<p className="p-10 text-center text-neutral-500">
								{t("noAccounts")}
							</p>
						)}
					</div>
					{cursor && (
						<button
							type="button"
							className="inline-flex min-h-[42px] cursor-pointer items-center justify-center gap-2 rounded-md border border-neutral-300 px-4 py-2 text-sm font-medium leading-snug break-words hover:bg-neutral-50 [&>svg]:size-[17px] [&>svg]:shrink-0 mt-6"
							disabled={action.busy}
							onClick={() =>
								void action.run(async () => {
									const r = await accountService.manageList(cursor);
									setAccounts((old) => [...old, ...r.data.items]);
									setCursor(r.data.next_cursor ?? null);
								})
							}
						>
							{t("loadMore")}
							<LuArrowRight />
						</button>
					)}
				</>
			}
			<dialog
				ref={dialog}
				className="w-[calc(100%-32px)] max-w-[440px] rounded-lg border border-neutral-200 p-7 text-neutral-800 backdrop:bg-black/40 [&_p]:mb-5 [&_p]:text-sm [&_p]:text-neutral-500 [&_p]:break-words"
				aria-labelledby="lifecycle-title"
				onCancel={() => setPending(null)}
			>
				{pending && (
					<>
						<h2 id="lifecycle-title">
							{t(
								pending.action === "delete"
									? "deleteAccount"
									: pending.action === "suspend"
										? "suspendAccount"
										: "restoreAccount",
							)}
						</h2>
						<p>{pending.account.display_name || pending.account.username}</p>
						{pending.action === "delete" && <p>{t("deleteWarning")}</p>}
						{
							<Field
								label={t("currentPassword")}
								type="password"
								autoComplete="current-password"
								required
								maxLength={128}
								value={currentPassword}
								onChange={(e) => setCurrentPassword(e.target.value)}
							/>
						}
						<Notice message={action.message} error={action.error} />
						<div className="flex flex-wrap items-center gap-2.5">
							<button
								type="button"
								className={`inline-flex min-h-[42px] cursor-pointer items-center justify-center gap-2 rounded-md border border-neutral-300 px-4 py-2 text-sm font-medium leading-snug break-words hover:bg-neutral-50 [&>svg]:size-[17px] [&>svg]:shrink-0 ${pending.action === "delete" ? "border-rose-700! bg-rose-700! text-white hover:bg-rose-800!" : "border-emerald-800! bg-emerald-800! text-white hover:bg-emerald-900!"}`}
								disabled={action.busy}
								onClick={() =>
									void action.run(async () => {
										await accountService.lifecycle(
											pending.account.id,
											pending.action,
											currentPassword,
										);
										setPending(null);
										setCurrentPassword("");
										await refresh();
									}, t("saved"))
								}
							>
								{t("confirm")}
							</button>
							<button
								type="button"
								className="inline-flex min-h-[42px] cursor-pointer items-center justify-center gap-2 rounded-md border border-neutral-300 px-4 py-2 text-sm font-medium leading-snug break-words hover:bg-neutral-50 [&>svg]:size-[17px] [&>svg]:shrink-0"
								disabled={action.busy}
								onClick={() => setPending(null)}
							>
								{t("cancel")}
							</button>
						</div>
					</>
				)}
			</dialog>
		</main>
	);
}
