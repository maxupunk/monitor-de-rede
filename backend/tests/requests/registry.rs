//! O registry de imagens pela API (ADR 022): o token pessoal só vale pela rede
//! interna, o fluxo de token do Docker entrega o escopo mínimo, o envio em
//! partes confere o digest, a tag de commit não muda de conteúdo e o pull
//! respeita `Range`.

use backend::app::App;
use base64::Engine;
use loco_rs::testing::prelude::*;
use netmonitor_core::models::users;
use sea_orm::{ActiveModelTrait, IntoActiveModel, Set};
use serde_json::{json, Value};
use serial_test::serial;
use sha2::{Digest, Sha256};

use super::prepare_data;

const INTERNAL: &str = "10.8.0.5";
const PUBLIC: &str = "203.0.113.9";

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn body(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or(Value::Null)
}

#[tokio::test]
#[serial]
async fn push_e_pull_pelo_fluxo_do_docker() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, ctx| async move {
        let session = prepare_data::init_operator(&ctx).await;
        let mut active: users::ActiveModel = session.user.clone().into_active_model();
        active.role = Set("admin".to_owned());
        active.update(&ctx.db).await.expect("papel");
        let api = format!("Bearer {}", session.token);

        // O token pessoal: o texto aparece uma vez só.
        let created = request
            .post("/api/registry/tokens")
            .add_header("Authorization", api.clone())
            .json(&json!({
                "name": "notebook",
                "scope": { "repositories": ["loja/*"], "actions": ["pull", "push", "delete"] },
                "days": 30
            }))
            .await;
        assert_eq!(created.status_code(), 201, "{}", created.text());
        let secret = body(&created.text())["secret"].as_str().expect("segredo").to_owned();
        assert!(secret.starts_with("nmr_"));
        let listed = request
            .get("/api/registry/tokens")
            .add_header("Authorization", api.clone())
            .await;
        assert!(!listed.text().contains(&secret), "o token não volta à tela");

        // Sem credencial: o desafio aponta onde pedir o token.
        let challenge = request
            .get("/v2/")
            .add_header("x-forwarded-for", INTERNAL)
            .await;
        assert_eq!(challenge.status_code(), 401);
        let header = challenge.header("www-authenticate");
        assert!(header.to_str().unwrap().contains("/v2/token"));

        let basic = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!("operator:{secret}"))
        );
        // Pela borda pública o token pessoal não vale.
        let outside = request
            .get("/v2/token?scope=repository:loja/api:pull,push")
            .add_header("Authorization", basic.clone())
            .add_header("x-forwarded-for", PUBLIC)
            .await;
        assert_eq!(outside.status_code(), 403);
        let issued = request
            .get("/v2/token?service=netmonitor-registry&scope=repository%3Aloja%2Fapi%3Apull%2Cpush")
            .add_header("Authorization", basic.clone())
            .add_header("x-forwarded-for", INTERNAL)
            .await;
        assert_eq!(issued.status_code(), 200, "{}", issued.text());
        let bearer = format!("Bearer {}", body(&issued.text())["token"].as_str().unwrap());

        // Uma camada em duas partes.
        let layer = b"conteudo-da-camada".to_vec();
        let started = request
            .post("/v2/loja/api/blobs/uploads/")
            .add_header("Authorization", bearer.clone())
            .add_header("x-forwarded-for", INTERNAL)
            .await;
        assert_eq!(started.status_code(), 202, "{}", started.text());
        let location = started.header("location").to_str().unwrap().to_owned();
        let first = request
            .patch(&location)
            .add_header("Authorization", bearer.clone())
            .add_header("x-forwarded-for", INTERNAL)
            .add_header("Content-Range", "0-8")
            .bytes(layer[..9].to_vec().into())
            .await;
        assert_eq!(first.status_code(), 202, "{}", first.text());
        assert_eq!(first.header("range").to_str().unwrap(), "0-8");
        let wrong = request
            .patch(&location)
            .add_header("Authorization", bearer.clone())
            .add_header("x-forwarded-for", INTERNAL)
            .add_header("Content-Range", "3-8")
            .bytes(layer[9..].to_vec().into())
            .await;
        assert_eq!(wrong.status_code(), 416, "parte fora de ordem");
        let finished = request
            // Como o cliente real manda: o digest codificado na query.
            .put(&format!("{location}?digest={}", digest(&layer).replace(':', "%3A")))
            .add_header("Authorization", bearer.clone())
            .add_header("x-forwarded-for", INTERNAL)
            .bytes(layer[9..].to_vec().into())
            .await;
        assert_eq!(finished.status_code(), 201, "{}", finished.text());

        // O config de uma vez, com o digest errado primeiro.
        let config = br#"{"architecture":"amd64","os":"linux","rootfs":{"type":"layers","diff_ids":[]}}"#.to_vec();
        let bad = request
            .post(&format!("/v2/loja/api/blobs/uploads/?digest={}", digest(b"outro")))
            .add_header("Authorization", bearer.clone())
            .add_header("x-forwarded-for", INTERNAL)
            .bytes(config.clone().into())
            .await;
        assert_eq!(bad.status_code(), 400);
        let monolithic = request
            .post(&format!("/v2/loja/api/blobs/uploads/?digest={}", digest(&config)))
            .add_header("Authorization", bearer.clone())
            .add_header("x-forwarded-for", INTERNAL)
            .bytes(config.clone().into())
            .await;
        assert_eq!(monolithic.status_code(), 201, "{}", monolithic.text());

        let manifest = json!({
            "schemaVersion": 2,
            "mediaType": "application/vnd.oci.image.manifest.v1+json",
            "config": {"mediaType": "application/vnd.oci.image.config.v1+json", "digest": digest(&config), "size": config.len()},
            "layers": [{"mediaType": "application/vnd.oci.image.layer.v1.tar+gzip", "digest": digest(&layer), "size": layer.len()}]
        })
        .to_string();
        let commit = "a1b2c3d4e5".repeat(4);
        let pushed = request
            .put(&format!("/v2/loja/api/manifests/{commit}"))
            .add_header("Authorization", bearer.clone())
            .add_header("x-forwarded-for", INTERNAL)
            .add_header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
            .bytes(manifest.clone().into_bytes().into())
            .await;
        assert_eq!(pushed.status_code(), 201, "{}", pushed.text());
        assert_eq!(
            pushed.header("docker-content-digest").to_str().unwrap(),
            digest(manifest.as_bytes())
        );

        // A tag de commit não muda de conteúdo.
        let other = manifest.replace("\"schemaVersion\":2", "\"schemaVersion\":2,\"annotations\":{\"x\":\"y\"}");
        let moved = request
            .put(&format!("/v2/loja/api/manifests/{commit}"))
            .add_header("Authorization", bearer.clone())
            .add_header("x-forwarded-for", INTERNAL)
            .add_header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
            .bytes(other.into_bytes().into())
            .await;
        assert_eq!(moved.status_code(), 409, "{}", moved.text());

        let pulled = request
            .get(&format!("/v2/loja/api/manifests/{commit}"))
            .add_header("Authorization", bearer.clone())
            .add_header("x-forwarded-for", INTERNAL)
            .await;
        assert_eq!(pulled.status_code(), 200);
        assert_eq!(pulled.text(), manifest);
        let partial = request
            .get(&format!("/v2/loja/api/blobs/{}", digest(&layer)))
            .add_header("Authorization", bearer.clone())
            .add_header("x-forwarded-for", INTERNAL)
            .add_header("Range", "bytes=2-6")
            .await;
        assert_eq!(partial.status_code(), 206);
        assert_eq!(partial.as_bytes().to_vec(), layer[2..=6].to_vec());
        let tags = request
            .get("/v2/loja/api/tags/list")
            .add_header("Authorization", bearer.clone())
            .add_header("x-forwarded-for", INTERNAL)
            .await;
        assert_eq!(body(&tags.text())["tags"], json!([commit]));

        // Outro repositório: o escopo não alcança, e o blob não é legível lá.
        let elsewhere = request
            .get(&format!("/v2/outra/api/blobs/{}", digest(&layer)))
            .add_header("Authorization", bearer.clone())
            .add_header("x-forwarded-for", INTERNAL)
            .await;
        assert_eq!(elsewhere.status_code(), 401);

        let overview = request
            .get("/api/registry")
            .add_header("Authorization", api.clone())
            .await;
        assert_eq!(overview.status_code(), 200, "{}", overview.text());
        let overview = body(&overview.text());
        assert_eq!(overview["repositories"][0]["name"], "loja/api");
        assert_eq!(overview["repositories"][0]["versions"], 1);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn aviso_do_github_app_sem_assinatura_valida_e_recusado() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, _ctx| async move {
        let unsigned = request
            .post("/api/deploy-hooks/github-app")
            .add_header("x-github-event", "push")
            .bytes(b"{}".to_vec().into())
            .await;
        assert_eq!(unsigned.status_code(), 401);
        let forged = request
            .post("/api/deploy-hooks/github-app")
            .add_header("x-github-event", "push")
            .add_header("x-hub-signature-256", format!("sha256={}", "0".repeat(64)))
            .bytes(b"{}".to_vec().into())
            .await;
        assert_eq!(forged.status_code(), 401);
    })
    .await;
}

/// Ponta a ponta com artefatos de verdade (ADR 022/023), fora da suíte normal
/// porque precisa do Docker e da internet (a base do OSV):
///
/// ```text
/// docker buildx build --sbom true --provenance mode=max \
///   --output type=oci,dest=/tmp/out.tar .
/// NM_E2E_OCI=/tmp/out.tar cargo test --test mod registry::e2e -- --ignored
/// ```
///
/// O arquivo que o `buildx` gerou entra como o agente o entregaria, a
/// varredura lê o SBOM anexado, e a entrega vira um `docker load` real.
#[tokio::test]
#[serial]
#[ignore = "precisa do Docker e de NM_E2E_OCI"]
async fn e2e_arquivo_do_buildx_ate_o_docker_load() {
    let Ok(path) = std::env::var("NM_E2E_OCI") else {
        return;
    };
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        use netmonitor_servers::deploys::images::{ImageOrigin, ImageStore};
        let registry = netmonitor_servers_registry::Registry;
        let archive = tokio::fs::File::open(&path)
            .await
            .expect("arquivo do buildx");
        let commit = "c0ffee".repeat(6) + "c0ff";
        let origin = ImageOrigin {
            source: netmonitor_servers::deploys::images::OriginKind::Build,
            repository: "acme/e2e".into(),
            commit: commit.clone(),
            reference: Some("refs/heads/main".into()),
            build_id: None,
            agent: Some("e2e".into()),
            triggered_by: Some("teste".into()),
            at: chrono::Utc::now().to_rfc3339(),
        };
        let digest = registry
            .ingest(
                &ctx,
                "e2e/app",
                1,
                &[commit.clone(), "main".into()],
                Box::new(archive),
                &origin,
            )
            .await
            .expect("ingestão");
        println!("guardado: {digest}");

        let verdict = registry
            .verdict(&ctx, "e2e/app", &commit, 1)
            .await
            .expect("varredura");
        println!("varredura: {verdict:?}");
        assert!(verdict.status.is_final(), "{verdict:?}");

        let out = std::env::temp_dir().join("nm-e2e-load.tar");
        let file = tokio::fs::File::create(&out).await.expect("tar");
        let mut sink: agent_protocol::tunnel::TunnelStream = Box::new(file);
        registry
            .deliver(
                &ctx,
                "e2e/app",
                &commit,
                "amd64",
                "netmonitor-e2e/app:1",
                &mut sink,
            )
            .await
            .expect("entrega");
        drop(sink);
        let loaded = std::process::Command::new("docker")
            .args(["load", "-i"])
            .arg(&out)
            .output()
            .expect("docker load");
        println!("{}", String::from_utf8_lossy(&loaded.stdout));
        assert!(
            loaded.status.success(),
            "{}",
            String::from_utf8_lossy(&loaded.stderr)
        );
        let ran = std::process::Command::new("docker")
            .args(["run", "--rm", "netmonitor-e2e/app:1"])
            .output()
            .expect("docker run");
        println!(
            "saída do container: {}",
            String::from_utf8_lossy(&ran.stdout)
        );
        assert!(ran.status.success());
        let _ = std::process::Command::new("docker")
            .args(["rmi", "netmonitor-e2e/app:1"])
            .output();
        let _ = std::fs::remove_file(&out);
    })
    .await;
}
