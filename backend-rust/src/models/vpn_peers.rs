//! `VpnPeer` — telemetria do túnel e o estado derivado dela.
//!
//! A máquina de estados abaixo é porte **literal** de
//! `backend/app/models/vpn_peer.ts` (§8.10.3 do roadmap), comentários
//! inclusive. Os números vêm do protocolo WireGuard e da cadência real do
//! pipeline de coleta; mexer neles sem medir faz o status piscar na tela.

use chrono::Utc;
use sea_orm::{entity::prelude::*, QueryOrder};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub use super::_entities::vpn_peers::{ActiveModel, Column, Entity, Model};

use crate::services::shared::{crypto, errors::AppResult};

pub type VpnPeers = Entity;

/// `REJECT_AFTER_TIME` do WireGuard: o keypair atual deixa de ser aceito depois
/// disso. É o teto do intervalo entre handshakes de um túnel em uso — mas só de
/// um túnel *em uso*: sem dados para enviar, o protocolo não renegocia nada.
pub const REJECT_AFTER_SECONDS: i64 = 180;

/// `KEEPALIVE_TIMEOUT` do WireGuard: quem recebe um pacote e não tem nada a
/// devolver responde com um keepalive vazio depois desse tempo.
pub const KEEPALIVE_TIMEOUT_SECONDS: i64 = 10;

/// Folga do caminho até o banco: o watcher republica `wg show dump` a cada 5s e
/// o scheduler sincroniza a cada 10s. Sem essa margem, um peer saudável cruzava
/// o limite só por causa da latência da coleta.
pub const STATUS_PIPELINE_SLACK_SECONDS: i64 = 45;

/// Keepalives que podem faltar antes de o túnel virar suspeito.
pub const KEEPALIVE_MISSES_ALLOWED: i64 = 3;

/// Janela usada quando não há keepalive contabilizado — resta olhar o handshake.
pub const HANDSHAKE_CONNECTED_SECONDS: i64 = REJECT_AFTER_SECONDS + STATUS_PIPELINE_SLACK_SECONDS;

/// Sem keepalive o silêncio é ambíguo: um túnel ocioso é indistinguível de um
/// túnel morto, porque o WireGuard só renegocia quando há o que enviar. Por isso
/// a janela até "caído" é generosa aqui — é o mesmo valor adotado pelo wg-easy.
pub const HANDSHAKE_DISCONNECTED_SECONDS: i64 = 600;

/// Cadência real com que o RX cresce num peer com keepalive.
///
/// Não é o `PersistentKeepalive` puro: o valor no dump é o intervalo do
/// *servidor*, e o que faz o RX subir é a resposta do peer — um keepalive
/// passivo emitido `KEEPALIVE_TIMEOUT` depois. Usar só o keepalive subestimava
/// o intervalo e a janela de "conectado" valia ~2,4 perdas em vez de 3.
#[must_use]
pub fn effective_keepalive_seconds(persistent_keepalive: i64) -> i64 {
    persistent_keepalive + KEEPALIVE_TIMEOUT_SECONDS
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum VpnPeerConnectionStatus {
    Connected,
    Unstable,
    Disconnected,
    /// Nunca deu sinal: o peer foi criado mas o cliente ainda não subiu.
    Awaiting,
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(self, _db: &C, insert: bool) -> std::result::Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if !insert && self.updated_at.is_unchanged() {
            let mut this = self;
            this.updated_at = sea_orm::ActiveValue::Set(chrono::Utc::now().into());
            Ok(this)
        } else {
            Ok(self)
        }
    }
}

impl Model {
    /// Chave pré-compartilhada em texto claro.
    ///
    /// A coluna guarda o criptograma; o valor claro só existe em memória, na
    /// hora de montar o `wg0.conf`. **Nunca** serializar isto num DTO.
    ///
    /// # Errors
    ///
    /// Falha se a `APP_KEY` mudou depois da gravação ou o dado foi adulterado.
    pub fn preshared_key(&self) -> AppResult<Option<String>> {
        self.preshared_key_encrypted
            .as_deref()
            .map(crypto::decrypt)
            .transpose()
    }

    /// Sinal de vida mais recente: keepalive contabilizado ou renegociação de
    /// chaves, o que tiver acontecido por último.
    #[must_use]
    pub fn last_activity_at(&self) -> Option<DateTimeWithTimeZone> {
        match (self.last_handshake_at, self.last_seen_at) {
            (None, seen) => seen,
            (handshake, None) => handshake,
            (Some(handshake), Some(seen)) => Some(if seen > handshake { seen } else { handshake }),
        }
    }

    /// Há keepalive contabilizado, ou só resta o handshake como régua?
    #[must_use]
    pub fn has_keepalive_heartbeat(&self) -> bool {
        self.persistent_keepalive > 0 && self.last_seen_at.is_some()
    }

    /// Quanto tempo sem sinal ainda conta como conectado.
    ///
    /// Com keepalive ativo a régua é o próprio keepalive: três perdas seguidas mais
    /// a folga da coleta. Sem ele — peer que só fala quando tem tráfego — resta o
    /// handshake, cuja janela precisa ser bem mais larga.
    #[must_use]
    pub fn connected_window_seconds(&self) -> i64 {
        if self.has_keepalive_heartbeat() {
            effective_keepalive_seconds(i64::from(self.persistent_keepalive))
                * KEEPALIVE_MISSES_ALLOWED
                + STATUS_PIPELINE_SLACK_SECONDS
        } else {
            HANDSHAKE_CONNECTED_SECONDS
        }
    }

    /// Onde "instável" acaba e "caído" começa.
    ///
    /// Com keepalive existe batimento previsível, então o dobro da janela de
    /// conectado já é diagnóstico — não há motivo para esperar 15 minutos. Sem
    /// keepalive o silêncio não prova nada e a régua tem que ser generosa.
    #[must_use]
    pub fn disconnected_window_seconds(&self) -> i64 {
        if self.has_keepalive_heartbeat() {
            self.connected_window_seconds() * 2
        } else {
            HANDSHAKE_DISCONNECTED_SECONDS
        }
    }

    /// Janela curta para afirmar que o túnel está de pé **agora**.
    ///
    /// `connection_status` é tolerante de propósito, para o status não piscar com
    /// uma perda isolada de keepalive. Mas um diagnóstico como "o túnel está de
    /// pé, o bloqueio é do ICMP" precisa de prova recente, não de tolerância: um
    /// único batimento perdido já basta para calar o aviso.
    #[must_use]
    pub fn proof_of_life_window_seconds(&self) -> i64 {
        if self.has_keepalive_heartbeat() {
            effective_keepalive_seconds(i64::from(self.persistent_keepalive))
                + STATUS_PIPELINE_SLACK_SECONDS
        } else {
            HANDSHAKE_CONNECTED_SECONDS
        }
    }

    /// O túnel deu sinal de vida recente o bastante para sustentar um diagnóstico.
    #[must_use]
    pub fn has_fresh_proof_of_life(&self) -> bool {
        self.last_activity_at().is_some_and(|last_activity| {
            (Utc::now() - last_activity.to_utc()).num_seconds()
                <= self.proof_of_life_window_seconds()
        })
    }

    /// §6.1 — `connectionStatus`.
    #[must_use]
    pub fn connection_status(&self) -> VpnPeerConnectionStatus {
        let Some(last_activity) = self.last_activity_at() else {
            return VpnPeerConnectionStatus::Awaiting;
        };

        let elapsed_seconds = (Utc::now() - last_activity.to_utc()).num_seconds();
        if elapsed_seconds <= self.connected_window_seconds() {
            VpnPeerConnectionStatus::Connected
        } else if elapsed_seconds <= self.disconnected_window_seconds() {
            VpnPeerConnectionStatus::Unstable
        } else {
            VpnPeerConnectionStatus::Disconnected
        }
    }
}

impl ActiveModel {
    /// Grava a chave pré-compartilhada já cifrada.
    ///
    /// # Errors
    ///
    /// Falha se a cifra não conseguir operar.
    pub fn set_preshared_key(&mut self, plain: Option<&str>) -> AppResult<()> {
        self.preshared_key_encrypted = sea_orm::ActiveValue::Set(match plain {
            Some(value) => Some(crypto::encrypt(value)?),
            None => None,
        });
        Ok(())
    }
}

impl Entity {
    /// Peers ativos de um servidor, em ordem estável — lido a cada ciclo de
    /// sincronia do WireGuard. Desenhada para `vpn_peers_server_enabled_index`.
    pub fn find_enabled_for_server(vpn_server_id: i64) -> Select<Entity> {
        Entity::find()
            .filter(Column::VpnServerId.eq(vpn_server_id))
            .filter(Column::Enabled.eq(true))
            .order_by_asc(Column::Id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn peer(persistent_keepalive: i32, seen_ago: Option<i64>, handshake_ago: Option<i64>) -> Model {
        let now = Utc::now();
        Model {
            id: 1,
            vpn_server_id: 1,
            device_id: 1,
            public_key: "chave".to_string(),
            preshared_key_encrypted: None,
            device_profile: "linux".to_string(),
            persistent_keepalive,
            last_handshake_at: handshake_ago.map(|s| (now - Duration::seconds(s)).into()),
            last_seen_at: seen_ago.map(|s| (now - Duration::seconds(s)).into()),
            bytes_rx: 0,
            bytes_tx: 0,
            enabled: true,
            last_connection_status: None,
            created_at: now.into(),
            updated_at: now.into(),
        }
    }

    #[test]
    fn sem_nenhum_sinal_o_peer_esta_aguardando() {
        assert_eq!(
            peer(25, None, None).connection_status(),
            VpnPeerConnectionStatus::Awaiting
        );
    }

    #[test]
    fn com_keepalive_a_janela_vale_tres_perdas_mais_a_folga() {
        // (25 + 10) * 3 + 45 = 150 s de tolerância.
        let p = peer(25, Some(0), None);
        assert_eq!(p.connected_window_seconds(), 150);
        assert_eq!(p.disconnected_window_seconds(), 300);

        assert_eq!(
            peer(25, Some(149), None).connection_status(),
            VpnPeerConnectionStatus::Connected
        );
        assert_eq!(
            peer(25, Some(200), None).connection_status(),
            VpnPeerConnectionStatus::Unstable
        );
        assert_eq!(
            peer(25, Some(400), None).connection_status(),
            VpnPeerConnectionStatus::Disconnected
        );
    }

    #[test]
    fn sem_keepalive_a_regua_e_o_handshake() {
        // 180 + 45 = 225 s conectado; 600 s até cair.
        let p = peer(0, None, Some(0));
        assert!(!p.has_keepalive_heartbeat());
        assert_eq!(p.connected_window_seconds(), HANDSHAKE_CONNECTED_SECONDS);
        assert_eq!(
            p.disconnected_window_seconds(),
            HANDSHAKE_DISCONNECTED_SECONDS
        );

        assert_eq!(
            peer(0, None, Some(300)).connection_status(),
            VpnPeerConnectionStatus::Unstable
        );
        assert_eq!(
            peer(0, None, Some(700)).connection_status(),
            VpnPeerConnectionStatus::Disconnected
        );
    }

    #[test]
    fn keepalive_configurado_mas_sem_last_seen_cai_na_regua_do_handshake() {
        // `persistentKeepalive > 0` sozinho não basta: sem `last_seen_at` não
        // houve batimento contabilizado.
        let p = peer(25, None, Some(0));
        assert!(!p.has_keepalive_heartbeat());
        assert_eq!(p.connected_window_seconds(), HANDSHAKE_CONNECTED_SECONDS);
    }

    #[test]
    fn last_activity_pega_o_mais_recente_dos_dois() {
        let p = peer(25, Some(10), Some(120));
        let elapsed = (Utc::now() - p.last_activity_at().unwrap().to_utc()).num_seconds();
        assert!(elapsed <= 11, "devia ter usado o last_seen_at (10 s atrás)");
    }

    #[test]
    fn prova_de_vida_e_mais_exigente_que_o_status() {
        // 100 s: dentro da janela de "conectado" (150 s), fora da de prova de
        // vida (25 + 10 + 45 = 80 s).
        let p = peer(25, Some(100), None);
        assert_eq!(p.connection_status(), VpnPeerConnectionStatus::Connected);
        assert!(!p.has_fresh_proof_of_life());

        assert!(peer(25, Some(30), None).has_fresh_proof_of_life());
    }

    #[test]
    fn status_serializa_em_minusculas_para_o_frontend() {
        assert_eq!(
            serde_json::to_value(VpnPeerConnectionStatus::Disconnected).unwrap(),
            serde_json::json!("disconnected")
        );
    }
}
