pub mod branding;
pub mod mail;
pub mod models;
pub mod policy;
pub mod site;
pub mod synchronization;

pub fn smtp_sender(
    runtime_config: std::sync::Arc<crate::config::runtime_config::RuntimeConfig>,
) -> std::sync::Arc<dyn aster_forge_mail::MailSender> {
    std::sync::Arc::new(RuntimeSmtpSender { runtime_config })
}
struct RuntimeSmtpSender {
    runtime_config: std::sync::Arc<crate::config::runtime_config::RuntimeConfig>,
}
#[async_trait::async_trait]
impl aster_forge_mail::MailSender for RuntimeSmtpSender {
    async fn send(
        &self,
        message: aster_forge_mail::MailMessage,
    ) -> aster_forge_mail::MailSendResult<()> {
        let settings = self.runtime_config.mail().map_err(|_| {
            aster_forge_mail::MailDeliveryError::NotConfigured(
                "system mail settings unavailable".into(),
            )
        })?;
        let smtp = settings.runtime();
        aster_forge_mail::smtp_sender(move || smtp.clone())
            .send(message)
            .await
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
