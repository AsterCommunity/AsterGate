import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { LuRefreshCw } from "react-icons/lu";
import { Navigate } from "react-router-dom";
import { type Settings, systemService } from "@/services/systemService";
import {
	hasSystemPermission,
	SystemPermission,
} from "@/types/systemPermissions";
import { useAccount } from "./AccountContext";
import { type SiteDraft, SiteSettingsForm } from "./SiteSettingsForm";
import { type PolicyDraft, SystemSettingsForm } from "./SystemSettingsForm";
import { Field, Notice, SiteName, Submit, useAction } from "./ui";

export function InstanceSettingsPage() {
	const { t } = useTranslation("account");
	const { account, reload } = useAccount();
	const [saved, setSaved] = useState<Settings | null>(null);
	const [draft, setDraft] = useState<PolicyDraft | null>(null);
	const [site, setSite] = useState<SiteDraft | null>(null);
	const [password, setPassword] = useState("");
	const [failed, setFailed] = useState(false);
	const action = useAction();
	const apply = useCallback((value: Settings) => {
		setSaved(value);
		setSite({
			site_urls: value.site_urls,
			branding_title: value.branding_title,
			branding_description: value.branding_description,
		});
		setDraft({
			registration: value.registration,
			email_verification: value.email_verification,
			session_ttl_seconds: value.session_ttl_seconds,
			challenge_ttl_seconds: value.challenge_ttl_seconds,
			mail: value.mail
				? {
						host: value.mail.host,
						port: value.mail.port,
						username: value.mail.username,
						from_address: value.mail.from_address,
						from_name: value.mail.from_name,
						security: value.mail.security,
						password: null,
					}
				: null,
		});
	}, []);
	useEffect(() => {
		let active = true;
		void systemService
			.settings()
			.then((r) => {
				if (active) apply(r.data);
			})
			.catch(() => {
				if (active) setFailed(true);
			});
		return () => {
			active = false;
		};
	}, [apply]);
	if (!account) return <Navigate to="/sign-in" replace />;
	if (!hasSystemPermission(account, SystemPermission.SettingsRead))
		return <Navigate to="/" replace />;
	return (
		<main className="mx-auto mt-8 mb-15 w-[calc(100%-40px)] max-w-[1000px] sm:mt-11 sm:w-[calc(100%-80px)] [&_a]:text-emerald-800">
			<div className="mb-8 flex items-center justify-between gap-3 sm:gap-5 [&_h1]:mb-0!">
				<div>
					<SiteName />
					<h1>{t("instanceSettings")}</h1>
				</div>
			</div>
			{failed ? (
				<>
					<Notice error message={t("errors.service_unavailable")} />
					<button
						className="inline-flex min-h-[42px] cursor-pointer items-center justify-center gap-2 rounded-md border border-neutral-300 px-4 py-2 text-sm font-medium leading-snug break-words hover:bg-neutral-50 [&>svg]:size-[17px] [&>svg]:shrink-0"
						type="button"
						onClick={() =>
							void action.run(async () => {
								const r = await systemService.settings();
								apply(r.data);
								setFailed(false);
							})
						}
					>
						<LuRefreshCw />
						{t("retry")}
					</button>
				</>
			) : !draft || !saved || !site ? (
				<p role="status">{t("loading")}</p>
			) : (
				<form
					onSubmit={(e) => {
						e.preventDefault();
						void action.run(async () => {
							const result = await systemService.updateSettings({
								...draft,
								...site,
								revision: saved.revision,
								current_password: password,
							});
							setPassword("");
							apply(result.data);
							if (!result.data.site_urls.includes(window.location.origin)) {
								window.location.assign(`${result.data.site_urls[0]}/sign-in`);
								return;
							}
							await reload();
						}, t("saved"));
					}}
				>
					<fieldset
						disabled={
							!hasSystemPermission(account, SystemPermission.SettingsUpdate) ||
							action.busy
						}
					>
						<SiteSettingsForm value={site} onChange={setSite} />
						<SystemSettingsForm
							value={draft}
							onChange={setDraft}
							lifetimes
							hasPassword={saved.mail?.has_password}
						/>
						<section className="border-t border-neutral-200 py-7 grid gap-5 max-w-[460px]">
							<Field
								label={t("currentPassword")}
								type="password"
								autoComplete="current-password"
								required
								maxLength={128}
								value={password}
								onChange={(e) => setPassword(e.target.value)}
							/>
							<Notice message={action.message} error={action.error} />
							<Submit busy={action.busy}>{t("saveChanges")}</Submit>
						</section>
					</fieldset>
				</form>
			)}
		</main>
	);
}
