import { useTranslation } from "react-i18next";
import { LuPlus, LuTrash2 } from "react-icons/lu";
import type { SettingsInput } from "@/services/systemService";
import { Field } from "./ui";

export type SiteDraft = Pick<
	SettingsInput,
	"site_urls" | "branding_title" | "branding_description"
>;

export function SiteSettingsForm({
	value,
	onChange,
}: {
	value: SiteDraft;
	onChange: (value: SiteDraft) => void;
}) {
	const { t } = useTranslation("account");
	return (
		<section className="border-t border-neutral-200 py-7">
			<h2>{t("instanceIdentity")}</h2>
			<div className="grid max-w-[600px] gap-5">
				<Field
					label={t("siteName")}
					maxLength={120}
					value={value.branding_title}
					onChange={(e) =>
						onChange({ ...value, branding_title: e.target.value })
					}
				/>
				<label className="grid gap-2 text-[13px] font-medium">
					<span>{t("siteDescription")}</span>
					<textarea
						className="min-h-24 w-full resize-y rounded-md border border-neutral-300 bg-white px-3 py-2.5 text-sm font-normal focus-visible:outline-2 focus-visible:outline-emerald-700 focus-visible:outline-offset-2"
						rows={3}
						maxLength={300}
						value={value.branding_description}
						onChange={(e) =>
							onChange({ ...value, branding_description: e.target.value })
						}
					/>
				</label>
				{value.site_urls.map((url, index) => (
					<div
						// biome-ignore lint/suspicious/noArrayIndexKey: Rows are controlled URL slots; URL keys would remount the input on every keystroke.
						key={index}
						className="grid grid-cols-[minmax(0,1fr)_38px] items-end gap-x-2 gap-y-2"
					>
						<Field
							label={t("publicSite", { index: index + 1 })}
							type="url"
							required
							maxLength={2048}
							value={url}
							onChange={(e) =>
								onChange({
									...value,
									site_urls: value.site_urls.map((site, i) =>
										i === index ? e.target.value : site,
									),
								})
							}
						/>
						<button
							type="button"
							className="mb-0.5 inline-flex size-[38px] items-center justify-center rounded-md text-neutral-500 hover:bg-rose-50 hover:text-rose-800"
							title={t("removeSite", { index: index + 1 })}
							aria-label={t("removeSite", { index: index + 1 })}
							disabled={value.site_urls.length === 1}
							onClick={() =>
								onChange({
									...value,
									site_urls: value.site_urls.filter((_, i) => i !== index),
								})
							}
						>
							<LuTrash2 className="size-[18px]" />
						</button>
						<label className="col-span-2 flex items-center gap-2 text-xs text-neutral-600">
							<input
								type="radio"
								name="primary-site"
								className="accent-emerald-800"
								checked={index === 0}
								onChange={() =>
									onChange({
										...value,
										site_urls: [
											url,
											...value.site_urls.filter((_, i) => i !== index),
										],
									})
								}
							/>
							{t("primarySite")}
						</label>
					</div>
				))}
				<button
					type="button"
					className="inline-flex min-h-10 items-center justify-center gap-2 justify-self-start rounded-md border border-neutral-300 px-3 text-sm hover:bg-neutral-50"
					disabled={value.site_urls.length >= 16}
					onClick={() =>
						onChange({ ...value, site_urls: [...value.site_urls, ""] })
					}
				>
					<LuPlus className="size-4" />
					{t("addSite")}
				</button>
			</div>
		</section>
	);
}
