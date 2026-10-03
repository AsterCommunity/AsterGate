import enAccount from "@/i18n/locales/en-US/account.json";
import enErrors from "@/i18n/locales/en-US/errors.json";
import zhAccount from "@/i18n/locales/zh-CN/account.json";
import zhErrors from "@/i18n/locales/zh-CN/errors.json";

export const resources = {
	"en-US": {
		account: enAccount,
		errors: enErrors,
	},
	"zh-CN": {
		account: zhAccount,
		errors: zhErrors,
	},
} as const;
