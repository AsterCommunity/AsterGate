import { describe, expect, it } from "vitest";
import { resources } from "@/i18n/resources";
import {
	ApiErrorCode,
	errorTranslationKeys,
	isApiErrorCode,
	readApiErrorCode,
} from "./api-helpers";

describe("API error contract", () => {
	it("covers every supported code and both locale catalogs", () => {
		for (const code of Object.values(ApiErrorCode)) {
			expect(isApiErrorCode(code)).toBe(true);
			if (code === ApiErrorCode.Success) continue;
			const key = errorTranslationKeys[code];
			for (const language of ["en-US", "zh-CN"] as const)
				expect(resources[language].account.errors).toHaveProperty(key);
		}
	});
	it("rejects unknown codes and malformed payloads", () => {
		for (const payload of [
			null,
			"error",
			{},
			{ code: 401 },
			{ code: "unknown" },
		])
			expect(readApiErrorCode(payload)).toBeNull();
		expect(readApiErrorCode({ code: ApiErrorCode.SessionRequired })).toBe(
			ApiErrorCode.SessionRequired,
		);
	});
});
