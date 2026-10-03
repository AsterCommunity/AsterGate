import { type ComponentProps, type ReactNode, useState } from "react";
import { useTranslation } from "react-i18next";
import { LuEye, LuEyeOff, LuLoaderCircle } from "react-icons/lu";
import { HttpError } from "@/services/httpClient";
import { ApiErrorCode, errorTranslationKeys } from "@/types/api-helpers";
import { useAccount } from "./AccountContext";

export function SiteName() {
	const { branding } = useAccount();
	return (
		<div
			className="mb-2.5 max-w-[240px] truncate text-xs text-neutral-500 sm:max-w-sm"
			title={branding?.title ?? "AsterGate"}
		>
			{branding?.title ?? "AsterGate"}
		</div>
	);
}

export function Field({
	label,
	...props
}: ComponentProps<"input"> & { label: string }) {
	const [visible, setVisible] = useState(false);
	const password = props.type === "password";
	const { t } = useTranslation("account");
	return (
		<label className="grid min-w-0 gap-2 text-[13px] font-medium [&_select]:min-h-11 [&_select]:w-full [&_select]:rounded-md [&_select]:border [&_select]:border-neutral-300 [&_select]:bg-white [&_select]:px-3 [&_select]:py-2.5 [&_select]:text-sm [&_select]:font-normal">
			<span>{label}</span>
			<span className="relative flex items-center [&>input]:min-h-11 [&>input]:w-full [&>input]:min-w-0 [&>input]:rounded-md [&>input]:border [&>input]:border-neutral-300 [&>input]:bg-white [&>input]:px-3 [&>input]:py-2.5 [&>input]:text-sm [&>input]:font-normal [&>input:read-only]:bg-neutral-50 [&>input:read-only]:text-neutral-500 [&:has(button)>input]:pr-12 [&>button]:absolute [&>button]:right-1">
				<input {...props} type={password && visible ? "text" : props.type} />
				{password && (
					<button
						className="inline-flex size-[38px] shrink-0 cursor-pointer items-center justify-center rounded-md border border-transparent text-neutral-500 hover:bg-neutral-100 [&>svg]:size-[19px]"
						type="button"
						onClick={() => setVisible(!visible)}
						title={t(visible ? "hidePassword" : "showPassword")}
						aria-label={t(visible ? "hidePassword" : "showPassword")}
					>
						{visible ? <LuEyeOff /> : <LuEye />}
					</button>
				)}
			</span>
		</label>
	);
}

export function Submit({
	busy,
	children,
	danger = false,
}: {
	busy: boolean;
	children: ReactNode;
	danger?: boolean;
}) {
	return (
		<button
			type="submit"
			disabled={busy}
			className={`inline-flex min-h-[42px] cursor-pointer items-center justify-center gap-2 rounded-md border border-neutral-300 px-4 py-2 text-sm font-medium leading-snug break-words hover:bg-neutral-50 [&>svg]:size-[17px] [&>svg]:shrink-0 ${danger ? "border-rose-700! bg-rose-700! text-white hover:bg-rose-800!" : "border-emerald-800! bg-emerald-800! text-white hover:bg-emerald-900!"}`}
		>
			{busy && <LuLoaderCircle className="motion-safe:animate-spin" />}
			{children}
		</button>
	);
}

export function Notice({
	message,
	error = false,
}: {
	message: string;
	error?: boolean;
}) {
	return message ? (
		<p
			className={`m-0 border-l-[3px] border-emerald-600 bg-emerald-50 px-3 py-2.5 text-[13px] text-emerald-900 break-words ${error ? "border-rose-700! bg-rose-50! text-rose-800!" : ""}`}
			role={error ? "alert" : "status"}
		>
			{message}
		</p>
	) : null;
}

export function useAction() {
	const { t } = useTranslation("account");
	const { setAccount } = useAccount();
	const [busy, setBusy] = useState(false);
	const [message, setMessage] = useState("");
	const [error, setError] = useState(false);
	async function run(action: () => Promise<void>, success = "") {
		if (busy) return;
		setBusy(true);
		setMessage("");
		setError(false);
		try {
			await action();
			setMessage(success);
		} catch (e) {
			setError(true);
			const code = e instanceof HttpError ? e.code : null;
			setMessage(
				t(
					`errors.${code ? errorTranslationKeys[code] : e instanceof HttpError ? "service_unavailable" : "network_error"}`,
					{ defaultValue: t("errors.service_unavailable") },
				),
			);
			if (
				e instanceof HttpError &&
				e.status === 401 &&
				code === ApiErrorCode.SessionRequired
			)
				setAccount(null);
		} finally {
			setBusy(false);
		}
	}
	return { busy, message, error, run, clear: () => setMessage("") };
}
