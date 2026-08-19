//! O checker do tipo `system_health`.
//!
//! É deliberadamente magro: toda a coleta vive em
//! [`crate::services::monitoring::health`], e aqui só se veste o resultado no
//! contrato [`Checker`] que o runner conhece. Sem isso o tipo teria de ser um
//! caso especial dentro do runner, e um caso especial no runner é o começo de
//! um segundo pipeline.
//!
//! O checker **não** faz rede: é a única checagem do produto cujo alvo é a
//! máquina onde ela roda. Por isso nenhum teste seu precisa de alvo externo.

use serde::{Deserialize, Serialize};

use crate::services::monitoring::{
    contracts::{CheckResult, Checker},
    health::HealthCoordinator,
};

/// Configuração do monitor gerenciado.
///
/// Vazia de propósito, e não ausente: o campo `configuration` de `monitors` é
/// `NOT NULL`, e um objeto vazio deixa espaço para uma opção futura sem trocar
/// o tipo da coluna. Campos desconhecidos são ignorados para que uma
/// configuração antiga nunca derrube a coleta.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SystemHealthConfig {}

pub struct SystemHealthChecker;

#[async_trait::async_trait]
impl Checker for SystemHealthChecker {
    type Config = SystemHealthConfig;

    async fn execute(&self, _config: Self::Config) -> CheckResult {
        // As fontes de produção guardam a amostra anterior de CPU e de
        // tráfego, então o coordenador precisa sobreviver entre ciclos: um
        // coordenador novo a cada execução reportaria "primeira amostra" para
        // sempre.
        coordinator().collect()
    }
}

/// O coordenador do processo.
///
/// Estado de processo, e não de banco: o que ele guarda são duas leituras de
/// contador acumulado, cujo único uso é a próxima subtração. Persistir isso
/// custaria uma tabela para economizar uma amostra por reinício.
fn coordinator() -> &'static HealthCoordinator {
    static COORDINATOR: std::sync::OnceLock<HealthCoordinator> = std::sync::OnceLock::new();
    COORDINATOR.get_or_init(HealthCoordinator::with_default_sources)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_coleta_nunca_falha_e_sempre_descreve_o_que_conseguiu() {
        // Roda contra o sistema real da suíte — Linux em CI, Windows na
        // máquina do desenvolvedor. Nos dois o contrato é o mesmo: um
        // resultado utilizável, com métricas ou com motivos.
        let resultado = SystemHealthChecker
            .execute(SystemHealthConfig::default())
            .await;

        assert!(
            resultado.message.is_some(),
            "um resultado sem mensagem não diz nada ao operador"
        );
        assert!(resultado.data["sources"].is_object());
        assert!(resultado.data["unavailable"].is_object());
        assert!(
            resultado.duration_ms >= 0,
            "duração negativa quebraria o clamp do process_result"
        );
        // Toda métrica publicada leva unidade explícita.
        for metrica in &resultado.metrics {
            assert!(!metrica.unit.is_empty(), "{} sem unidade", metrica.name);
            assert!(metrica.value.is_finite(), "{} não finito", metrica.name);
        }
    }
}
