//! Entrega de uma notificação a todos os canais configurados (§8.9).

use loco_rs::app::AppContext;

use super::{
    channels::{
        discord::DiscordChannel, email::EmailChannel, telegram::TelegramChannel,
        webhook::WebhookChannel,
    },
    contracts::{NotificationChannel, NotificationMessage},
    http_channel::HttpNotificationChannel,
};

/// Despachante de notificações.
///
/// Os canais são montados a partir do ambiente. Quem não tiver token/URL
/// responde `false` sem tentar, então uma instalação que só usa Telegram não
/// paga nada pelos outros três.
pub struct NotificationService {
    channels: Vec<Box<dyn NotificationChannel>>,
}

impl Default for NotificationService {
    fn default() -> Self {
        Self::with_default_channels()
    }
}

impl NotificationService {
    /// Os quatro canais do §8.9, na ordem em que o backend anterior os
    /// registrava.
    #[must_use]
    pub fn with_default_channels() -> Self {
        Self {
            channels: vec![
                Box::new(EmailChannel::from_env()),
                Box::new(HttpNotificationChannel(TelegramChannel::from_env())),
                Box::new(HttpNotificationChannel(DiscordChannel::from_env())),
                Box::new(HttpNotificationChannel(WebhookChannel::from_env())),
            ],
        }
    }

    /// Serviço sem canal algum — base para testes e para quem quer registrar
    /// só o que interessa.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            channels: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_channel(mut self, channel: Box<dyn NotificationChannel>) -> Self {
        self.channels.push(channel);
        self
    }

    /// Entrega a mensagem a todos os canais.
    ///
    /// Não devolve erro: a falha de um canal **nunca** propaga (§8.9). O alerta
    /// já está gravado no banco quando chegamos aqui — deixar a exceção subir
    /// desfaria o trabalho útil por causa do acessório.
    pub async fn notify(&self, ctx: &AppContext, message: &NotificationMessage) {
        for channel in &self.channels {
            let delivered = channel.send(ctx, message).await;
            tracing::debug!(
                channel = channel.name(),
                delivered,
                title = %message.title,
                "notificação processada"
            );
        }
    }
}
