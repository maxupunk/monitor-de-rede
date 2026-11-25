//! Quem roda as rotinas de fundo quando há mais de um processo (ADR 024).
//!
//! Cada [`LeaderRole`] tem uma trava ([`LeaderLock`]); o processo que a segura
//! é o líder daquela função. Um [`Elector`] por função e por processo tenta
//! pegá-la, confere que continua com ela e, ao perdê-la, avisa pelo `watch`
//! — e [`while_leader`] interrompe a rotina. Se o líder morre, a sessão dele
//! cai, a trava se solta e o próximo processo a pega na tentativa seguinte.
//!
//! **Duas funções, e não uma**, por causa do `SCHEDULER_ENABLED=false`: o
//! servidor que abre mão do ciclo não pode, ao ganhar a eleição das outras
//! rotinas, impedir o processo `task scheduler_loop` de rodar o ciclo.
//!
//! **O que é rotina de líder e o que é dever de cada processo** está
//! registrado no ponto de montagem (`initializers::monitoring`). A regra: é do
//! líder o que age sobre o banco ou o mundo e duplicaria se dois processos o
//! fizessem (ciclo, backups, deploys, tarefas agendadas, IA proativa,
//! atualização de fabricantes, recuperação de execuções interrompidas); é de
//! cada processo o que serve às conexões dele (SSE e snapshots, canais dos
//! agentes, o mapa Wi-Fi aberto nas abas dele, a porta de syslog).
//!
//! No SQLite não há trava: o processo é sempre líder.

use std::{collections::HashMap, future::Future, sync::Arc, time::Duration};

use async_trait::async_trait;
use loco_rs::app::AppContext;
use tokio::sync::watch;

use super::Coordination;
use crate::shared::errors::{AppError, AppResult};

/// As funções que só um processo exerce por vez.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LeaderRole {
    /// O ciclo do scheduler: monitores, rotinas e retenção das áreas,
    /// despacho de notificações, purga e rollup.
    Scheduler,
    /// As demais rotinas de fundo (backups, deploys, tarefas agendadas…).
    Background,
}

impl LeaderRole {
    /// A chave da trava consultiva. Metade alta `NM` (0x4E4D), metade baixa a
    /// função: as duas cabem em 31 bits, sem ambiguidade de sinal no
    /// `pg_locks`.
    #[must_use]
    pub const fn lock_key(self) -> i64 {
        let low = match self {
            Self::Scheduler => 1,
            Self::Background => 2,
        };
        (0x4E4D_i64 << 32) | low
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Scheduler => "ciclo do scheduler",
            Self::Background => "rotinas de fundo",
        }
    }
}

/// A trava de uma função.
#[async_trait]
pub trait LeaderLock: Send + Sync {
    /// Tenta pegar a trava; `true` se ela é deste processo agora.
    async fn try_acquire(&self) -> AppResult<bool>;
    /// A trava continua deste processo? Erro conta como perda.
    async fn still_held(&self) -> AppResult<bool>;
    /// Solta a trava (e a sessão que a segura).
    async fn release(&self);
}

/// Os intervalos do [`Elector`].
#[derive(Debug, Clone, Copy)]
pub struct ElectorTiming {
    /// Entre tentativas de quem não é líder.
    pub retry: Duration,
    /// Entre conferências de quem é líder. Bem abaixo do
    /// `idle_session_timeout` da sessão da trava.
    pub confirm: Duration,
}

impl Default for ElectorTiming {
    fn default() -> Self {
        Self {
            retry: Duration::from_secs(5),
            confirm: Duration::from_secs(5),
        }
    }
}

/// A máquina de estados da liderança de uma função num processo.
pub struct Elector {
    role: LeaderRole,
    lock: Arc<dyn LeaderLock>,
    state: watch::Sender<bool>,
    timing: ElectorTiming,
}

impl Elector {
    #[must_use]
    pub fn new(
        role: LeaderRole,
        lock: Arc<dyn LeaderLock>,
        state: watch::Sender<bool>,
        timing: ElectorTiming,
    ) -> Self {
        Self {
            role,
            lock,
            state,
            timing,
        }
    }

    /// Uma volta: tenta pegar (seguidor) ou confere (líder). Devolve quanto
    /// esperar até a próxima.
    pub async fn step(&self) -> Duration {
        if *self.state.borrow() {
            match self.lock.still_held().await {
                Ok(true) => return self.timing.confirm,
                Ok(false) => {
                    tracing::warn!(funcao = self.role.label(), "trava de liderança perdida");
                }
                Err(error) => {
                    tracing::warn!(%error, funcao = self.role.label(), "sessão da trava de liderança caiu");
                }
            }
            // Para as rotinas antes de qualquer outra coisa: outro processo
            // pode estar assumindo agora.
            self.state.send_replace(false);
            self.lock.release().await;
            return self.timing.retry;
        }
        match self.lock.try_acquire().await {
            Ok(true) => {
                tracing::info!(
                    funcao = self.role.label(),
                    "este processo assumiu a liderança"
                );
                self.state.send_replace(true);
                self.timing.confirm
            }
            Ok(false) => self.timing.retry,
            Err(error) => {
                tracing::warn!(%error, funcao = self.role.label(), "falha ao disputar a liderança");
                self.timing.retry
            }
        }
    }

    /// Roda para sempre.
    pub async fn run(self) {
        loop {
            let wait = self.step().await;
            tokio::time::sleep(wait).await;
        }
    }
}

/// A liderança do processo, no `shared_store`.
#[derive(Clone)]
pub struct Leadership {
    coordination: Coordination,
    timing: ElectorTiming,
    roles: Arc<std::sync::Mutex<HashMap<LeaderRole, watch::Sender<bool>>>>,
}

impl Leadership {
    #[must_use]
    pub fn new(coordination: Coordination, timing: ElectorTiming) -> Self {
        Self {
            coordination,
            timing,
            roles: Arc::default(),
        }
    }

    /// Registra a do processo.
    pub fn install(ctx: &AppContext, coordination: Coordination) {
        ctx.shared_store
            .insert(Self::new(coordination, ElectorTiming::default()));
    }

    /// A do processo; sem ela instalada, o processo é sempre líder.
    #[must_use]
    pub fn from_context(ctx: &AppContext) -> Self {
        ctx.shared_store
            .get::<Self>()
            .unwrap_or_else(|| Self::new(Coordination::Standalone, ElectorTiming::default()))
    }

    /// Acompanha a liderança de uma função. A primeira chamada sobe o
    /// [`Elector`] dela — um processo que nunca pede uma função nunca a
    /// disputa (o servidor com `SCHEDULER_ENABLED=false`, por exemplo).
    /// Precisa de um runtime tokio.
    #[must_use]
    pub fn watch(&self, role: LeaderRole) -> watch::Receiver<bool> {
        let mut roles = self
            .roles
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(state) = roles.get(&role) {
            return state.subscribe();
        }
        let lock = self.coordination.leader_lock(role);
        let (state, receiver) = watch::channel(lock.is_none());
        if let Some(lock) = lock {
            tokio::spawn(Elector::new(role, lock, state.clone(), self.timing).run());
        }
        roles.insert(role, state);
        receiver
    }

    /// Pega a função para **uma** execução avulsa (`task scheduler_run`) e a
    /// solta no fim. Se outro processo é o líder, recusa: rodar o ciclo ao
    /// lado dele executaria os mesmos monitores duas vezes.
    ///
    /// # Errors
    ///
    /// [`AppError::Conflict`] com outro líder; erro do banco ao disputar.
    pub async fn run_once<F, T>(&self, role: LeaderRole, work: F) -> AppResult<T>
    where
        F: Future<Output = AppResult<T>>,
    {
        let Some(lock) = self.coordination.leader_lock(role) else {
            return work.await;
        };
        if !lock.try_acquire().await? {
            lock.release().await;
            return Err(AppError::Conflict(format!(
                "Outro processo da central está rodando o {}; execute-o por lá ou pare-o antes",
                role.label()
            )));
        }
        let outcome = work.await;
        lock.release().await;
        outcome
    }
}

/// Roda `routine` enquanto `leader` disser que este processo é o líder; ao
/// perder a liderança, interrompe a rotina (o futuro é descartado — a
/// transação aberta nele é desfeita) e espera voltar a ser líder para
/// recomeçá-la do início, recuperação de boot incluída.
///
/// Termina quando a rotina termina sozinha.
pub async fn while_leader<F, Fut>(
    mut leader: watch::Receiver<bool>,
    name: &'static str,
    mut routine: F,
) where
    F: FnMut() -> Fut,
    Fut: Future<Output = ()>,
{
    loop {
        if leader.wait_for(|is_leader| *is_leader).await.is_err() {
            return;
        }
        tracing::info!(rotina = name, "rotina iniciada neste processo (líder)");
        let run = routine();
        tokio::pin!(run);
        tokio::select! {
            () = &mut run => return,
            lost = leader.wait_for(|is_leader| !*is_leader) => {
                if lost.is_err() {
                    return;
                }
                tracing::warn!(rotina = name, "liderança perdida: rotina interrompida");
            }
        }
    }
}

/// Sobe `routine` como rotina do líder da função `role`.
pub fn spawn_while_leader<F, Fut>(
    ctx: &AppContext,
    role: LeaderRole,
    name: &'static str,
    routine: F,
) where
    F: Fn(AppContext) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let leader = Leadership::from_context(ctx).watch(role);
    let ctx = ctx.clone();
    tokio::spawn(while_leader(leader, name, move || routine(ctx.clone())));
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Mutex,
    };

    use super::*;

    /// Uma trava em memória disputada por vários "processos".
    #[derive(Default)]
    struct FakeServer {
        holder: Mutex<Option<usize>>,
    }

    struct FakeLock {
        server: Arc<FakeServer>,
        me: usize,
        /// A sessão deste processo caiu (rede, banco reiniciado).
        broken: AtomicBool,
    }

    impl FakeLock {
        fn new(server: &Arc<FakeServer>, me: usize) -> Arc<Self> {
            Arc::new(Self {
                server: Arc::clone(server),
                me,
                broken: AtomicBool::new(false),
            })
        }

        /// A sessão cai: o servidor solta a trava, como o PostgreSQL faz.
        fn drop_session(&self) {
            self.broken.store(true, Ordering::SeqCst);
            let mut holder = self.server.holder.lock().unwrap();
            if *holder == Some(self.me) {
                *holder = None;
            }
        }
    }

    #[async_trait]
    impl LeaderLock for FakeLock {
        async fn try_acquire(&self) -> AppResult<bool> {
            if self.broken.swap(false, Ordering::SeqCst) {
                return Err(AppError::Internal(anyhow::anyhow!("sessão caiu")));
            }
            let mut holder = self.server.holder.lock().unwrap();
            match *holder {
                None => {
                    *holder = Some(self.me);
                    Ok(true)
                }
                Some(current) => Ok(current == self.me),
            }
        }

        async fn still_held(&self) -> AppResult<bool> {
            if self.broken.load(Ordering::SeqCst) {
                return Err(AppError::Internal(anyhow::anyhow!("sessão caiu")));
            }
            Ok(*self.server.holder.lock().unwrap() == Some(self.me))
        }

        async fn release(&self) {
            let mut holder = self.server.holder.lock().unwrap();
            if *holder == Some(self.me) {
                *holder = None;
            }
        }
    }

    fn elector(lock: Arc<FakeLock>) -> (Elector, watch::Receiver<bool>) {
        let (state, receiver) = watch::channel(false);
        (
            Elector::new(
                LeaderRole::Scheduler,
                lock,
                state,
                ElectorTiming {
                    retry: Duration::from_millis(7),
                    confirm: Duration::from_millis(3),
                },
            ),
            receiver,
        )
    }

    #[tokio::test]
    async fn so_um_lider_e_o_outro_assume_quando_ele_cai() {
        let server = Arc::new(FakeServer::default());
        let lock_a = FakeLock::new(&server, 1);
        let (a, leader_a) = elector(Arc::clone(&lock_a));
        let (b, leader_b) = elector(FakeLock::new(&server, 2));

        assert_eq!(a.step().await, Duration::from_millis(3), "líder confere");
        assert_eq!(
            b.step().await,
            Duration::from_millis(7),
            "seguidor tenta de novo"
        );
        assert!(*leader_a.borrow());
        assert!(!*leader_b.borrow());

        // Líder continua líder enquanto a sessão vive.
        a.step().await;
        assert!(*leader_a.borrow());

        // A sessão do líder cai: o servidor solta a trava na hora, o outro
        // processo a pega, e o antigo líder para ao conferir.
        lock_a.drop_session();
        b.step().await;
        assert!(*leader_b.borrow(), "o seguidor assumiu");
        a.step().await;
        assert!(!*leader_a.borrow(), "o antigo líder parou");

        // O antigo líder volta a disputar e não toma a trava de quem a tem.
        a.step().await;
        a.step().await;
        assert!(!*leader_a.borrow());
        assert!(*leader_b.borrow());
    }

    #[tokio::test]
    async fn a_rotina_para_ao_perder_e_recomeca_ao_voltar() {
        let (state, receiver) = watch::channel(false);
        let starts = Arc::new(AtomicUsize::new(0));
        let running = Arc::new(AtomicBool::new(false));
        let supervisor = tokio::spawn(while_leader(receiver, "teste", {
            let starts = Arc::clone(&starts);
            let running = Arc::clone(&running);
            move || {
                let starts = Arc::clone(&starts);
                let running = Arc::clone(&running);
                async move {
                    starts.fetch_add(1, Ordering::SeqCst);
                    running.store(true, Ordering::SeqCst);
                    // Marca que parou quando o futuro é descartado.
                    struct Stop(Arc<AtomicBool>);
                    impl Drop for Stop {
                        fn drop(&mut self) {
                            self.0.store(false, Ordering::SeqCst);
                        }
                    }
                    let _stop = Stop(running);
                    std::future::pending::<()>().await;
                }
            }
        }));

        let settle = || tokio::time::sleep(Duration::from_millis(20));
        settle().await;
        assert_eq!(
            starts.load(Ordering::SeqCst),
            0,
            "seguidor não roda a rotina"
        );

        state.send_replace(true);
        settle().await;
        assert_eq!(starts.load(Ordering::SeqCst), 1);
        assert!(running.load(Ordering::SeqCst));

        state.send_replace(false);
        settle().await;
        assert!(
            !running.load(Ordering::SeqCst),
            "perdeu a liderança e não parou"
        );

        state.send_replace(true);
        settle().await;
        assert_eq!(starts.load(Ordering::SeqCst), 2, "recomeça do início");
        supervisor.abort();
    }

    #[tokio::test]
    async fn sem_trava_o_processo_e_sempre_lider_e_roda_o_avulso() {
        let leadership = Leadership::new(Coordination::Standalone, ElectorTiming::default());
        assert!(*leadership.watch(LeaderRole::Background).borrow());
        let value = leadership
            .run_once(LeaderRole::Scheduler, async { Ok(7) })
            .await
            .unwrap();
        assert_eq!(value, 7);
    }
}
