use crate::services::account::{AccountError, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicSiteOrigin(url::Url);
impl PublicSiteOrigin {
    pub fn parse(value: &str) -> Result<Self> {
        let value = value.trim();
        if value.len() > 2048 || value.chars().any(|c| c.is_control() || c.is_whitespace()) {
            return Err(AccountError::InvalidInput);
        }
        let url = url::Url::parse(value).map_err(|_| AccountError::InvalidInput)?;
        if !["http", "https"].contains(&url.scheme())
            || url.host_str().is_none()
            || url.host_str().is_some_and(|host| host.contains('*'))
            || !url.username().is_empty()
            || url.password().is_some()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
            || (url.scheme() == "http"
                && !matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")))
        {
            return Err(AccountError::InvalidInput);
        }
        Ok(Self(url))
    }
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
    pub fn browser_origin(&self) -> String {
        self.0.origin().ascii_serialization()
    }
    pub fn secure_cookies(&self) -> bool {
        self.0.scheme() == "https"
    }
    pub fn authority(&self) -> &str {
        &self.0[url::Position::BeforeHost..url::Position::AfterPort]
    }
    pub fn link(&self, path: &str, token: &str) -> String {
        let mut link = self.0.clone();
        link.set_path(path);
        link.set_fragment(Some(&format!("token={token}")));
        link.to_string()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicSiteUrls(Vec<PublicSiteOrigin>);
impl PublicSiteUrls {
    pub const MAX: usize = 16;
    pub fn parse(values: &[String]) -> Result<Self> {
        if values.is_empty() || values.len() > Self::MAX {
            return Err(AccountError::InvalidInput);
        }
        let mut origins = Vec::new();
        for value in values {
            let origin = PublicSiteOrigin::parse(value)?;
            if !origins.contains(&origin) {
                origins.push(origin);
            }
        }
        // The request Host must unambiguously determine cookie security on reads.
        if origins.iter().any(|a| {
            origins
                .iter()
                .any(|b| a != b && a.authority() == b.authority())
        }) {
            return Err(AccountError::InvalidInput);
        }
        Ok(Self(origins))
    }
    pub fn contains(&self, origin: &PublicSiteOrigin) -> bool {
        self.0.contains(origin)
    }
    pub fn primary(&self) -> &PublicSiteOrigin {
        &self.0[0]
    }
    pub fn iter(&self) -> impl Iterator<Item = &PublicSiteOrigin> {
        self.0.iter()
    }
    pub fn values(&self) -> Vec<String> {
        self.iter().map(PublicSiteOrigin::browser_origin).collect()
    }
    pub fn find_browser_origin(&self, value: &str) -> Option<&PublicSiteOrigin> {
        self.iter().find(|origin| origin.browser_origin() == value)
    }
}
