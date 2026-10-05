//! Registro de fabricantes do IEEE: atualização, consulta pela API e a regra
//! de "ou atualiza tudo, ou nada muda".
//!
//! As fontes são um servidor HTTP em 127.0.0.1 servindo CSV no formato do
//! IEEE — nenhum teste sai da máquina.

use backend::{
    app::App,
    services::vendors::{
        registry::{self, OuiRegistry},
        service,
    },
};
use loco_rs::testing::prelude::*;
use serial_test::serial;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

use super::prepare_data;

const MA_L: &str = "Registry,Assignment,Organization Name,Organization Address\n\
MA-L,5CCF7F,Espressif Inc.,\"Shanghai, CN\"\n\
MA-L,C056E3,\"Hangzhou Hikvision Digital Technology Co.,Ltd.\",Hangzhou CN\n";
const MA_S: &str = "Registry,Assignment,Organization Name,Organization Address\n\
MA-S,70B3D5123,Sensor Industrial GmbH,Berlin DE\n";

/// Servidor que responde `/ma-l.csv` e `/ma-s.csv`; o resto é 404.
async fn registry_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let mut buffer = [0_u8; 1_024];
            let read = socket.read(&mut buffer).await.unwrap_or(0);
            let request = String::from_utf8_lossy(&buffer[..read]);
            let body = if request.starts_with("GET /ma-l.csv") {
                Some(MA_L)
            } else if request.starts_with("GET /ma-s.csv") {
                Some(MA_S)
            } else {
                None
            };
            let response = match body {
                Some(body) => format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/csv\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                ),
                None => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    .to_string(),
            };
            let _ = socket.write_all(response.as_bytes()).await;
        }
    });
    format!("http://{address}")
}

#[tokio::test]
#[serial]
async fn atualiza_o_registro_e_a_consulta_usa_o_bloco_mais_especifico() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);
        let base = registry_server().await;

        let outcome = service::refresh(
            &ctx.db,
            &[format!("{base}/ma-l.csv"), format!("{base}/ma-s.csv")],
        )
        .await
        .unwrap();
        assert_eq!(outcome.entries, 3);
        assert_eq!(outcome.by_registry["MA-L"], 2);
        assert_eq!(outcome.by_registry["MA-S"], 1);

        let status = request.get("/api/vendors/status").await;
        assert_eq!(status.status_code(), 200, "{}", status.text());
        let status: serde_json::Value = serde_json::from_str(&status.text()).unwrap();
        assert_eq!(status["entries"], 3);
        assert_eq!(status["stale"], false);
        assert!(status["builtinEntries"].as_u64().unwrap() > 500);

        let found = request
            .get("/api/vendors/lookup?mac=5c:cf:7f:00:11:22")
            .await;
        let found: serde_json::Value = serde_json::from_str(&found.text()).unwrap();
        assert_eq!(found["vendor"], "Espressif");
        assert_eq!(found["organization"], "Espressif Inc.");
        assert_eq!(found["source"], "ieee");
        assert_eq!(found["hint"]["deviceType"], "iot");

        let camera = request
            .get("/api/vendors/lookup?mac=C0-56-E3-AA-BB-CC")
            .await;
        let camera: serde_json::Value = serde_json::from_str(&camera.text()).unwrap();
        assert_eq!(camera["vendor"], "Hangzhou Hikvision Digital Technology");
        assert_eq!(camera["hint"]["deviceType"], "camera");

        let small_block = request
            .get("/api/vendors/lookup?mac=70:b3:d5:12:3f:ff")
            .await;
        let small_block: serde_json::Value = serde_json::from_str(&small_block.text()).unwrap();
        assert_eq!(small_block["vendor"], "Sensor Industrial");

        let invalid = request.get("/api/vendors/lookup?mac=5c").await;
        assert_eq!(invalid.status_code(), 422, "{}", invalid.text());

        registry::install(OuiRegistry::default());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn fonte_que_falha_nao_apaga_o_registro_anterior() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let base = registry_server().await;
        service::refresh(&ctx.db, &[format!("{base}/ma-l.csv")])
            .await
            .unwrap();

        let error = service::refresh(
            &ctx.db,
            &[format!("{base}/ma-l.csv"), format!("{base}/sumiu.csv")],
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("404"), "{error}");

        let status = service::status(&ctx.db).await.unwrap();
        assert_eq!(status.entries, 2, "a tabela anterior continua valendo");
        assert_eq!(service::load(&ctx.db).await.unwrap(), 2);

        registry::install(OuiRegistry::default());
    })
    .await;
}
