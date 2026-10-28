//! Acesso dedicado por chave SSH (ADR 016) pela API, contra um "OpenWrt" falso
//! em 127.0.0.1: um servidor SSH em processo que guarda o `authorized_keys` em
//! memória e só aceita a senha do root ou uma chave instalada ali.

use std::sync::{Arc, Mutex};

use backend::{
    app::App,
    models::{device_credentials, devices},
    services::{
        plugins::{
            credentials,
            dedicated_access::keys,
            manifest::TransportKind,
            transport::{
                local::LocalTransport, DeviceIoCall, DeviceIoReply, DeviceTransport, Endpoint,
            },
        },
        shared::crypto,
        users::Role,
    },
};
use loco_rs::testing::prelude::*;
use russh::{
    keys::ssh_key::PublicKey,
    server::{self, Auth, Msg, Session},
    Channel, ChannelId,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set};
use serde_json::{json, Value};
use serial_test::serial;

use super::prepare_data;

const ROOT_PASSWORD: &str = "senha-do-root";

fn json_of(text: &str) -> Value {
    serde_json::from_str(text).expect("resposta JSON")
}

#[derive(Clone, Default)]
struct FakeOpenwrt {
    authorized_keys: Arc<Mutex<Vec<String>>>,
    root_password: Arc<Mutex<String>>,
}

fn base64_of(line: &str) -> Option<&str> {
    line.split_whitespace()
        .find(|part| part.starts_with("AAAA"))
}

impl FakeOpenwrt {
    fn run(&self, command: &str) -> String {
        if command.starts_with("cat /etc/openwrt_release") {
            return "DISTRIB_ID='OpenWrt'\nDISTRIB_RELEASE='23.05.3'\n/usr/sbin/dropbear\n".into();
        }
        let mut keys = self.authorized_keys.lock().unwrap();
        if let Some(rest) = command.split("echo '").nth(1) {
            let line = rest.split("' >>").next().unwrap().to_owned();
            let comment = line.split_whitespace().last().unwrap().to_owned();
            if !keys.iter().any(|k| k.ends_with(&comment)) {
                keys.push(line);
            }
        } else if let Some(rest) = command.strip_prefix("sed -i '/ ") {
            let comment = rest.split("$/d'").next().unwrap();
            keys.retain(|k| !k.ends_with(comment));
        }
        "ok\n".into()
    }
}

impl server::Handler for FakeOpenwrt {
    type Error = russh::Error;

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        Ok(
            if user == "root" && password == *self.root_password.lock().unwrap() {
                Auth::Accept
            } else {
                Auth::reject()
            },
        )
    }

    async fn auth_publickey(&mut self, user: &str, key: &PublicKey) -> Result<Auth, Self::Error> {
        let offered = key.to_openssh().unwrap();
        let installed = self
            .authorized_keys
            .lock()
            .unwrap()
            .iter()
            .any(|line| base64_of(line) == base64_of(&offered));
        Ok(if user == "root" && installed {
            Auth::Accept
        } else {
            Auth::reject()
        })
    }

    async fn channel_open_session(
        &mut self,
        _channel: Channel<Msg>,
        reply: server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        Ok(())
    }

    async fn exec_request(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let output = self.run(&String::from_utf8_lossy(data));
        session.channel_success(channel)?;
        session.data(channel, output.into_bytes())?;
        session.exit_status_request(channel, 0)?;
        session.eof(channel)?;
        session.close(channel)?;
        Ok(())
    }
}

async fn fake_openwrt() -> (u16, FakeOpenwrt) {
    let host_key = keys::generate().unwrap();
    let config = Arc::new(server::Config {
        keys: vec![russh::keys::decode_secret_key(&host_key.private_openssh, None).unwrap()],
        auth_rejection_time: std::time::Duration::from_millis(10),
        auth_rejection_time_initial: Some(std::time::Duration::from_millis(0)),
        ..Default::default()
    });
    let fake = FakeOpenwrt::default();
    *fake.root_password.lock().unwrap() = ROOT_PASSWORD.into();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handler = fake.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let (config, handler) = (config.clone(), handler.clone());
            tokio::spawn(async move {
                if let Ok(session) = server::run_stream(config, stream, handler).await {
                    let _ = session.await;
                }
            });
        }
    });
    (port, fake)
}

/// O que um plugin faria: resolver a credencial e rodar um comando.
async fn run_as_plugin(
    ctx: &loco_rs::app::AppContext,
    device_id: i64,
    port: u16,
) -> Result<String, String> {
    let resolved = credentials::resolve(&ctx.db, device_id, &[TransportKind::Ssh])
        .await
        .map_err(|error| error.to_string())?;
    let login = resolved.credentials.ssh.unwrap().login;
    let reply = LocalTransport::new("central")
        .unwrap()
        .execute(&DeviceIoCall::SshExec {
            endpoint: Endpoint {
                host: "127.0.0.1".into(),
                port,
            },
            login,
            command: "uptime".into(),
            timeout_ms: 5_000,
        })
        .await
        .map_err(|error| error.to_string())?;
    match reply {
        DeviceIoReply::Exec { stdout, .. } => Ok(stdout),
        DeviceIoReply::Http { .. } => Err("http".into()),
    }
}

async fn ssh_row(ctx: &loco_rs::app::AppContext, device_id: i64) -> device_credentials::Model {
    device_credentials::Entity::find()
        .filter(device_credentials::Column::DeviceId.eq(device_id))
        .one(&ctx.db)
        .await
        .unwrap()
        .expect("credencial SSH")
}

#[tokio::test]
#[serial]
async fn acesso_dedicado_do_primeiro_acesso_a_revogacao() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_operator(&ctx).await;
        let mut user = session.user.into_active_model();
        user.role = Set(Role::Admin.as_str().to_string());
        user.update(&ctx.db).await.unwrap();
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let (port, fake) = fake_openwrt().await;
        let device = devices::ActiveModel {
            name: Set("AP-terraço".into()),
            r#type: Set("router".into()),
            status: Set("online".into()),
            ip_address: Set(Some("127.0.0.1".into())),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();
        let dedicated = format!("/api/devices/{}/access/dedicated", device.id);

        // Senha errada: nada instalado, nada guardado.
        let recusado = request
            .post(&dedicated)
            .json(&json!({ "username": "root", "password": "errada", "port": port }))
            .await;
        assert!(!recusado.status_code().is_success());
        assert!(fake.authorized_keys.lock().unwrap().is_empty());
        assert!(credentials::list(&ctx.db, device.id)
            .await
            .unwrap()
            .is_empty());

        // Primeiro acesso: a senha do root entra uma vez.
        let criado = request
            .post(&dedicated)
            .json(&json!({ "username": "root", "password": ROOT_PASSWORD, "port": port }))
            .await;
        criado.assert_status_success();
        let texto = criado.text();
        assert!(!texto.contains("PRIVATE KEY"), "a chave privada não sai");
        assert!(!texto.contains(ROOT_PASSWORD));
        let view = json_of(&texto);
        assert_eq!(view["storage"], "key");
        assert_eq!(view["username"], "root");
        assert_eq!(view["dedicated"]["platform"], "openwrt");
        let first_fingerprint = view["dedicated"]["keyFingerprint"]
            .as_str()
            .unwrap()
            .to_owned();
        let host_key = view["dedicated"]["hostKey"].as_str().unwrap().to_owned();
        assert!(
            host_key.starts_with("SHA256:"),
            "a chave de host fica fixada"
        );
        {
            let keys = fake.authorized_keys.lock().unwrap();
            assert_eq!(keys.len(), 1);
            assert!(keys[0].starts_with(
                "no-port-forwarding,no-agent-forwarding,no-X11-forwarding ssh-ed25519 "
            ));
        }
        let row = ssh_row(&ctx, device.id).await;
        let guardado = crypto::decrypt(row.secret_encrypted.as_deref().unwrap()).unwrap();
        assert!(
            guardado.contains("OPENSSH PRIVATE KEY"),
            "só a chave vai ao banco"
        );

        // Trocar a senha do root não derruba o monitoramento.
        *fake.root_password.lock().unwrap() = "senha-nova".into();
        assert_eq!(run_as_plugin(&ctx, device.id, port).await.unwrap(), "ok\n");

        // O formulário comum não troca o acesso dedicado por senha.
        request
            .put(&format!("/api/devices/{}/credentials", device.id))
            .json(&json!({ "kind": "ssh", "username": "root", "secret": "x", "storage": "ask" }))
            .await
            .assert_status_unprocessable_entity();

        // Rotação: chave nova, a antiga sai do equipamento.
        let rodado = request.post(&format!("{dedicated}/rotate")).await;
        rodado.assert_status_success();
        let rodado = json_of(&rodado.text());
        assert_ne!(
            rodado["dedicated"]["keyFingerprint"],
            first_fingerprint.as_str()
        );
        assert_eq!(rodado["dedicated"]["hostKey"], host_key.as_str());
        assert_eq!(fake.authorized_keys.lock().unwrap().len(), 1);
        assert_eq!(run_as_plugin(&ctx, device.id, port).await.unwrap(), "ok\n");

        // Outro equipamento no lugar (chave de host diferente) é recusado.
        let row = ssh_row(&ctx, device.id).await;
        let mut extra = row.extra.clone().unwrap();
        extra["dedicated"]["hostKey"] = json!("SHA256:outra-identidade");
        let mut active = row.clone().into_active_model();
        active.extra = Set(Some(extra));
        active.update(&ctx.db).await.unwrap();
        let erro = run_as_plugin(&ctx, device.id, port).await.unwrap_err();
        assert!(erro.contains("identidade do equipamento mudou"), "{erro}");
        let mut active = row.into_active_model();
        active.extra = Set(Some({
            let mut extra = ssh_row(&ctx, device.id).await.extra.unwrap();
            extra["dedicated"]["hostKey"] = json!(host_key);
            extra
        }));
        active.update(&ctx.db).await.unwrap();

        // Apagar só aqui deixaria a chave esquecida lá: o DELETE comum recusa.
        let apagar = request
            .delete(&format!("/api/devices/{}/credentials/ssh", device.id))
            .await;
        assert!(!apagar.status_code().is_success(), "{}", apagar.text());
        assert!(
            apagar.text().contains("Revogar acesso"),
            "{}",
            apagar.text()
        );
        assert_eq!(fake.authorized_keys.lock().unwrap().len(), 1);

        // Revogar tira a chave de lá e a credencial daqui.
        let revogado = request.delete(&dedicated).await;
        revogado.assert_status_success();
        assert_eq!(json_of(&revogado.text())["leftovers"], json!([]));
        assert!(fake.authorized_keys.lock().unwrap().is_empty());
        assert!(credentials::list(&ctx.db, device.id)
            .await
            .unwrap()
            .is_empty());
    })
    .await;
}

/// O mesmo fluxo contra um equipamento de verdade — RouterOS CHR ou OpenWrt
/// em Docker, nunca a rede de produção. Roda só quando pedido:
///
/// ```text
/// NM_REAL_SSH_PORT=2222 NM_REAL_SSH_USER=root NM_REAL_SSH_PASSWORD=… \
///   cargo test --test mod real_device -- --ignored
/// ```
///
/// `NM_REAL_SSH_HOST` (padrão `127.0.0.1`) e `NM_REAL_PLATFORM` (`mikrotik`
/// ou `openwrt`; sem ele, descobre) são opcionais; `NM_REAL_KEEP=1` para antes
/// de revogar, para conferir o equipamento.
#[tokio::test]
#[serial]
#[ignore = "precisa de um RouterOS/OpenWrt real (ver o comentário)"]
async fn acesso_dedicado_em_equipamento_real() {
    use backend::services::plugins::dedicated_access::{
        self, DedicatedPlatform, ProvisionInput, RevokeInput, Target,
    };
    let env = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
    let host = env("NM_REAL_SSH_HOST").unwrap_or_else(|| "127.0.0.1".into());
    let port: u16 = env("NM_REAL_SSH_PORT")
        .expect("NM_REAL_SSH_PORT")
        .parse()
        .unwrap();
    let username = env("NM_REAL_SSH_USER").expect("NM_REAL_SSH_USER");
    let password = env("NM_REAL_SSH_PASSWORD").unwrap_or_default();
    let platform = env("NM_REAL_PLATFORM").map(|name| match name.as_str() {
        "mikrotik" => DedicatedPlatform::Mikrotik,
        _ => DedicatedPlatform::Openwrt,
    });

    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let device = devices::ActiveModel {
            name: Set("Equipamento real".into()),
            r#type: Set("router".into()),
            status: Set("online".into()),
            ip_address: Set(Some(host.clone())),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();
        let target = Target {
            ip: host.clone(),
            port,
            via_probe_id: None,
        };
        let transport = LocalTransport::new("central").unwrap();

        let view = dedicated_access::provision_on(
            &ctx.db,
            &transport,
            device.id,
            &target,
            ProvisionInput {
                platform,
                username,
                password,
                port: Some(port),
                via_probe_id: None,
            },
        )
        .await
        .unwrap_or_else(|error| panic!("criar: {error}"));
        let first = view.dedicated.clone().expect("acesso dedicado");
        println!(
            "criado: {} como {} — {:?}",
            first.key_fingerprint, view.username, first.platform
        );
        assert!(first.host_key.is_some(), "a chave de host fica fixada");
        run_as_plugin(&ctx, device.id, port)
            .await
            .unwrap_or_else(|error| panic!("usar a chave: {error}"));

        let rotated = dedicated_access::rotate_on(&ctx.db, &transport, device.id, &target)
            .await
            .unwrap_or_else(|error| panic!("rotacionar: {error}"));
        let second = rotated.dedicated.expect("acesso dedicado");
        assert_ne!(second.key_fingerprint, first.key_fingerprint);
        // Transporte novo: a sessão da chave antiga não pode mascarar o teste.
        run_as_plugin(&ctx, device.id, port)
            .await
            .unwrap_or_else(|error| panic!("usar a chave rotacionada: {error}"));

        if env("NM_REAL_KEEP").is_some() {
            // Para conferir o equipamento à mão; revogue depois.
            return;
        }
        let outcome = dedicated_access::revoke_on(
            &ctx.db,
            &LocalTransport::new("central").unwrap(),
            device.id,
            &target,
            &RevokeInput::default(),
        )
        .await
        .unwrap_or_else(|error| panic!("revogar: {error}"));
        println!("revogado; sobrou: {:?}", outcome.leftovers);
        assert!(credentials::list(&ctx.db, device.id)
            .await
            .unwrap()
            .is_empty());
    })
    .await;
}
