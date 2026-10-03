import { useTranslation } from "react-i18next";
import {
	LuLanguages,
	LuLogOut,
	LuMonitor,
	LuRefreshCw,
	LuShieldCheck,
	LuUserRound,
	LuUsersRound,
} from "react-icons/lu";
import {
	Link,
	Navigate,
	NavLink,
	Outlet,
	useLocation,
	useNavigate,
} from "react-router-dom";
import { accountService } from "@/services/accountService";
import { useUiPreferencesStore } from "@/stores/uiPreferencesStore";
import {
	hasSystemPermission,
	SystemPermission,
} from "@/types/systemPermissions";
import { AccountProvider, useAccount } from "./AccountContext";
import { Notice, useAction } from "./ui";

function Shell() {
	const { t } = useTranslation("account");
	const { account, policy, branding, loading, failure, reload, setAccount } =
		useAccount();
	const location = useLocation();
	const language = useUiPreferencesStore((s) => s.language);
	const setLanguage = useUiPreferencesStore((s) => s.setLanguage);
	const action = useAction();
	const navigate = useNavigate();
	return (
		<div className="flex min-h-svh flex-col bg-white text-neutral-800 [&_h1]:mb-6 [&_h1]:text-2xl [&_h1]:font-semibold [&_h1]:leading-tight [&_h1]:break-words sm:[&_h1]:text-[28px] [&_h2]:mb-[18px] [&_h2]:flex [&_h2]:items-center [&_h2]:gap-2.5 [&_h2]:text-[17px] [&_h2]:font-semibold [&_p]:leading-relaxed [&_button:disabled]:cursor-wait [&_button:disabled]:opacity-50 [&_input:focus-visible]:outline-emerald-700 [&_input:focus-visible]:outline-2 [&_input:focus-visible]:outline-offset-2 [&_button:focus-visible]:outline-emerald-700 [&_button:focus-visible]:outline-2 [&_button:focus-visible]:outline-offset-2 [&_a:focus-visible]:outline-emerald-700 [&_a:focus-visible]:outline-2 [&_a:focus-visible]:outline-offset-2">
			<header className="grid min-h-[68px] grid-cols-[minmax(0,1fr)_auto] items-center gap-4 border-b border-neutral-200 px-5 py-4 sm:min-h-[76px] sm:px-10">
				<Link
					to="/"
					className="inline-flex min-w-0 max-w-full items-center gap-2 overflow-hidden text-base font-bold no-underline sm:gap-3 sm:text-xl [&>img]:size-7 [&>img]:shrink-0 sm:[&>img]:size-[34px]"
				>
					<img src="/favicon.svg" alt="" />
					<span
						className="line-clamp-2 min-w-0 max-w-sm break-all"
						title={branding?.title ?? "AsterGate"}
					>
						{branding?.title ?? "AsterGate"}
					</span>
				</Link>
				<div className="flex shrink-0 items-center gap-1 sm:gap-2.5">
					<button
						type="button"
						className="inline-flex size-[38px] shrink-0 cursor-pointer items-center justify-center rounded-md border border-transparent text-neutral-500 hover:bg-neutral-100 [&>svg]:size-[19px]"
						title={t("language")}
						aria-label={t("language")}
						onClick={() =>
							setLanguage(language === "en-US" ? "zh-CN" : "en-US")
						}
					>
						<LuLanguages />
					</button>
					{account && (
						<button
							type="button"
							className="inline-flex min-h-[42px] cursor-pointer items-center justify-center gap-2 rounded-md border border-neutral-300 px-4 py-2 text-sm font-medium leading-snug whitespace-nowrap hover:bg-neutral-50 [&>svg]:size-[17px] [&>svg]:shrink-0 border-transparent! bg-transparent! hover:bg-neutral-50!"
							disabled={action.busy}
							onClick={() =>
								void action.run(async () => {
									await accountService.logout();
									setAccount(null);
									navigate("/sign-in");
								})
							}
						>
							<LuLogOut />
							{t("signOut")}
						</button>
					)}
				</div>
			</header>
			{loading ? (
				<main className="mx-auto my-20 text-sm text-neutral-500" role="status">
					{t("loading")}
				</main>
			) : failure ? (
				<main className="mx-auto my-11 w-[calc(100%-40px)] max-w-[400px] sm:my-[72px] [&_a]:text-emerald-800">
					<h1>{t("connectionFailed")}</h1>
					<button
						type="button"
						className="inline-flex min-h-[42px] cursor-pointer items-center justify-center gap-2 rounded-md border border-neutral-300 px-4 py-2 text-sm font-medium leading-snug break-words hover:bg-neutral-50 [&>svg]:size-[17px] [&>svg]:shrink-0"
						onClick={() => void reload()}
					>
						<LuRefreshCw />
						{t("retry")}
					</button>
				</main>
			) : (
				<>
					{account && (
						<nav
							className="flex gap-6 overflow-x-auto border-b border-neutral-200 px-5 sm:gap-7 sm:px-10 [&>a]:flex [&>a]:items-center [&>a]:gap-2 [&>a]:whitespace-nowrap [&>a]:border-b-2 [&>a]:border-transparent [&>a]:py-[18px] [&>a]:text-sm [&>a]:text-neutral-500 [&>a.active]:border-emerald-800 [&>a.active]:text-emerald-800"
							aria-label={t("accountNavigation")}
						>
							<NavLink to="/" end>
								<LuUserRound />
								{t("profile")}
							</NavLink>
							<NavLink to="/security">
								<LuShieldCheck />
								{t("security")}
							</NavLink>
							<NavLink to="/sessions">
								<LuMonitor />
								{t("sessions")}
							</NavLink>
							{(hasSystemPermission(account, SystemPermission.AccountsRead) ||
								hasSystemPermission(
									account,
									SystemPermission.AccountsCreate,
								)) && (
								<NavLink to="/admin">
									<LuUsersRound />
									{t("accountManagement")}
								</NavLink>
							)}
							{hasSystemPermission(account, SystemPermission.SettingsRead) && (
								<NavLink to="/instance-settings">
									<LuShieldCheck />
									{t("instanceSettings")}
								</NavLink>
							)}
						</nav>
					)}
					<Notice message={action.message} error={action.error} />
					{policy?.setup_state === "needs_setup" &&
					location.pathname !== "/setup" ? (
						<Navigate to="/setup" replace />
					) : (
						<Outlet />
					)}
				</>
			)}
			<footer className="mt-auto flex flex-wrap items-center justify-between gap-4 border-t border-neutral-200 px-5 py-[22px] text-xs text-neutral-500 sm:px-10 [&_a]:inline-flex [&_a]:items-center [&_a]:gap-1.5">
				<span className="min-w-0 break-all">
					{branding?.description ?? "AsterGate"}
				</span>
				<Link to="/admin">
					<LuUsersRound />
					{t("accountManagement")}
				</Link>
			</footer>
		</div>
	);
}
export function AccountLayout() {
	return (
		<AccountProvider>
			<Shell />
		</AccountProvider>
	);
}
