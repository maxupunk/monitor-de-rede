//! Estado ligado/desligado das áreas e o serviço que o altera.

use sea_orm::{ConnectionTrait, DatabaseConnection};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{Area, AreaPolicy, Blocker, Consequence, EnabledAreas, Evidence};
use crate::{
    events::EventBus,
    models::system_settings,
    shared::errors::{AppError, AppResult},
};

/// Chave em `system_settings`. JSON `{"network": true, "servers": false, …}`.
pub const STORAGE_KEY: &str = "areas";

/// Tópico SSE com o estado de todas as áreas; vai também no snapshot de quem
/// conecta (AGENTS §9 — sem polling).
pub const EVENT: &str = "areas:updated";

/// Chave do snapshot no barramento.
const SNAPSHOT_KEY: &str = "areas";

/// Lê o estado. Chave ausente ou JSON ilegível valem "tudo ligado": um valor
/// corrompido não pode esconder a tela de ninguém.
///
/// Rotinas de fundo chamam isto **a cada ciclo**, sem cache: o agendador pode
/// rodar em outro processo (`task scheduler_loop`), e um cache em memória ali
/// continuaria vendo a área ligada depois de desligada.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn load<C: ConnectionTrait>(db: &C) -> AppResult<EnabledAreas> {
    Ok(system_settings::Model::get(db, STORAGE_KEY)
        .await?
        .and_then(|row| row.value)
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default())
}

/// Atalho para "a área está ligada?".
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn is_enabled<C: ConnectionTrait>(db: &C, area: Area) -> AppResult<bool> {
    if !area.is_optional() {
        return Ok(true);
    }
    Ok(load(db).await?.is_enabled(area))
}

/// Grava o estado.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn save<C: ConnectionTrait>(db: &C, enabled: &EnabledAreas) -> AppResult<()> {
    let text = serde_json::to_string(enabled)
        .map_err(|error| AppError::Internal(anyhow::Error::new(error)))?;
    system_settings::Model::set(db, STORAGE_KEY, Some(text)).await?;
    Ok(())
}

/// Uma área como a tela a vê.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AreaState {
    pub area: Area,
    pub title: String,
    pub summary: String,
    pub icon: String,
    pub enabled: bool,
    /// `false` para o núcleo.
    pub optional: bool,
    /// Sinal persistido de uso (para as sugestões). O núcleo não tem.
    pub evidence: Option<Evidence>,
}

/// O estado de todas as áreas — corpo do `GET /api/areas` e do `areas:updated`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AreasSnapshot {
    pub areas: Vec<AreaState>,
}

impl AreasSnapshot {
    #[must_use]
    pub fn is_enabled(&self, area: Area) -> bool {
        self.areas
            .iter()
            .find(|state| state.area == area)
            .is_none_or(|state| state.enabled)
    }
}

/// O que acontece ao desligar uma área — para a confirmação na tela.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AreaImpact {
    pub area: Area,
    pub consequences: Vec<Consequence>,
    /// Presente, o desligamento será recusado.
    pub blocker: Option<Blocker>,
}

/// Monta o snapshot a partir do estado gravado e das políticas registradas.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn snapshot(
    db: &DatabaseConnection,
    policies: &[Box<dyn AreaPolicy>],
) -> AppResult<AreasSnapshot> {
    let enabled = load(db).await?;
    let mut areas = Vec::with_capacity(Area::ALL.len());
    for area in Area::ALL {
        let evidence = match policies.iter().find(|policy| policy.area() == area) {
            Some(policy) => Some(policy.evidence(db).await?),
            None => None,
        };
        areas.push(AreaState {
            area,
            title: area.title().to_owned(),
            summary: area.summary().to_owned(),
            icon: area.icon().to_owned(),
            enabled: enabled.is_enabled(area),
            optional: area.is_optional(),
            evidence,
        });
    }
    Ok(AreasSnapshot { areas })
}

/// Publica o estado no SSE e o guarda como snapshot para quem conectar depois.
pub fn publish(bus: &EventBus, snapshot: &AreasSnapshot) {
    match serde_json::to_value(snapshot) {
        Ok(payload) => {
            bus.publish_snapshot(EVENT, SNAPSHOT_KEY, payload);
        }
        Err(error) => tracing::warn!(%error, "não foi possível serializar o estado das áreas"),
    }
}

/// Lê e altera as áreas, validando com as políticas registradas.
pub struct AreaService<'a> {
    db: &'a DatabaseConnection,
    policies: &'a [Box<dyn AreaPolicy>],
}

impl<'a> AreaService<'a> {
    #[must_use]
    pub fn new(db: &'a DatabaseConnection, policies: &'a [Box<dyn AreaPolicy>]) -> Self {
        Self { db, policies }
    }

    fn policy(&self, area: Area) -> Option<&dyn AreaPolicy> {
        self.policies
            .iter()
            .find(|policy| policy.area() == area)
            .map(AsRef::as_ref)
    }

    /// # Errors
    ///
    /// Propaga erro do banco.
    pub async fn snapshot(&self) -> AppResult<AreasSnapshot> {
        snapshot(self.db, self.policies).await
    }

    /// O impacto de desligar a área, sem desligá-la.
    ///
    /// # Errors
    ///
    /// Área obrigatória, ou erro do banco.
    pub async fn impact(&self, area: Area) -> AppResult<AreaImpact> {
        if !area.is_optional() {
            return Err(AppError::validation("O núcleo não pode ser desligado."));
        }
        let (consequences, blocker) = match self.policy(area) {
            Some(policy) => (
                policy.consequences(self.db).await?,
                policy.blocker(self.db).await?,
            ),
            None => (Vec::new(), None),
        };
        Ok(AreaImpact {
            area,
            consequences,
            blocker,
        })
    }

    /// Liga ou desliga a área e devolve o novo estado. Não publica: quem chama
    /// decide (o controller publica e audita).
    ///
    /// # Errors
    ///
    /// Núcleo, desligamento bloqueado pela área (409), ou erro do banco.
    pub async fn set(&self, area: Area, enabled: bool) -> AppResult<AreasSnapshot> {
        if !area.is_optional() {
            return Err(AppError::validation("O núcleo não pode ser desligado."));
        }
        if !enabled {
            if let Some(policy) = self.policy(area) {
                if let Some(blocker) = policy.blocker(self.db).await? {
                    return Err(AppError::conflict(blocker.message));
                }
            }
        }
        let mut state = load(self.db).await?;
        state.set(area, enabled);
        save(self.db, &state).await?;
        self.snapshot().await
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use migration::{Migrator, MigratorTrait};
    use sea_orm::Database;

    use super::*;

    async fn banco() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.expect("banco");
        Migrator::up(&db, None).await.expect("migrations");
        db
    }

    struct VpnAtiva;

    #[async_trait]
    impl AreaPolicy for VpnAtiva {
        fn area(&self) -> Area {
            Area::Vpn
        }
        async fn evidence(&self, _db: &DatabaseConnection) -> AppResult<Evidence> {
            Ok(Evidence::because("1 servidor VPN"))
        }
        async fn consequences(&self, _db: &DatabaseConnection) -> AppResult<Vec<Consequence>> {
            Ok(vec![Consequence::new("O menu VPN some.")])
        }
        async fn blocker(&self, _db: &DatabaseConnection) -> AppResult<Option<Blocker>> {
            Ok(Some(Blocker {
                message: "Desative o servidor VPN antes.".to_owned(),
            }))
        }
    }

    fn politicas() -> Vec<Box<dyn AreaPolicy>> {
        vec![Box::new(VpnAtiva)]
    }

    #[tokio::test]
    async fn banco_novo_tem_tudo_ligado() {
        let db = banco().await;
        let estado = load(&db).await.expect("ler");
        assert!(Area::ALL.iter().all(|area| estado.is_enabled(*area)));
    }

    #[tokio::test]
    async fn json_corrompido_vale_tudo_ligado() {
        let db = banco().await;
        system_settings::Model::set(&db, STORAGE_KEY, Some("{nada".to_owned()))
            .await
            .expect("gravar");
        assert!(is_enabled(&db, Area::Servers).await.expect("ler"));
    }

    #[tokio::test]
    async fn desligar_e_religar_volta_ao_que_era() {
        let db = banco().await;
        let policies = politicas();
        let service = AreaService::new(&db, &policies);
        let estado = service.set(Area::Servers, false).await.expect("desligar");
        assert!(!estado.is_enabled(Area::Servers));
        assert!(estado.is_enabled(Area::Network), "as outras não mudam");
        assert!(!is_enabled(&db, Area::Servers).await.expect("ler"));

        let estado = service.set(Area::Servers, true).await.expect("religar");
        assert!(estado.is_enabled(Area::Servers));
    }

    #[tokio::test]
    async fn nucleo_nao_desliga() {
        let db = banco().await;
        let policies = politicas();
        let erro = AreaService::new(&db, &policies)
            .set(Area::Core, false)
            .await
            .expect_err("recusa");
        assert!(matches!(erro, AppError::Validation(_)));
        assert!(is_enabled(&db, Area::Core).await.expect("ler"));
    }

    #[tokio::test]
    async fn bloqueio_da_area_recusa_desligar_mas_nao_ligar() {
        let db = banco().await;
        let policies = politicas();
        let service = AreaService::new(&db, &policies);
        let erro = service.set(Area::Vpn, false).await.expect_err("recusa");
        assert!(matches!(erro, AppError::Conflict(_)));
        assert!(
            is_enabled(&db, Area::Vpn).await.expect("ler"),
            "nada gravado"
        );
        service.set(Area::Vpn, true).await.expect("ligar é livre");
    }

    #[tokio::test]
    async fn snapshot_traz_evidencia_so_de_quem_tem_politica() {
        let db = banco().await;
        let policies = politicas();
        let snapshot = AreaService::new(&db, &policies)
            .snapshot()
            .await
            .expect("snapshot");
        let vpn = snapshot
            .areas
            .iter()
            .find(|s| s.area == Area::Vpn)
            .expect("vpn");
        assert_eq!(vpn.evidence, Some(Evidence::because("1 servidor VPN")));
        let core = snapshot
            .areas
            .iter()
            .find(|s| s.area == Area::Core)
            .expect("core");
        assert!(!core.optional);
        assert!(core.evidence.is_none());
    }

    #[tokio::test]
    async fn impacto_lista_consequencias_e_bloqueio() {
        let db = banco().await;
        let policies = politicas();
        let impacto = AreaService::new(&db, &policies)
            .impact(Area::Vpn)
            .await
            .expect("impacto");
        assert_eq!(impacto.consequences.len(), 1);
        assert!(impacto.blocker.is_some());
    }

    #[test]
    fn area_ausente_no_json_vale_ligada() {
        let estado: EnabledAreas = serde_json::from_str(r#"{"servers": false}"#).expect("json");
        assert!(!estado.is_enabled(Area::Servers));
        assert!(estado.is_enabled(Area::Network));
        assert!(estado.is_enabled(Area::Core));
    }
}
