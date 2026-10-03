import {
	createContext,
	type ReactNode,
	useCallback,
	useContext,
	useEffect,
	useRef,
	useState,
} from "react";
import { type Account, accountService } from "@/services/accountService";
import { HttpError } from "@/services/httpClient";
import {
	type AccountPolicy,
	type PublicBranding,
	systemService,
} from "@/services/systemService";

type AccountState = {
	notice: "signInAgain" | "accountDeleted" | null;
	setNotice: (notice: "signInAgain" | "accountDeleted" | null) => void;
	account: Account | null;
	policy: AccountPolicy | null;
	branding: PublicBranding | null;
	loading: boolean;
	failure: boolean;
	setAccount: (account: Account | null) => void;
	reload: () => Promise<void>;
};
const AccountContext = createContext<AccountState | null>(null);

export function AccountProvider({ children }: { children: ReactNode }) {
	const [notice, setNotice] = useState<"signInAgain" | "accountDeleted" | null>(
		null,
	);
	const [account, setAccount] = useState<Account | null>(null);
	const [policy, setPolicy] = useState<AccountPolicy | null>(null);
	const [branding, setBranding] = useState<PublicBranding | null>(null);
	const initialized = useRef(false);
	const [loading, setLoading] = useState(true);
	const [failure, setFailure] = useState(false);
	const reload = useCallback(async () => {
		if (!initialized.current) setLoading(true);
		setFailure(false);
		try {
			const { data: config } = await systemService.frontendConfig();
			setBranding(config.branding);
			const accountResult =
				config.account.setup_state === "ready"
					? await accountService.me().catch((error: unknown) => {
							if (error instanceof HttpError && error.status === 401)
								return null;
							throw error;
						})
					: null;
			setPolicy(config.account);
			setAccount(accountResult?.data ?? null);
		} catch {
			setFailure(true);
		} finally {
			initialized.current = true;
			setLoading(false);
		}
	}, []);
	useEffect(() => {
		if (!branding) return;
		document.title = branding.title;
		document
			.querySelector('meta[name="description"]')
			?.setAttribute("content", branding.description);
	}, [branding]);
	useEffect(() => {
		void reload();
	}, [reload]);
	return (
		<AccountContext.Provider
			value={{
				account,
				policy,
				branding,
				loading,
				failure,
				setAccount,
				reload,
				notice,
				setNotice,
			}}
		>
			{children}
		</AccountContext.Provider>
	);
}

export function useAccount() {
	const context = useContext(AccountContext);
	if (!context) throw new Error("AccountProvider is required");
	return context;
}
