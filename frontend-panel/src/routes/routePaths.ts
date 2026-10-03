export const appPaths = {
	setup: "/setup",
	instanceSettings: "/instance-settings",
	profile: "/",
	security: "/security",
	sessions: "/sessions",
	signIn: "/sign-in",
	signUp: "/sign-up",
	recover: "/recover",
	resendVerification: "/resend-verification",
	verifyEmail: "/verify-email",
	resetPassword: "/reset-password",
	confirmEmail: "/confirm-email",
	management: "/admin",
} as const;

export const backendPaths = {
	api: "/api/v1",
	health: "/health",
	readiness: "/health/ready",
	metrics: "/health/metrics",
	openapi: "/api-docs/openapi.json",
	swaggerUi: "/swagger-ui/",
} as const;

export const backendPathDenylist = [
	/^\/api(?:\/.*)?$/,
	/^\/health(?:\/.*)?$/,
	/^\/api-docs(?:\/.*)?$/,
	/^\/swagger-ui(?:\/.*)?$/,
] as const;
