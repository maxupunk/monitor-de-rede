//! Cadastro das credenciais SNMP (`snmp_profiles`): listar, criar, editar,
//! apagar e testar contra um alvo.
//!
//! A cifra, a validação e a resolução "qual credencial vale para este
//! equipamento" moram no núcleo (`devices::snmp_credentials`), porque o
//! núcleo também precisa delas; aqui fica o que é da tela — máscara dos
//! segredos, "máscara ou vazio mantém o gravado", o padrão único e a recusa de
//! apagar credencial em uso.

use std::collections::HashMap;

use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};

use netmonitor_core::{
    devices::snmp_credentials::{self as core_credentials, SnmpCredentials, SnmpVersion},
    models::{devices, snmp_profiles},
    shared::{
        errors::{AppError, AppResult},
        transaction::begin_write,
    },
};

use crate::{
    dtos::snmp_credentials::{
        SnmpCredentialInput, SnmpCredentialView, SnmpTargetInput, SECRET_MASK,
    },
    snmp::{
        client::SnmpConfig,
        service::{self, DetectCandidate, SnmpTestResult},
    },
};

/// Tamanho máximo do nome (o da coluna).
const NAME_MAX: usize = 120;

/// Texto opcional aparado; vazio vira `None`.
fn trimmed(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

/// Segredo digitado: vazio e a máscara significam "não mexi".
fn typed_secret(value: Option<&str>) -> Option<String> {
    value
        .filter(|text| !text.is_empty() && *text != SECRET_MASK)
        .map(str::to_owned)
}

fn lower(value: Option<&str>) -> Option<String> {
    trimmed(value).map(|text| text.to_ascii_lowercase())
}

/// Monta a credencial pedida. Na edição, `stored` é a gravada: segredo não
/// digitado continua o de antes.
fn credentials_from(
    input: &SnmpCredentialInput,
    stored: Option<&SnmpCredentials>,
) -> AppResult<SnmpCredentials> {
    let version = SnmpVersion::parse(&input.version)
        .ok_or_else(|| AppError::validation("Versão SNMP inválida. Use v1, v2c ou v3."))?;
    let keep = |typed: Option<String>, old: fn(&SnmpCredentials) -> Option<String>| {
        typed.or_else(|| stored.and_then(old))
    };
    if version == SnmpVersion::V3 {
        let auth_protocol = lower(input.auth_protocol.as_deref());
        let priv_protocol = lower(input.priv_protocol.as_deref());
        Ok(SnmpCredentials {
            version,
            community: String::new(),
            username: trimmed(input.username.as_deref()),
            auth_key: auth_protocol.as_ref().and_then(|_| {
                keep(typed_secret(input.auth_key.as_deref()), |c| {
                    c.auth_key.clone()
                })
            }),
            priv_key: priv_protocol.as_ref().and_then(|_| {
                keep(typed_secret(input.priv_key.as_deref()), |c| {
                    c.priv_key.clone()
                })
            }),
            auth_protocol,
            priv_protocol,
            context_name: trimmed(input.context_name.as_deref()),
        })
    } else {
        let stored_community = |c: &SnmpCredentials| c.community_text().map(str::to_owned);
        let community = typed_secret(input.community.as_deref().map(str::trim))
            .or_else(|| stored.and_then(stored_community))
            .unwrap_or_default();
        Ok(SnmpCredentials::community(version, community))
    }
}

fn validated_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::validation("Dê um nome à credencial SNMP."));
    }
    if name.chars().count() > NAME_MAX {
        return Err(AppError::validation(format!(
            "O nome da credencial SNMP tem no máximo {NAME_MAX} caracteres."
        )));
    }
    Ok(name.to_owned())
}

async fn ensure_unique_name<C: ConnectionTrait>(
    db: &C,
    name: &str,
    except: Option<i64>,
) -> AppResult<()> {
    let mut query = snmp_profiles::Entity::find().filter(snmp_profiles::Column::Name.eq(name));
    if let Some(id) = except {
        query = query.filter(snmp_profiles::Column::Id.ne(id));
    }
    if query.count(db).await? > 0 {
        return Err(AppError::conflict(format!(
            "Já existe uma credencial SNMP chamada \"{name}\"."
        )));
    }
    Ok(())
}

/// Só uma credencial é a padrão: marcar uma desmarca as outras.
async fn clear_other_defaults<C: ConnectionTrait>(db: &C, keep: i64) -> AppResult<()> {
    snmp_profiles::Entity::update_many()
        .col_expr(
            snmp_profiles::Column::IsDefault,
            sea_orm::sea_query::Expr::value(false),
        )
        .filter(snmp_profiles::Column::Id.ne(keep))
        .filter(snmp_profiles::Column::IsDefault.eq(true))
        .exec(db)
        .await?;
    Ok(())
}

fn masked(present: bool) -> Option<String> {
    present.then(|| SECRET_MASK.to_owned())
}

/// A credencial como a tela a vê.
#[must_use]
pub fn present(profile: &snmp_profiles::Model, device_count: u64) -> SnmpCredentialView {
    let v3 = profile.version == SnmpVersion::V3.as_str();
    let security_level = if v3 {
        core_credentials::SecurityLevel::of(
            profile.auth_protocol.is_some(),
            profile.priv_protocol.is_some(),
        )
        .as_str()
    } else {
        "community"
    };
    SnmpCredentialView {
        id: profile.id,
        name: profile.name.clone(),
        version: profile.version.clone(),
        security_level: security_level.to_owned(),
        community: masked(profile.community_encrypted.is_some()),
        username: profile.username.clone(),
        auth_protocol: profile.auth_protocol.clone(),
        auth_key: masked(profile.auth_key_encrypted.is_some()),
        priv_protocol: profile.priv_protocol.clone(),
        priv_key: masked(profile.priv_key_encrypted.is_some()),
        context_name: profile.context_name.clone(),
        is_default: profile.is_default,
        device_count,
        created_at: profile.created_at.to_rfc3339(),
        updated_at: profile.updated_at.to_rfc3339(),
    }
}

async fn device_count<C: ConnectionTrait>(db: &C, id: i64) -> AppResult<u64> {
    Ok(devices::Entity::find()
        .filter(devices::Column::SnmpProfileId.eq(id))
        .count(db)
        .await?)
}

async fn find<C: ConnectionTrait>(db: &C, id: i64) -> AppResult<snmp_profiles::Model> {
    snmp_profiles::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Credencial SNMP não encontrada"))
}

/// Todas as credenciais, a padrão primeiro, com quantos dispositivos usam
/// cada uma.
///
/// # Errors
///
/// Erro do banco.
pub async fn list(db: &DatabaseConnection) -> AppResult<Vec<SnmpCredentialView>> {
    let profiles = snmp_profiles::Entity::find()
        .order_by_desc(snmp_profiles::Column::IsDefault)
        .order_by_asc(snmp_profiles::Column::Name)
        .all(db)
        .await?;
    let counts: HashMap<i64, i64> = devices::Entity::find()
        .select_only()
        .column(devices::Column::SnmpProfileId)
        .column_as(devices::Column::Id.count(), "total")
        .filter(devices::Column::SnmpProfileId.is_not_null())
        .group_by(devices::Column::SnmpProfileId)
        .into_tuple::<(Option<i64>, i64)>()
        .all(db)
        .await?
        .into_iter()
        .filter_map(|(id, total)| id.map(|id| (id, total)))
        .collect();
    Ok(profiles
        .iter()
        .map(|profile| {
            let total = counts.get(&profile.id).copied().unwrap_or(0);
            present(profile, u64::try_from(total).unwrap_or(0))
        })
        .collect())
}

/// Uma credencial.
///
/// # Errors
///
/// 404 ou erro do banco.
pub async fn show(db: &DatabaseConnection, id: i64) -> AppResult<SnmpCredentialView> {
    let profile = find(db, id).await?;
    Ok(present(&profile, device_count(db, id).await?))
}

/// Cria uma credencial.
///
/// # Errors
///
/// Nome vazio ou repetido (409), credencial incompleta (422) ou erro do banco.
pub async fn create(
    db: &DatabaseConnection,
    input: &SnmpCredentialInput,
) -> AppResult<SnmpCredentialView> {
    let name = validated_name(&input.name)?;
    let credentials = credentials_from(input, None)?;
    let txn = begin_write(db).await?;
    ensure_unique_name(&txn, &name, None).await?;
    let mut row = snmp_profiles::ActiveModel {
        name: Set(name),
        is_default: Set(input.is_default),
        ..Default::default()
    };
    core_credentials::apply(&mut row, &credentials)?;
    let saved = row.insert(&txn).await?;
    if saved.is_default {
        clear_other_defaults(&txn, saved.id).await?;
    }
    txn.commit().await?;
    Ok(present(&saved, 0))
}

/// Edita uma credencial. Segredo vazio ou mascarado mantém o gravado.
///
/// Devolve a de antes e a de depois (para a auditoria).
///
/// # Errors
///
/// 404, nome repetido (409), credencial incompleta (422), gravada ilegível ou
/// erro do banco.
pub async fn update(
    db: &DatabaseConnection,
    id: i64,
    input: &SnmpCredentialInput,
) -> AppResult<(SnmpCredentialView, SnmpCredentialView)> {
    let name = validated_name(&input.name)?;
    let txn = begin_write(db).await?;
    let current = find(&txn, id).await?;
    let count = device_count(&txn, id).await?;
    let before = present(&current, count);
    let stored = core_credentials::decrypt(&current)?;
    let credentials = credentials_from(input, Some(&stored))?;
    ensure_unique_name(&txn, &name, Some(id)).await?;
    let mut row: snmp_profiles::ActiveModel = current.into();
    row.name = Set(name);
    row.is_default = Set(input.is_default);
    core_credentials::apply(&mut row, &credentials)?;
    let saved = row.update(&txn).await?;
    if saved.is_default {
        clear_other_defaults(&txn, saved.id).await?;
    }
    txn.commit().await?;
    Ok((before, present(&saved, count)))
}

/// Apaga uma credencial que nenhum dispositivo usa.
///
/// Recusar (em vez de soltar os dispositivos para a padrão) é deliberado: a
/// troca silenciosa de credencial faria equipamentos pararem de responder sem
/// ninguém ter mexido neles. Quem quer apagar troca a credencial deles antes.
///
/// # Errors
///
/// 404, credencial em uso (409) ou erro do banco.
pub async fn delete(db: &DatabaseConnection, id: i64) -> AppResult<SnmpCredentialView> {
    let txn = begin_write(db).await?;
    let current = find(&txn, id).await?;
    let in_use: Vec<String> = devices::Entity::find()
        .select_only()
        .column(devices::Column::Name)
        .filter(devices::Column::SnmpProfileId.eq(id))
        .order_by_asc(devices::Column::Name)
        .into_tuple::<String>()
        .all(&txn)
        .await?;
    if !in_use.is_empty() {
        const SHOWN: usize = 5;
        let mut names = in_use
            .iter()
            .take(SHOWN)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        if in_use.len() > SHOWN {
            names.push_str(&format!(" e mais {}", in_use.len() - SHOWN));
        }
        return Err(AppError::conflict(format!(
            "A credencial \"{}\" está em uso por {} dispositivo(s): {names}. Troque a credencial \
             deles antes de apagá-la.",
            current.name,
            in_use.len()
        )));
    }
    let view = present(&current, 0);
    snmp_profiles::Entity::delete_by_id(id).exec(&txn).await?;
    txn.commit().await?;
    Ok(view)
}

/// A credencial de uma consulta avulsa ([`SnmpTargetInput`]): o perfil, a
/// avulsa ou a padrão.
///
/// # Errors
///
/// Perfil inexistente (404), credencial avulsa incompleta (422), ilegível ou
/// erro do banco.
pub async fn target_candidate<C: ConnectionTrait>(
    db: &C,
    input: &SnmpTargetInput,
) -> AppResult<DetectCandidate> {
    if let Some(id) = input.profile_id {
        return Ok(DetectCandidate {
            profile_id: Some(id),
            credentials: core_credentials::for_profile(db, id).await?,
        });
    }
    if let Some(version) = trimmed(input.version.as_deref()) {
        let credentials = credentials_from(
            &SnmpCredentialInput {
                name: String::new(),
                version,
                community: input.community.clone(),
                username: input.username.clone(),
                auth_protocol: input.auth_protocol.clone(),
                auth_key: input.auth_key.clone(),
                priv_protocol: input.priv_protocol.clone(),
                priv_key: input.priv_key.clone(),
                context_name: input.context_name.clone(),
                is_default: false,
            },
            None,
        )?;
        credentials.validate()?;
        return Ok(DetectCandidate {
            profile_id: None,
            credentials,
        });
    }
    let (profile_id, credentials) = core_credentials::default_credentials(db).await?;
    Ok(DetectCandidate {
        profile_id,
        credentials,
    })
}

/// A configuração do cliente para uma consulta avulsa.
///
/// # Errors
///
/// Os de [`target_candidate`], e host vazio (422).
pub async fn target_config<C: ConnectionTrait>(
    db: &C,
    input: &SnmpTargetInput,
) -> AppResult<SnmpConfig> {
    let host = input.host.trim();
    if host.is_empty() {
        return Err(AppError::validation("Host SNMP é obrigatório"));
    }
    let candidate = target_candidate(db, input).await?;
    Ok(SnmpConfig::from_credentials(
        host,
        input.port.unwrap_or(161),
        &candidate.credentials,
    ))
}

/// Testa uma credencial gravada contra um alvo (`sysDescr`, `sysName`…).
/// Senha errada volta como erro, com a mensagem do agente.
///
/// # Errors
///
/// 404, host vazio, credencial ilegível ou falha SNMP.
pub async fn test_against(
    db: &DatabaseConnection,
    id: i64,
    host: &str,
    port: Option<u16>,
) -> AppResult<SnmpTestResult> {
    let host = host.trim();
    if host.is_empty() {
        return Err(AppError::validation("Informe o IP ou nome do alvo."));
    }
    let credentials = core_credentials::for_profile(db, id).await?;
    service::test_connection(SnmpConfig::from_credentials(
        host,
        port.unwrap_or(161),
        &credentials,
    ))
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(version: &str) -> SnmpCredentialInput {
        SnmpCredentialInput {
            name: "teste".into(),
            version: version.into(),
            ..SnmpCredentialInput::default()
        }
    }

    fn v3_gravada() -> SnmpCredentials {
        SnmpCredentials {
            version: SnmpVersion::V3,
            community: String::new(),
            username: Some("monitor".into()),
            auth_protocol: Some("sha".into()),
            auth_key: Some("senha-auth".into()),
            priv_protocol: Some("aes128".into()),
            priv_key: Some("senha-priv".into()),
            context_name: None,
        }
    }

    #[test]
    fn mascara_ou_vazio_mantem_o_segredo_gravado() {
        let gravada = v3_gravada();
        for digitado in [None, Some(""), Some(SECRET_MASK)] {
            let pedido = SnmpCredentialInput {
                username: Some("monitor".into()),
                auth_protocol: Some("SHA".into()),
                auth_key: digitado.map(Into::into),
                priv_protocol: Some("aes128".into()),
                priv_key: digitado.map(Into::into),
                ..input("v3")
            };
            let resultado = credentials_from(&pedido, Some(&gravada)).unwrap();
            assert_eq!(resultado, gravada, "{digitado:?}");
        }
        let trocada = SnmpCredentialInput {
            username: Some("monitor".into()),
            auth_protocol: Some("sha".into()),
            auth_key: Some("senha-nova-1".into()),
            ..input("v3")
        };
        let resultado = credentials_from(&trocada, Some(&gravada)).unwrap();
        assert_eq!(resultado.auth_key.as_deref(), Some("senha-nova-1"));
        assert!(
            resultado.priv_key.is_none() && resultado.priv_protocol.is_none(),
            "sem protocolo de privacidade, authNoPriv: a senha velha não sobra"
        );
    }

    #[test]
    fn comunidade_mascarada_mantem_a_gravada_e_nova_substitui() {
        let gravada = SnmpCredentials::community(SnmpVersion::V2c, "segredo");
        let mantida = credentials_from(
            &SnmpCredentialInput {
                community: Some(SECRET_MASK.into()),
                ..input("v2c")
            },
            Some(&gravada),
        )
        .unwrap();
        assert_eq!(mantida, gravada);
        let nova = credentials_from(
            &SnmpCredentialInput {
                community: Some(" outra ".into()),
                ..input("v1")
            },
            Some(&gravada),
        )
        .unwrap();
        assert_eq!(nova, SnmpCredentials::community(SnmpVersion::V1, "outra"));
        // De v3 para v2c não há comunidade para herdar.
        let sem = credentials_from(&input("v2c"), Some(&v3_gravada())).unwrap();
        assert!(sem.validate().is_err());
    }

    /// Validação contra um `snmpd` (net-snmp) de verdade, em `127.0.0.1`.
    ///
    /// Suba o agente com uma comunidade v2c `nm-v2c-test`, o usuário
    /// `nmauth` (authNoPriv, SHA, `authpass123`) e o `nmpriv` (authPriv,
    /// SHA/AES, `authpass123`/`privpass123`), publicado só no loopback, e rode:
    ///
    /// ```text
    /// NM_SNMPD_PORT=16161 cargo test -p netmonitor-network -j 2 \
    ///     agente_real -- --ignored --nocapture
    /// ```
    ///
    /// O caminho é o de produção: perfil gravado (cifrado) → dispositivo →
    /// `service::device_config` → cliente; e a detecção da descoberta com os
    /// perfis do banco.
    #[tokio::test]
    #[ignore = "precisa de um snmpd em 127.0.0.1 (ver o comentário)"]
    async fn agente_real_autentica_v2c_v3_authnopriv_e_authpriv_por_perfil() {
        use crate::migrations::{MigratorTrait, TestMigrator};
        use crate::snmp::{client::SnmpClient, collectors};

        let port: u16 = std::env::var("NM_SNMPD_PORT")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(16_161);
        let mut options = sea_orm::ConnectOptions::new("sqlite::memory:".to_owned());
        options.max_connections(1).min_connections(1);
        let db = sea_orm::Database::connect(options).await.unwrap();
        TestMigrator::up(&db, None).await.unwrap();

        let perfil = |name: &str, credentials: SnmpCredentials| {
            let db = db.clone();
            let name = name.to_owned();
            async move {
                let mut row = snmp_profiles::ActiveModel {
                    name: Set(name),
                    is_default: Set(false),
                    ..Default::default()
                };
                core_credentials::apply(&mut row, &credentials).unwrap();
                row.insert(&db).await.unwrap().id
            }
        };
        let v3 = |user: &str, auth: &str, privacy: Option<&str>| SnmpCredentials {
            version: SnmpVersion::V3,
            community: String::new(),
            username: Some(user.into()),
            auth_protocol: Some("sha".into()),
            auth_key: Some(auth.into()),
            priv_protocol: privacy.map(|_| "aes128".into()),
            priv_key: privacy.map(Into::into),
            context_name: None,
        };
        let casos = [
            (
                "v2c",
                perfil(
                    "v2c",
                    SnmpCredentials::community(SnmpVersion::V2c, "nm-v2c-test"),
                )
                .await,
            ),
            (
                "v3 authNoPriv",
                perfil("authNoPriv", v3("nmauth", "authpass123", None)).await,
            ),
            (
                "v3 authPriv",
                perfil("authPriv", v3("nmpriv", "authpass123", Some("privpass123"))).await,
            ),
        ];

        for (rotulo, profile_id) in casos {
            let device = devices::ActiveModel {
                name: Set(format!("snmpd {rotulo}")),
                r#type: Set("server".into()),
                ip_address: Set(Some("127.0.0.1".into())),
                snmp_enabled: Set(true),
                snmp_profile_id: Set(Some(profile_id)),
                ..Default::default()
            }
            .insert(&db)
            .await
            .unwrap();
            let mut config = service::device_config(&db, &device).await.unwrap();
            config.port = port;
            let teste = service::test_connection(config.clone())
                .await
                .unwrap_or_else(|error| panic!("{rotulo}: {error}"));
            assert!(teste.success, "{rotulo}: sem sysDescr");
            let interfaces = collectors::collect_interfaces(&SnmpClient::new(config))
                .await
                .unwrap_or_else(|error| panic!("{rotulo}: ifTable: {error}"));
            assert!(!interfaces.is_empty(), "{rotulo}: ifTable vazia");
            println!(
                "{rotulo}: sysDescr={:?} sysName={:?} interfaces={:?}",
                teste.system.sys_descr,
                teste.system.sys_name,
                interfaces
                    .iter()
                    .map(|i| i.if_name.clone())
                    .collect::<Vec<_>>()
            );
        }

        // A descoberta tenta os perfis do banco e diz qual respondeu.
        let candidatos = DetectCandidate::load(&db).await.unwrap();
        let detectado = service::detect_connection("127.0.0.1", port, None, &candidatos)
            .await
            .unwrap();
        assert!(detectado.detected);
        println!(
            "detecção: versão {:?}, perfil {:?}",
            detectado.version, detectado.profile_id
        );

        // Senha errada falha com erro claro — nunca "respondeu vazio".
        for (rotulo, errada) in [
            (
                "auth errada",
                v3("nmpriv", "senha-errada", Some("privpass123")),
            ),
            (
                "priv errada",
                v3("nmpriv", "authpass123", Some("priv-errada")),
            ),
            ("usuário inexistente", v3("ninguem", "authpass123", None)),
            (
                "comunidade errada",
                SnmpCredentials::community(SnmpVersion::V2c, "nao-existe"),
            ),
        ] {
            let mut config = SnmpConfig::from_credentials("127.0.0.1", port, &errada);
            config.timeout_ms = 1_500;
            let erro = service::test_connection(config)
                .await
                .expect_err(rotulo)
                .to_string();
            println!("{rotulo}: {erro}");
        }
    }

    #[test]
    fn visao_nunca_traz_segredo() {
        let agora = chrono::Utc::now().into();
        let mut row = snmp_profiles::ActiveModel {
            id: Set(7),
            name: Set("core".into()),
            is_default: Set(false),
            created_at: Set(agora),
            updated_at: Set(agora),
            ..Default::default()
        };
        core_credentials::apply(&mut row, &v3_gravada()).unwrap();
        let model = snmp_profiles::Model {
            id: 7,
            name: "core".into(),
            version: row.version.clone().unwrap(),
            community_encrypted: row.community_encrypted.clone().unwrap(),
            username: row.username.clone().unwrap(),
            auth_protocol: row.auth_protocol.clone().unwrap(),
            auth_key_encrypted: row.auth_key_encrypted.clone().unwrap(),
            priv_protocol: row.priv_protocol.clone().unwrap(),
            priv_key_encrypted: row.priv_key_encrypted.clone().unwrap(),
            context_name: None,
            is_default: false,
            created_at: agora,
            updated_at: agora,
        };
        let view = present(&model, 2);
        assert_eq!(view.security_level, "authPriv");
        assert_eq!(view.auth_key.as_deref(), Some(SECRET_MASK));
        assert_eq!(view.priv_key.as_deref(), Some(SECRET_MASK));
        assert!(view.community.is_none());
        let json = serde_json::to_string(&view).unwrap();
        assert!(!json.contains("senha-auth") && !json.contains("senha-priv"));
    }
}
