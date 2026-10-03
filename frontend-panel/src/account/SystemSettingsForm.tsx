import { useTranslation } from "react-i18next";
import type { MailInput, SettingsInput } from "@/services/systemService";
import { Field } from "./ui";

export type PolicyDraft = Pick<
	SettingsInput,
	| "registration"
	| "email_verification"
	| "session_ttl_seconds"
	| "challenge_ttl_seconds"
	| "mail"
>;
export const defaultPolicy: PolicyDraft = {
	registration: "closed",
	email_verification: "optional",
	session_ttl_seconds: 86400,
	challenge_ttl_seconds: 1800,
	mail: null,
};
const defaultMail: MailInput = {
	host: "",
	port: 587,
	username: "",
	password: "",
	from_address: "",
	from_name: "AsterGate",
	security: "start_tls",
};

export function SystemSettingsForm({
	value,
	onChange,
	lifetimes = false,
	hasPassword = false,
}: {
	value: PolicyDraft;
	onChange: (value: PolicyDraft) => void;
	lifetimes?: boolean;
	hasPassword?: boolean;
}) {
	const { t } = useTranslation("account");
	const mail = value.mail;
	const setMail = (patch: Partial<MailInput>) =>
		onChange({ ...value, mail: { ...defaultMail, ...mail, ...patch } });
	return (
		<>
			<section className="border-t border-neutral-200 py-7">
				<h2>{t("registrationPolicy")}</h2>
				<div className="grid gap-5 max-w-[460px]">
					<label className="flex items-center gap-2.5 text-sm leading-relaxed [&>input]:size-4 [&>input]:shrink-0 [&>input]:accent-emerald-800">
						<input
							type="checkbox"
							checked={value.registration === "open"}
							onChange={(e) =>
								onChange({
									...value,
									registration: e.target.checked ? "open" : "closed",
								})
							}
						/>
						{t("allowRegistration")}
					</label>
					<label className="flex items-center gap-2.5 text-sm leading-relaxed [&>input]:size-4 [&>input]:shrink-0 [&>input]:accent-emerald-800">
						<input
							type="checkbox"
							checked={value.email_verification === "required"}
							onChange={(e) =>
								onChange({
									...value,
									email_verification: e.target.checked
										? "required"
										: "optional",
								})
							}
						/>
						{t("requireEmailVerification")}
					</label>
				</div>
			</section>
			<section className="border-t border-neutral-200 py-7">
				<h2>{t("outboundEmail")}</h2>
				<div className="grid gap-5 max-w-[460px]">
					<label className="flex items-center gap-2.5 text-sm leading-relaxed [&>input]:size-4 [&>input]:shrink-0 [&>input]:accent-emerald-800">
						<input
							type="checkbox"
							checked={Boolean(mail)}
							onChange={(e) =>
								onChange({
									...value,
									mail: e.target.checked ? { ...defaultMail } : null,
								})
							}
						/>
						{t("configureSmtp")}
					</label>
					{mail && (
						<>
							<Field
								label={t("smtpHost")}
								required
								maxLength={253}
								value={mail.host}
								onChange={(e) => setMail({ host: e.target.value })}
							/>
							<Field
								label={t("smtpPort")}
								type="number"
								required
								min={1}
								max={65535}
								value={mail.port}
								onChange={(e) => setMail({ port: Number(e.target.value) })}
							/>
							<label className="grid min-w-0 gap-2 text-[13px] font-medium [&_select]:min-h-11 [&_select]:w-full [&_select]:rounded-md [&_select]:border [&_select]:border-neutral-300 [&_select]:bg-white [&_select]:px-3 [&_select]:py-2.5 [&_select]:text-sm [&_select]:font-normal">
								<span>{t("smtpSecurity")}</span>
								<select
									value={mail.security ?? "start_tls"}
									onChange={(e) =>
										setMail({
											security: e.target.value as MailInput["security"],
										})
									}
								>
									<option value="start_tls">STARTTLS</option>
									<option value="implicit_tls">TLS</option>
									<option value="loopback_plaintext">
										{t("loopbackSmtp")}
									</option>
								</select>
							</label>
							<Field
								label={t("smtpUsernameOptional")}
								autoComplete="off"
								maxLength={254}
								value={mail.username ?? ""}
								onChange={(e) => setMail({ username: e.target.value })}
							/>
							<Field
								label={t("smtpPasswordOptional")}
								type="password"
								autoComplete="new-password"
								maxLength={4096}
								value={mail.password ?? ""}
								placeholder={hasPassword ? t("secretUnchanged") : ""}
								onChange={(e) => setMail({ password: e.target.value })}
							/>
							{hasPassword && (
								<label className="flex items-center gap-2.5 text-sm leading-relaxed [&>input]:size-4 [&>input]:shrink-0 [&>input]:accent-emerald-800">
									<input
										type="checkbox"
										checked={mail.password === ""}
										onChange={(e) =>
											setMail({ password: e.target.checked ? "" : null })
										}
									/>
									{t("clearSmtpPassword")}
								</label>
							)}
							<Field
								label={t("senderEmail")}
								type="email"
								required
								maxLength={254}
								value={mail.from_address}
								onChange={(e) => setMail({ from_address: e.target.value })}
							/>
							<Field
								label={t("senderName")}
								maxLength={128}
								value={mail.from_name ?? "AsterGate"}
								onChange={(e) => setMail({ from_name: e.target.value })}
							/>
						</>
					)}
				</div>
			</section>
			{lifetimes && (
				<section className="border-t border-neutral-200 py-7">
					<h2>{t("sessionPolicy")}</h2>
					<div className="grid gap-5 max-w-[460px]">
						<Field
							label={t("sessionLifetime")}
							type="number"
							required
							min={300}
							max={2592000}
							value={value.session_ttl_seconds}
							onChange={(e) =>
								onChange({
									...value,
									session_ttl_seconds: Number(e.target.value),
								})
							}
						/>
						<Field
							label={t("challengeLifetime")}
							type="number"
							required
							min={60}
							max={86400}
							value={value.challenge_ttl_seconds}
							onChange={(e) =>
								onChange({
									...value,
									challenge_ttl_seconds: Number(e.target.value),
								})
							}
						/>
					</div>
				</section>
			)}
		</>
	);
}
