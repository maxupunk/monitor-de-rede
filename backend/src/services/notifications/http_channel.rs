//! Envio HTTP compartilhado pelos canais de webhook (§8.9).

use std::sync::OnceLock;
use std::time::Duration;

use loco_rs::app::AppContext;

use super::contracts::{HttpChannelSpec, NotificationChannel, NotificationMessage};

/// Teto por entrega. Um webhook lento não pode segurar o ciclo do scheduler:
/// o alerta já está gravado, a notificação é o acessório.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Um cliente por processo — abrir um por notificação descarta o pool de
/// conexões e o handshake TLS a cada alerta.
fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .unwrap_or_default()
    })
}

/// Adapta qualquer [`HttpChannelSpec`] a um [`NotificationChannel`].
pub struct HttpNotificationChannel<S: HttpChannelSpec>(pub S);

#[async_trait::async_trait]
impl<S: HttpChannelSpec> NotificationChannel for HttpNotificationChannel<S> {
    fn name(&self) -> &str {
        self.0.name()
    }

    async fn send(&self, _ctx: &AppContext, message: &NotificationMessage) -> bool {
        if !self.0.is_configured() {
            return false;
        }
        let request = self.0.build_request(message);
        let mut builder = client().post(&request.url).json(&request.body);
        for (key, value) in &request.headers {
            builder = builder.header(key, value);
        }
        match builder.send().await {
            Ok(response) => {
                let ok = response.status().is_success();
                if !ok {
                    tracing::warn!(
                        channel = self.0.name(),
                        status = %response.status(),
                        "canal de notificação recusou a mensagem"
                    );
                }
                ok
            }
            Err(error) => {
                tracing::warn!(channel = self.0.name(), %error, "erro ao enviar notificação");
                false
            }
        }
    }
}
