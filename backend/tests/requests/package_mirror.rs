//! Espelho de pacotes do OpenWrt (`GET /api/package-mirror/openwrt/{token}/…`,
//! ADR 015 §10).
//!
//! Só o que recusa: nenhum teste sai da máquina, então a busca na origem fica
//! de fora — a política de caminhos e os tokens têm teste de unidade.

use backend::{app::App, services::package_mirror};
use loco_rs::testing::prelude::*;
use serial_test::serial;

const INDEX: &str = "releases/25.12.5/packages/aarch64_cortex-a53/base/packages.adb";

#[tokio::test]
#[serial]
async fn sem_login_mas_com_token_desconhecido_e_recusado() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, _ctx| async move {
        let response = request
            .get(&format!("/api/package-mirror/openwrt/inventado/{INDEX}"))
            .await;
        assert_eq!(response.status_code(), 403);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn token_valido_nao_abre_caminho_fora_do_repositorio() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, _ctx| async move {
        let token = package_mirror::issue();
        for path in [
            "releases/25.12.5/sha256sums",
            "outra-coisa/25.12.5/packages/x.apk",
            "releases/25.12.5/packages/..%2F..%2Fx.apk",
        ] {
            let response = request
                .get(&format!("/api/package-mirror/openwrt/{token}/{path}"))
                .await;
            assert_eq!(response.status_code(), 404, "{path}");
        }
    })
    .await;
}
