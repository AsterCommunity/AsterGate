import { createBrowserRouter } from "react-router-dom";
import { AccountLayout } from "@/account/AccountLayout";
import {
	ProfilePage,
	SecurityPage,
	SessionsPage,
} from "@/account/AccountPages";
import {
	ChallengePage,
	RequestMailPage,
	SignInPage,
	SignUpPage,
} from "@/account/AuthPages";
import { InstanceSettingsPage } from "@/account/InstanceSettingsPage";
import { ManagementPage } from "@/account/ManagementPage";
import { SetupPage } from "@/account/SetupPage";
import { ErrorPage } from "@/pages/ErrorPage";

export const router = createBrowserRouter([
	{
		path: "/",
		element: <AccountLayout />,
		errorElement: <ErrorPage />,
		children: [
			{ path: "setup", element: <SetupPage /> },
			{ path: "instance-settings", element: <InstanceSettingsPage /> },
			{ index: true, element: <ProfilePage /> },
			{ path: "security", element: <SecurityPage /> },
			{ path: "sessions", element: <SessionsPage /> },
			{ path: "sign-in", element: <SignInPage /> },
			{ path: "sign-up", element: <SignUpPage /> },
			{ path: "recover", element: <RequestMailPage /> },
			{
				path: "resend-verification",
				element: <RequestMailPage verification />,
			},
			{ path: "verify-email", element: <ChallengePage kind="verify" /> },
			{ path: "reset-password", element: <ChallengePage kind="reset" /> },
			{ path: "confirm-email", element: <ChallengePage kind="email" /> },
			{ path: "admin", element: <ManagementPage /> },
			{ path: "*", element: <ErrorPage /> },
		],
	},
]);
