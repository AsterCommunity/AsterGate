import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Navigate, useNavigate } from "react-router-dom";
import { systemService } from "@/services/systemService";
import { useAccount } from "./AccountContext";
import { type SiteDraft, SiteSettingsForm } from "./SiteSettingsForm";
import {
	defaultPolicy,
	type PolicyDraft,
	SystemSettingsForm,
} from "./SystemSettingsForm";
import { Field, Notice, SiteName, Submit, useAction } from "./ui";

export function SetupPage() {
	const { t } = useTranslation("account");
	const { policy, reload } = useAccount();
	const [site, setSite] = useState<SiteDraft>({
		site_urls: [window.location.origin],
		branding_title: "AsterGate",
		branding_description: "Account and identity management",
	});
	const [account, setAccount] = useState({
		username: "",
		email: "",
		display_name: "",
		password: "",
	});
	const [settings, setSettings] = useState<PolicyDraft>(defaultPolicy);
	const action = useAction();
	const navigate = useNavigate();
	if (policy?.setup_state === "ready") return <Navigate to="/" replace />;
	const update = (key: keyof typeof account, value: string) =>
		setAccount((previous) => ({ ...previous, [key]: value }));
	return (
		<main className="mx-auto mt-8 mb-15 w-[calc(100%-40px)] max-w-[1000px] sm:mt-11 sm:w-[calc(100%-80px)] [&_a]:text-emerald-800 max-w-[800px]!">
			<div className="mb-8 flex items-center justify-between gap-3 sm:gap-5 [&_h1]:mb-0!">
				<div>
					<SiteName />
					<h1>{t("initializeAsterGate")}</h1>
				</div>
			</div>
			<form
				onSubmit={(e) => {
					e.preventDefault();
					void action.run(async () => {
						await systemService.setup({
							...site,
							initial_account: account,
							registration: settings.registration,
							email_verification: settings.email_verification,
							mail: settings.mail,
						});
						setAccount({
							username: "",
							email: "",
							display_name: "",
							password: "",
						});
						await reload();
						navigate("/", { replace: true });
					});
				}}
			>
				<SiteSettingsForm value={site} onChange={setSite} />
				<section className="border-t border-neutral-200 py-7">
					<h2>{t("initialOperatorAccount")}</h2>
					<div className="grid gap-5 max-w-[460px]">
						<Field
							label={t("displayName")}
							autoComplete="name"
							required
							maxLength={128}
							value={account.display_name}
							onChange={(e) => update("display_name", e.target.value)}
						/>
						<Field
							label={t("username")}
							autoComplete="username"
							required
							minLength={3}
							maxLength={32}
							pattern={"[A-Za-z0-9_\\-]+"}
							value={account.username}
							onChange={(e) => update("username", e.target.value)}
						/>
						<Field
							label={t("email")}
							type="email"
							autoComplete="email"
							required
							maxLength={254}
							value={account.email}
							onChange={(e) => update("email", e.target.value)}
						/>
						<Field
							label={t("newPassword")}
							type="password"
							autoComplete="new-password"
							required
							minLength={12}
							maxLength={128}
							value={account.password}
							onChange={(e) => update("password", e.target.value)}
						/>
					</div>
				</section>
				<SystemSettingsForm value={settings} onChange={setSettings} />
				<div className="grid gap-5 max-w-[460px]">
					<Notice message={action.message} error={action.error} />
					<Submit busy={action.busy}>{t("completeSetup")}</Submit>
				</div>
			</form>
		</main>
	);
}
