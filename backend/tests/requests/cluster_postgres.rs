//! Vários processos da central contra um PostgreSQL de verdade (ADR 025).
//!
//! O que o SQLite da suíte não prova: a entrega por `LISTEN`/`NOTIFY` no
//! commit, o snapshot atravessando de um barramento para outro, o aviso grande
//! pela linha do outbox, o `resync` de quem chega depois e a troca de líder da
//! trava consultiva quando a sessão do líder cai.
//!
//! Cada "processo" é um pool próprio sobre o mesmo banco — conexões, escuta e
//! origem separadas, como dois `backend-cli start`. Roda só com
//! `NM_TEST_POSTGRES_URL` (um servidor descartável; o teste cria e apaga um
//! banco próprio nele) e `--ignored`.

use std::{future::Future, time::Duration};

use backend::migrator::Migrator;
use netmonitor_core::migrations::MigratorTrait;
use netmonitor_core::{
    cluster::{
        advisory::PgAdvisoryLock,
        leadership::{Elector, ElectorTiming, LeaderRole, Leadership},
        Coordination,
    },
    events::EventBus,
    models::{_entities::event_outbox as event_outbox_entity, event_outbox},
    shared::errors::AppError,
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, Database, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter,
};
use std::sync::Arc;
use tokio::sync::watch;

const SKIP: &str = "NM_TEST_POSTGRES_URL ausente: teste de PostgreSQL pulado";

/// Um banco novo no servidor de teste, migrado, e o nome dele.
struct Banco {
    admin: DatabaseConnection,
    url: String,
    nome: String,
}

impl Banco {
    async fn criar(sufixo: &str) -> Option<Self> {
        let Ok(base) = std::env::var("NM_TEST_POSTGRES_URL") else {
            eprintln!("{SKIP}");
            return None;
        };
        let admin = Database::connect(&base)
            .await
            .expect("conectar no PostgreSQL");
        let nome = format!("nm_cluster_{sufixo}_{}", std::process::id());
        admin
            .execute_unprepared(&format!("DROP DATABASE IF EXISTS {nome} WITH (FORCE)"))
            .await
            .expect("limpar banco antigo");
        admin
            .execute_unprepared(&format!("CREATE DATABASE {nome}"))
            .await
            .expect("criar banco de teste");
        let (raiz, _) = base.rsplit_once('/').expect("URL com o banco no fim");
        let url = format!("{raiz}/{nome}");
        let db = Database::connect(&url)
            .await
            .expect("conectar no banco de teste");
        Migrator::up(&db, None)
            .await
            .expect("migrar o banco de teste");
        db.close().await.ok();
        Some(Self { admin, url, nome })
    }

    /// Um "processo": pool próprio.
    async fn processo(&self) -> DatabaseConnection {
        Database::connect(&self.url)
            .await
            .expect("conectar o processo")
    }

    async fn apagar(self) {
        let _ = self
            .admin
            .execute_unprepared(&format!(
                "DROP DATABASE IF EXISTS {} WITH (FORCE)",
                self.nome
            ))
            .await;
    }
}

/// Espera a condição virar verdade (teste: a entrega é assíncrona).
async fn ate<F, Fut>(descricao: &str, mut condicao: F)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = bool>,
{
    let limite = tokio::time::Instant::now() + Duration::from_secs(10);
    while tokio::time::Instant::now() < limite {
        if condicao().await {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("não aconteceu em 10 s: {descricao}");
}

fn tem_snapshot(bus: &EventBus, tipo: &str) -> bool {
    bus.current_snapshots()
        .iter()
        .any(|event| event.event_type == tipo)
}

/// O `resync` de quem começa a escutar só sai depois do `LISTEN`; um aviso
/// que A mande nesse instante é o que se quer evitar no teste, não no código.
async fn escuta_de_pe() {
    tokio::time::sleep(Duration::from_millis(500)).await;
}

#[tokio::test]
#[ignore = "precisa de PostgreSQL em NM_TEST_POSTGRES_URL"]
async fn eventos_e_snapshots_atravessam_processos_pelo_notify() {
    let Some(banco) = Banco::criar("eventos").await else {
        return;
    };
    let db_a = banco.processo().await;
    let db_b = banco.processo().await;
    let bus_a = Coordination::for_database(&db_a).start_event_bus();
    let bus_b = Coordination::for_database(&db_b).start_event_bus();
    let mut aba_a = bus_a.subscribe();
    let mut aba_b = bus_b.subscribe();
    escuta_de_pe().await;

    // Evento de domínio: linha + NOTIFY no commit; B recebe, A não em dobro.
    bus_a
        .publish(&db_a, "device:status", serde_json::json!({ "id": 7 }))
        .await
        .expect("publicar");
    let recebido = tokio::time::timeout(Duration::from_secs(10), aba_b.recv())
        .await
        .expect("B recebeu a tempo")
        .expect("canal aberto");
    assert_eq!(recebido.event_type, "device:status");
    assert_eq!(recebido.payload["id"], 7);
    assert_eq!(aba_a.recv().await.unwrap().event_type, "device:status");
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        aba_a.try_recv().is_err(),
        "A recebeu o próprio evento de volta pelo NOTIFY"
    );

    // Efêmero: só o aviso, sem linha.
    bus_a.publish_ephemeral("deploy:log", serde_json::json!({ "lines": ["ok"] }));
    let efemero = tokio::time::timeout(Duration::from_secs(10), aba_b.recv())
        .await
        .expect("efêmero a tempo")
        .unwrap();
    assert_eq!(efemero.event_type, "deploy:log");

    // Snapshot pequeno: chega ao cache de B, como snapshot.
    bus_a.publish_snapshot("vpn:peers", "vpn-1", serde_json::json!({ "peers": 3 }));
    ate("snapshot da VPN no cache de B", || async {
        tem_snapshot(&bus_b, "vpn:peers")
    })
    .await;

    // Snapshot grande demais para o NOTIFY: vai pela linha, com a chave.
    let grande = "x".repeat(20_000);
    bus_a.publish_snapshot("wifi:ap", "ap-9", serde_json::json!({ "blob": grande }));
    ate("snapshot grande no cache de B", || async {
        bus_b
            .current_snapshots()
            .iter()
            .any(|event| event.event_type == "wifi:ap" && event.payload["blob"] == grande)
    })
    .await;
    let linhas = event_outbox::Entity::find()
        .filter(event_outbox_entity::Column::SnapshotKey.eq("ap-9"))
        .count(&db_a)
        .await
        .unwrap();
    assert_eq!(linhas, 1, "o snapshot grande precisa da linha");

    // Um processo que sobe depois recebe o estado atual pelo `resync`.
    let db_c = banco.processo().await;
    let bus_c = Coordination::for_database(&db_c).start_event_bus();
    ate(
        "C recebeu os snapshots de A ao começar a escutar",
        || async { tem_snapshot(&bus_c, "vpn:peers") && tem_snapshot(&bus_c, "wifi:ap") },
    )
    .await;

    // Esquecer em A esquece em B e C.
    bus_a.forget_snapshots("vpn-1");
    ate("B e C esqueceram a VPN", || async {
        !tem_snapshot(&bus_b, "vpn:peers") && !tem_snapshot(&bus_c, "vpn:peers")
    })
    .await;
    assert!(
        tem_snapshot(&bus_b, "wifi:ap"),
        "esqueceu chave que não devia"
    );

    for db in [db_a, db_b, db_c] {
        db.close().await.ok();
    }
    banco.apagar().await;
}

fn eleitor(lock: Arc<PgAdvisoryLock>) -> (Elector, watch::Receiver<bool>) {
    let (estado, receptor) = watch::channel(false);
    (
        Elector::new(
            LeaderRole::Scheduler,
            lock,
            estado,
            ElectorTiming {
                retry: Duration::from_millis(100),
                confirm: Duration::from_millis(100),
            },
        ),
        receptor,
    )
}

#[tokio::test]
#[ignore = "precisa de PostgreSQL em NM_TEST_POSTGRES_URL"]
async fn so_um_lider_e_outro_assume_quando_a_sessao_dele_cai() {
    let Some(banco) = Banco::criar("lider").await else {
        return;
    };
    let db_a = banco.processo().await;
    let db_b = banco.processo().await;
    let chave = LeaderRole::Scheduler.lock_key();
    let (a, lider_a) = eleitor(Arc::new(PgAdvisoryLock::new(db_a.clone(), chave)));
    let (b, lider_b) = eleitor(Arc::new(PgAdvisoryLock::new(db_b.clone(), chave)));

    a.step().await;
    b.step().await;
    assert!(*lider_a.borrow(), "A pegou a trava");
    assert!(!*lider_b.borrow(), "B não pode pegar a trava de A");
    a.step().await;
    b.step().await;
    assert!(
        *lider_a.borrow() && !*lider_b.borrow(),
        "nada muda com A vivo"
    );

    // O avulso (`task scheduler_run`) recusa com outro líder vivo.
    let avulso = Leadership::new(Coordination::for_database(&db_b), ElectorTiming::default())
        .run_once(LeaderRole::Scheduler, async { Ok(()) })
        .await;
    assert!(matches!(avulso, Err(AppError::Conflict(_))), "{avulso:?}");

    // A sessão de A cai (processo morto, rede): o PostgreSQL solta a trava.
    let derrubadas = db_b
        .execute_unprepared(&format!(
            "SELECT pg_terminate_backend(pid) FROM pg_locks \
              WHERE locktype = 'advisory' AND granted AND objid = {}",
            chave & 0xFFFF_FFFF
        ))
        .await
        .unwrap()
        .rows_affected();
    assert_eq!(derrubadas, 1, "uma sessão segurava a trava");

    ate("B assumiu", || async {
        b.step().await;
        *lider_b.borrow()
    })
    .await;
    a.step().await;
    assert!(!*lider_a.borrow(), "A percebeu que perdeu a trava");
    a.step().await;
    a.step().await;
    assert!(!*lider_a.borrow(), "A não toma a trava de B");

    // Sem líder, o avulso pega a função, roda e a solta.
    drop(b);
    db_b.close().await.ok();
    let db_c = banco.processo().await;
    let (c, lider_c) = eleitor(Arc::new(PgAdvisoryLock::new(db_c.clone(), chave)));
    ate("a trava de B foi solta com a sessão dele", || async {
        c.step().await;
        *lider_c.borrow()
    })
    .await;

    for db in [db_a, db_c] {
        db.close().await.ok();
    }
    banco.apagar().await;
}
