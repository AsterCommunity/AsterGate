use crate::services::account::{AccountError, Result};

pub const DEFAULT_BRANDING_TITLE: &str = "AsterGate";
pub const DEFAULT_BRANDING_DESCRIPTION: &str = "Account and identity management";
macro_rules! display_text {
    ($name:ident, $key:literal, $max:literal, $default:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $name(String);
        impl $name {
            pub fn parse(value: &str) -> Result<Self> {
                let value = aster_forge_validation::display::normalize_bounded_display_text(
                    $key, value, $max,
                )
                .map_err(|_| AccountError::InvalidInput)?;
                Ok(Self(if value.is_empty() {
                    $default.into()
                } else {
                    value
                }))
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
        impl Default for $name {
            fn default() -> Self {
                Self($default.into())
            }
        }
    };
}
display_text!(BrandingTitle, "branding_title", 120, DEFAULT_BRANDING_TITLE);
display_text!(
    BrandingDescription,
    "branding_description",
    300,
    DEFAULT_BRANDING_DESCRIPTION
);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PublicBranding {
    pub title: BrandingTitle,
    pub description: BrandingDescription,
}
