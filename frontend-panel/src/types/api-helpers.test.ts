import { describe, expect, it } from "vitest";
import { resources } from "@/i18n/resources";
import {
	ApiErrorCode,
	errorTranslationKeys,
	isApiErrorCode,
	isApiFailureEnvelope,
	isApiSuccessEnvelope,
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

describe("controlled REST wire states", () => {
	it("preserves explicit null and empty data values", () => {
		for (const data of [null, {}, [], true, 0, ""])
			expect(isApiSuccessEnvelope({ code: "success", msg: "", data })).toBe(
				true,
			);
		expect(
			isApiFailureEnvelope({ code: "future.code", msg: "Future failure" }),
		).toBe(true);
		expect(
			isApiFailureEnvelope({
				code: "rate_limited",
				msg: "Wait",
				error: { retryable: true, diagnostic: null },
			}),
		).toBe(true);
	});
	it("rejects contradictory, missing or malformed metadata", () => {
		for (const value of [
			null,
			[],
			{},
			{ code: "success", data: true },
			{ code: "success", msg: "" },
			{ code: "success", msg: "", data: true, error: { retryable: false } },
		])
			expect(isApiSuccessEnvelope(value)).toBe(false);
		for (const value of [
			null,
			[],
			{ code: "success", msg: "" },
			{ code: "rate_limited", msg: "", data: null },
			{ code: "rate_limited", msg: "", error: null },
			{ code: "rate_limited", msg: "", error: {} },
			{ code: "rate_limited", msg: "", error: { retryable: "true" } },
			{ code: "", msg: "" },
		])
			expect(isApiFailureEnvelope(value)).toBe(false);
	});
});
