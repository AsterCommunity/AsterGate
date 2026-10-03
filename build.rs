//! Build script: inject build metadata and select an embeddable frontend artifact.

use std::env;
use std::fs;
use std::io;
use std::path::Path;

const BUILD_TIME_ENV: &str = "ASTER_BUILD_TIME";
const FRONTEND_DIST_ENV: &str = "ASTER_FRONTEND_DIST_DIR";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=frontend-panel/dist");
    println!("cargo:rerun-if-env-changed={BUILD_TIME_ENV}");

    configure_build_time()?;

    let manifest_dir = env::var("CARGO_MANIFEST_DIR")
        .map_err(|error| io::Error::other(format!("missing CARGO_MANIFEST_DIR: {error}")))?;
    let dist_path = Path::new(&manifest_dir).join("frontend-panel/dist");
    let out_dir = env::var("OUT_DIR")
        .map_err(|error| io::Error::other(format!("missing OUT_DIR: {error}")))?;
    let fallback_dist_path = Path::new(&out_dir).join("frontend-dist-fallback");
    let profile = env::var("PROFILE")
        .map_err(|error| io::Error::other(format!("missing PROFILE: {error}")))?;

    let selected_dist_path = match frontend_dist_state(&dist_path)? {
        FrontendDistState::Real => dist_path,
        FrontendDistState::Missing if fallback_allowed(&profile) => {
            eprintln!(
                "Warning: frontend-panel/dist is missing; generating isolated development fallback assets"
            );
            create_fallback_files(&manifest_dir, &fallback_dist_path)?;
            fallback_dist_path
        }
        FrontendDistState::Missing => {
            return Err(io::Error::other(format!(
                "frontend-panel/dist does not contain a production frontend for the {profile} profile; run `cd frontend-panel && bun install --frozen-lockfile && bun run build` before building"
            ))
            .into());
        }
    };

    let selected_dist_path = selected_dist_path.to_str().ok_or_else(|| {
        io::Error::other("selected frontend dist path must contain valid Unicode")
    })?;
    println!("cargo:rustc-env={FRONTEND_DIST_ENV}={selected_dist_path}");

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrontendDistState {
    Missing,
    Real,
}

fn configure_build_time() -> io::Result<()> {
    let value = match env::var(BUILD_TIME_ENV) {
        Ok(value) => value,
        Err(env::VarError::NotPresent) => {
            chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
        }
        Err(env::VarError::NotUnicode(_)) => {
            return Err(io::Error::other(format!(
                "{BUILD_TIME_ENV} must contain valid Unicode"
            )));
        }
    };

    let value = value.trim();
    if value.is_empty() {
        return Err(io::Error::other(format!(
            "{BUILD_TIME_ENV} must not be empty when set"
        )));
    }
    if value.contains('\r') || value.contains('\n') {
        return Err(io::Error::other(format!(
            "{BUILD_TIME_ENV} must be a single-line value"
        )));
    }
    println!("cargo:rustc-env={BUILD_TIME_ENV}={value}");

    Ok(())
}

fn fallback_allowed(profile: &str) -> bool {
    matches!(profile, "debug" | "test")
}

fn frontend_dist_state(dist_path: &Path) -> io::Result<FrontendDistState> {
    if !dist_path.exists() {
        return Ok(FrontendDistState::Missing);
    }

    let index_path = dist_path.join("index.html");
    if !index_path.exists() {
        return Ok(FrontendDistState::Missing);
    }

    Ok(FrontendDistState::Real)
}

fn create_fallback_files(manifest_dir: &str, dist_path: &Path) -> io::Result<()> {
    if dist_path.exists() {
        fs::remove_dir_all(dist_path)?;
    }
    fs::create_dir_all(dist_path.join("assets"))?;

    let fallback_html = r#"<!doctype html>
<html lang="en"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<link rel="icon" href="/favicon.svg"><meta name="description" content="{{astergate_description}}"><title>{{astergate_title}}</title></head>
<body><main><h1>{{astergate_title}}</h1><p>Development backend. Run the frontend development server or build frontend-panel.</p></main></body></html>
"#;
    fs::write(dist_path.join("index.html"), fallback_html)?;
    fs::copy(
        Path::new(manifest_dir).join("frontend-panel/public/favicon.svg"),
        dist_path.join("favicon.svg"),
    )?;
    Ok(())
}
