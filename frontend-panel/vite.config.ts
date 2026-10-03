import path from "node:path";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import { VitePWA } from "vite-plugin-pwa";

const backendTarget = process.env.ASTER_DEV_BACKEND ?? "http://127.0.0.1:3000";

// https://vite.dev/config/
export default defineConfig({
	plugins: [
		{
			name: "astergate-development-branding",
			apply: "serve",
			transformIndexHtml: (html) =>
				html
					.replaceAll("{{astergate_title}}", "AsterGate")
					.replaceAll(
						"{{astergate_description}}",
						"Account and identity management",
					),
		},
		react(),
		tailwindcss(),
		VitePWA({
			registerType: "autoUpdate",
			injectRegister: "script",
			includeAssets: ["favicon.svg"],
			manifest: {
				name: "AsterGate",
				short_name: "AsterGate",
				description: "AsterGate account and identity management",
				theme_color: "#0f172a",
				background_color: "#f8fafc",
				display: "standalone",
				icons: [
					{
						src: "/favicon.svg",
						sizes: "any",
						type: "image/svg+xml",
						purpose: "any",
					},
					{
						src: "/favicon.svg",
						sizes: "any",
						type: "image/svg+xml",
						purpose: "maskable",
					},
				],
			},
			workbox: {
				globPatterns: ["assets/**/*.{js,css,mjs,woff2}"],
				navigateFallback: undefined,
			},
			devOptions: {
				enabled: false,
				navigateFallbackAllowlist: [/^\/$/],
			},
		}),
	],
	base: "/",
	resolve: {
		alias: {
			"@": path.resolve(__dirname, "./src"),
		},
		dedupe: ["react", "react-dom"],
	},
	server: {
		proxy: {
			"/api": { target: backendTarget, changeOrigin: false },
			"/manifest.webmanifest": { target: backendTarget, changeOrigin: false },
			"/health": backendTarget,
		},
	},
	build: {
		target: "esnext",
		outDir: "dist",
		emptyOutDir: true,
	},
});
