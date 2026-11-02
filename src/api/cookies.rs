//! Host-scoped Cookie policy and paired session/CSRF lifecycle after committed operations.
use crate::services::account::{AccountError, Result};
use axum::{
    http::{HeaderMap, header},
    response::Response,
};

#[derive(Clone)]
pub struct SessionCookiePolicy {
    secure: bool,
    lifetime: i64,
    authority: String,
}
impl SessionCookiePolicy {
    pub fn for_request(
        settings: &crate::services::system_settings::models::AuthenticationSettings,
        headers: &HeaderMap,
    ) -> Result<Self> {
        use crate::services::system_settings::site::PublicSiteOrigin;
        // Only configured authorities can determine cookie security. Forwarded headers are never trusted.
        let host_site = if let Some(host) = headers.get(header::HOST) {
            let host = host.to_str().map_err(|_| AccountError::Forbidden)?;
            Some(
                settings
                    .site_urls
                    .iter()
                    .find(|site| site.authority() == host)
                    .ok_or(AccountError::Forbidden)?,
            )
        } else {
            None
        };
        let origin_site = if let Some(origin) = headers.get(header::ORIGIN) {
            let origin = origin.to_str().map_err(|_| AccountError::Forbidden)?;
            Some(
                settings
                    .site_urls
                    .find_browser_origin(origin)
                    .ok_or(AccountError::Forbidden)?,
            )
        } else {
            None
        };
        if let (Some(host), Some(origin)) = (host_site, origin_site)
            && host != origin
        {
            return Err(AccountError::Forbidden);
        }
        let site: &PublicSiteOrigin = host_site
            .or(origin_site)
            .unwrap_or_else(|| settings.site_urls.primary());
        Ok(Self {
            secure: site.secure_cookies(),
            lifetime: settings.session_lifetime.seconds(),
            authority: site.authority().to_owned(),
        })
    }
    pub(crate) fn before_setup(
        headers: &HeaderMap,
        config: &crate::config::AppConfig,
    ) -> Result<Self> {
        use crate::services::system_settings::site::PublicSiteOrigin;
        let fallback = format!("{}:{}", config.server.host, config.server.port);
        let host = headers
            .get(header::HOST)
            .map(|value| value.to_str())
            .transpose()
            .map_err(|_| AccountError::Forbidden)?
            .unwrap_or(&fallback);
        // Browser-controlled source headers may disambiguate loopback schemes only for the
        // exact Host. They never select another authority or trust forwarded scheme/host.
        // Forge separately validates Origin, or Referer when Origin is absent, plus Fetch Metadata.
        let browser_site = [header::ORIGIN, header::REFERER]
            .into_iter()
            .find_map(|name| {
                let raw = headers.get(name)?.to_str().ok()?;
                let url = url::Url::parse(raw).ok()?;
                if !url.username().is_empty() || url.password().is_some() {
                    return None;
                }
                let site = PublicSiteOrigin::parse(&url.origin().ascii_serialization()).ok()?;
                (site.authority() == host).then_some(site)
            });
        // Source-less CLI bootstrap keeps HTTP loopback / HTTPS domain defaults. HTTPS
        // loopback clients must provide their same-host Origin or Referer, as browsers do.
        let site = browser_site
            .or_else(|| {
                PublicSiteOrigin::parse(&format!("http://{host}"))
                    .or_else(|_| PublicSiteOrigin::parse(&format!("https://{host}")))
                    .ok()
            })
            .ok_or(AccountError::Forbidden)?;
        if site.authority() != host {
            return Err(AccountError::Forbidden);
        }
        Ok(Self {
            secure: site.secure_cookies(),
            lifetime: 86400,
            authority: host.to_owned(),
        })
    }
    pub(crate) fn csrf_name(&self) -> &'static str {
        if self.secure {
            "__Host-ag_csrf"
        } else {
            "ag_csrf"
        }
    }
    pub(crate) fn source(&self) -> aster_forge_middleware::axum::csrf::RequestSourcePolicy {
        use aster_forge_middleware::axum::csrf::{RequestSourceMode, RequestSourcePolicy};
        RequestSourcePolicy {
            scheme: if self.secure { "https" } else { "http" }.into(),
            host: self.authority.clone(),
            public_site_origins: Vec::new(),
            mode: RequestSourceMode::OptionalWhenPresent,
        }
    }
    pub(crate) fn origin(&self) -> String {
        format!(
            "{}://{}",
            if self.secure { "https" } else { "http" },
            self.authority
        )
    }
    pub(crate) fn name(&self) -> &'static str {
        if self.secure {
            "__Host-ag_session"
        } else {
            "ag_session"
        }
    }
}

fn csrf_cookie(policy: &SessionCookiePolicy, value: &str) -> Result<axum::http::HeaderValue> {
    cookie::Cookie::build((policy.csrf_name(), value.to_string()))
        .path("/")
        .http_only(false)
        .secure(policy.secure)
        .same_site(cookie::SameSite::Strict)
        .max_age(cookie::time::Duration::seconds(if value.is_empty() {
            0
        } else {
            policy.lifetime
        }))
        .build()
        .to_string()
        .parse()
        .map_err(|_| AccountError::Security)
}

fn session_cookie(policy: &SessionCookiePolicy, value: &str) -> Result<axum::http::HeaderValue> {
    let max_age = if value.is_empty() { 0 } else { policy.lifetime };
    cookie::Cookie::build((policy.name(), value.to_string()))
        .path("/")
        .http_only(true)
        .secure(policy.secure)
        .same_site(cookie::SameSite::Lax)
        .max_age(cookie::time::Duration::seconds(max_age))
        .build()
        .to_string()
        .parse()
        .map_err(|_| AccountError::Security)
}

/// Issue both cookies only after the caller's session transaction has committed.
pub fn issue_session(
    response: &mut Response,
    policy: &SessionCookiePolicy,
    token: &str,
) -> Result<()> {
    let session = session_cookie(policy, token)?;
    let csrf = csrf_cookie(
        policy,
        &aster_forge_middleware::shared::csrf::build_csrf_token(),
    )?;
    response.headers_mut().append(header::SET_COOKIE, session);
    response.headers_mut().append(header::SET_COOKIE, csrf);
    Ok(())
}

/// Clear both host cookies after a successful operation invalidates this browser session.
pub fn clear_session(response: &mut Response, policy: &SessionCookiePolicy) -> Result<()> {
    let session = session_cookie(policy, "")?;
    let csrf = csrf_cookie(policy, "")?;
    response.headers_mut().append(header::SET_COOKIE, session);
    response.headers_mut().append(header::SET_COOKIE, csrf);
    Ok(())
}

/// Anonymous bootstrap preserves existing valid tokens; only its caller decides eligibility.
pub fn bootstrap_csrf(response: &mut Response, policy: &SessionCookiePolicy) -> Result<()> {
    response.headers_mut().append(
        header::SET_COOKIE,
        csrf_cookie(
            policy,
            &aster_forge_middleware::shared::csrf::build_csrf_token(),
        )?,
    );
    Ok(())
}
