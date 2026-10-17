//! Negociação de motor e papéis de uma perna (ADR 015 §5) — função pura.
//!
//! Cada ponta declara o que sabe **servir** e o que sabe **dirigir**; a central
//! escolhe: prefere `iperf3`, depois `btest`; o servidor é quem sabe servir o
//! motor que o outro sabe dirigir; a autenticação vem do `drive` do cliente.
//! Sem combinação, a perna não roda — e o motivo é dito por extenso.
//!
//! Também mora aqui a tradução de sentido: os motores falam do ponto de vista
//! de quem **dirige** (`Send`/`Receive`, `tx`/`rx`); a perna fala do enlace
//! (`forward` = A→B, `reverse` = B→A).

use crate::services::{
    link_tests::model::{LinkTestEndpointKind, LinkTestFlow, LinkTestSide},
    network_tools::throughput::{Flow, TestDirection, ThroughputEngine},
    plugins::manifest::ThroughputRoles,
};

/// Ordem de preferência dos motores.
pub const ENGINE_PREFERENCE: [ThroughputEngine; 2] =
    [ThroughputEngine::Iperf3, ThroughputEngine::Btest];

/// Um motor que a ponta dirige.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DriveCap {
    pub engine: ThroughputEngine,
    /// `false`: só fala com servidor sem autenticação.
    pub auth: bool,
    /// Teto por sentido, em segundos.
    pub max_duration_s: Option<u8>,
}

/// O que uma ponta sabe fazer num teste.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Capabilities {
    pub serve: Vec<ThroughputEngine>,
    pub drive: Vec<DriveCap>,
}

impl Capabilities {
    /// A central: serve btest (servidor nativo armado) e dirige iperf3
    /// (cliente nativo). Não dirige btest: contra um MikroTik, é ele quem
    /// conecta no servidor da central (ADR 015, tabela de ferramentas).
    #[must_use]
    pub fn central() -> Self {
        Self {
            serve: vec![ThroughputEngine::Btest],
            drive: vec![DriveCap {
                engine: ThroughputEngine::Iperf3,
                auth: false,
                max_duration_s: None,
            }],
        }
    }

    /// Um agente: só dirige (nunca abre porta) — iperf3 e btest sem
    /// autenticação.
    #[must_use]
    pub fn agent() -> Self {
        Self {
            serve: Vec::new(),
            drive: vec![
                DriveCap {
                    engine: ThroughputEngine::Iperf3,
                    auth: false,
                    max_duration_s: None,
                },
                DriveCap {
                    engine: ThroughputEngine::Btest,
                    auth: false,
                    max_duration_s: None,
                },
            ],
        }
    }

    /// Os papéis declarados no manifesto de um plugin.
    #[must_use]
    pub fn from_roles(roles: &ThroughputRoles) -> Self {
        Self {
            serve: roles.serve.iter().map(|serve| serve.engine).collect(),
            drive: roles
                .drive
                .iter()
                .map(|drive| DriveCap {
                    engine: drive.engine,
                    auth: drive.auth,
                    max_duration_s: drive.max_duration_s,
                })
                .collect(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.serve.is_empty() && self.drive.is_empty()
    }

    #[must_use]
    pub fn serves(&self, engine: ThroughputEngine) -> bool {
        self.serve.contains(&engine)
    }

    #[must_use]
    pub fn driver(&self, engine: ThroughputEngine) -> Option<DriveCap> {
        self.drive
            .iter()
            .copied()
            .find(|drive| drive.engine == engine)
    }

    /// "serve btest e dirige iperf3, btest", para o motivo da recusa.
    #[must_use]
    pub fn describe(&self) -> String {
        let list = |engines: Vec<ThroughputEngine>| {
            engines
                .into_iter()
                .map(ThroughputEngine::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        };
        let serve = list(self.serve.clone());
        let drive = list(self.drive.iter().map(|drive| drive.engine).collect());
        match (serve.is_empty(), drive.is_empty()) {
            (true, true) => "não serve nem dirige teste algum".to_owned(),
            (false, true) => format!("só serve {serve}"),
            (true, false) => format!("só dirige {drive}"),
            (false, false) => format!("serve {serve} e dirige {drive}"),
        }
    }
}

/// Uma ponta, como a negociação a vê.
#[derive(Debug, Clone, Copy)]
pub struct Party<'a> {
    pub kind: LinkTestEndpointKind,
    pub name: &'a str,
    pub caps: &'a Capabilities,
}

/// O que ficou combinado para a perna.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Negotiation {
    pub engine: ThroughputEngine,
    /// Quem serve; o outro lado dirige.
    pub server: LinkTestSide,
    /// O servidor (um equipamento) exige usuário e senha do teste.
    pub require_auth: bool,
    /// Duração de cada sentido, já limitada pelo teto do cliente.
    pub duration_s: u8,
    pub warnings: Vec<String>,
}

impl Negotiation {
    /// Quem dirige.
    #[must_use]
    pub const fn driver(&self) -> LinkTestSide {
        self.server.other()
    }
}

/// Escolhe motor e papéis para o par `(a, b)`.
///
/// # Errors
///
/// O motivo, em português, quando nenhuma combinação serve.
pub fn negotiate(a: Party<'_>, b: Party<'_>, duration_s: u8) -> Result<Negotiation, String> {
    for engine in ENGINE_PREFERENCE {
        let viable: Vec<LinkTestSide> = [LinkTestSide::A, LinkTestSide::B]
            .into_iter()
            .filter(|server| {
                let (serving, driving) = pick(*server, a, b);
                serving.caps.serves(engine) && driving.caps.driver(engine).is_some()
            })
            .collect();
        let Some(server) = choose_server(&viable, a, b) else {
            continue;
        };
        let (serving, driving) = pick(server, a, b);
        let Some(drive) = driving.caps.driver(engine) else {
            continue;
        };
        let mut warnings = Vec::new();
        let duration_s = match drive.max_duration_s {
            Some(max) if duration_s > max => {
                warnings.push(format!(
                    "{} aceita no máximo {max} s por sentido com {}: a duração foi limitada a {max} s",
                    driving.name,
                    engine.as_str()
                ));
                max
            }
            _ => duration_s,
        };
        return Ok(Negotiation {
            engine,
            server,
            // Só um equipamento servidor pede usuário e senha; o servidor
            // btest da central responde "sem autenticação".
            require_auth: engine == ThroughputEngine::Btest
                && drive.auth
                && serving.kind == LinkTestEndpointKind::Device,
            duration_s,
            warnings,
        });
    }
    Err(no_combination(a, b))
}

/// `(quem serve, quem dirige)` para o lado servidor dado.
const fn pick<'a>(server: LinkTestSide, a: Party<'a>, b: Party<'a>) -> (Party<'a>, Party<'a>) {
    match server {
        LinkTestSide::A => (a, b),
        LinkTestSide::B => (b, a),
    }
}

/// Entre os lados viáveis: o equipamento serve quando o outro é a central ou
/// um agente (eles dirigem); entre dois equipamentos, B serve e A dirige.
fn choose_server(viable: &[LinkTestSide], a: Party<'_>, b: Party<'_>) -> Option<LinkTestSide> {
    match viable {
        [] => None,
        [only] => Some(*only),
        _ => {
            let a_device = a.kind == LinkTestEndpointKind::Device;
            let b_device = b.kind == LinkTestEndpointKind::Device;
            Some(if a_device && !b_device {
                LinkTestSide::A
            } else {
                LinkTestSide::B
            })
        }
    }
}

fn no_combination(a: Party<'_>, b: Party<'_>) -> String {
    let missing: Vec<String> = [a, b]
        .into_iter()
        .filter(|party| party.caps.is_empty())
        .map(|party| {
            format!(
                "{} não tem plugin de teste de enlace ativo e instalado",
                party.name
            )
        })
        .collect();
    if !missing.is_empty() {
        return missing.join("; ");
    }
    format!(
        "Sem motor em comum: {} {}; {} {}",
        a.name,
        a.caps.describe(),
        b.name,
        b.caps.describe()
    )
}

/// Sentido que quem dirige pede ao motor para medir um sentido do enlace.
#[must_use]
pub const fn driver_direction(driver: LinkTestSide, flow: LinkTestFlow) -> TestDirection {
    match (driver, flow) {
        (LinkTestSide::A, LinkTestFlow::Forward) | (LinkTestSide::B, LinkTestFlow::Reverse) => {
            TestDirection::Send
        }
        (LinkTestSide::A, LinkTestFlow::Reverse) | (LinkTestSide::B, LinkTestFlow::Forward) => {
            TestDirection::Receive
        }
    }
}

/// Sentido do enlace de uma amostra do motor (`tx`/`rx` de quem dirige).
#[must_use]
pub const fn link_flow(driver: LinkTestSide, flow: Flow) -> LinkTestFlow {
    match (driver, flow) {
        (LinkTestSide::A, Flow::Tx) | (LinkTestSide::B, Flow::Rx) => LinkTestFlow::Forward,
        (LinkTestSide::A, Flow::Rx) | (LinkTestSide::B, Flow::Tx) => LinkTestFlow::Reverse,
    }
}

/// `(forward, reverse)` a partir de `(tx, rx)` de quem dirige.
#[must_use]
pub fn link_rates<T>(driver: LinkTestSide, tx: T, rx: T) -> (T, T) {
    match driver {
        LinkTestSide::A => (tx, rx),
        LinkTestSide::B => (rx, tx),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drive(engine: ThroughputEngine, auth: bool) -> DriveCap {
        DriveCap {
            engine,
            auth,
            max_duration_s: None,
        }
    }

    /// RouterOS: btest nos dois papéis, com autenticação.
    fn mikrotik() -> Capabilities {
        Capabilities {
            serve: vec![ThroughputEngine::Btest],
            drive: vec![drive(ThroughputEngine::Btest, true)],
        }
    }

    /// OpenWrt: iperf3 nos dois papéis e o cliente `mikrotik-btest`.
    fn openwrt() -> Capabilities {
        Capabilities {
            serve: vec![ThroughputEngine::Iperf3],
            drive: vec![
                drive(ThroughputEngine::Iperf3, true),
                DriveCap {
                    engine: ThroughputEngine::Btest,
                    auth: false,
                    max_duration_s: Some(29),
                },
            ],
        }
    }

    /// Linux: iperf3 nos dois papéis.
    fn linux() -> Capabilities {
        Capabilities {
            serve: vec![ThroughputEngine::Iperf3],
            drive: vec![drive(ThroughputEngine::Iperf3, true)],
        }
    }

    fn device<'a>(name: &'a str, caps: &'a Capabilities) -> Party<'a> {
        Party {
            kind: LinkTestEndpointKind::Device,
            name,
            caps,
        }
    }

    fn ok(a: Party<'_>, b: Party<'_>) -> Negotiation {
        negotiate(a, b, 5).expect("combinação")
    }

    #[test]
    fn mikrotik_com_mikrotik_usa_btest_com_autenticacao() {
        let (x, y) = (mikrotik(), mikrotik());
        let n = ok(device("R1", &x), device("R2", &y));
        assert_eq!(n.engine, ThroughputEngine::Btest);
        assert_eq!(n.server, LinkTestSide::B, "B serve, A dirige");
        assert_eq!(n.driver(), LinkTestSide::A);
        assert!(n.require_auth);
        assert_eq!(n.duration_s, 5);
    }

    #[test]
    fn openwrt_com_openwrt_usa_iperf3() {
        let (x, y) = (openwrt(), openwrt());
        let n = ok(device("O1", &x), device("O2", &y));
        assert_eq!(n.engine, ThroughputEngine::Iperf3);
        assert_eq!(n.server, LinkTestSide::B);
        assert!(!n.require_auth, "iperf3 não autentica");
    }

    #[test]
    fn openwrt_com_mikrotik_usa_btest_sem_autenticacao() {
        let (o, m) = (openwrt(), mikrotik());
        // OpenWrt em A: ele dirige (mikrotik-btest) e o MikroTik serve.
        let n = ok(device("O1", &o), device("R1", &m));
        assert_eq!(n.engine, ThroughputEngine::Btest);
        assert_eq!(n.server, LinkTestSide::B);
        assert!(!n.require_auth, "o mikrotik-btest só fala sem autenticação");
        // Invertido: o servidor continua sendo o MikroTik.
        let n = ok(device("R1", &m), device("O1", &o));
        assert_eq!(n.server, LinkTestSide::A);
        assert!(!n.require_auth);
    }

    #[test]
    fn teto_do_cliente_limita_a_duracao_e_avisa() {
        let (o, m) = (openwrt(), mikrotik());
        let n = negotiate(device("O1", &o), device("R1", &m), 45).unwrap();
        assert_eq!(n.duration_s, 29);
        assert!(n.warnings[0].contains("no máximo 29 s"), "{:?}", n.warnings);
        let n = negotiate(device("O1", &o), device("R1", &m), 10).unwrap();
        assert_eq!(n.duration_s, 10);
        assert!(n.warnings.is_empty());
    }

    #[test]
    fn linux_com_openwrt_usa_iperf3() {
        let (l, o) = (linux(), openwrt());
        let n = ok(device("L1", &l), device("O1", &o));
        assert_eq!(n.engine, ThroughputEngine::Iperf3);
        assert_eq!(n.server, LinkTestSide::B);
    }

    #[test]
    fn central_com_mikrotik_arma_o_btest_da_central() {
        let (c, m) = (Capabilities::central(), mikrotik());
        let central = Party {
            kind: LinkTestEndpointKind::Central,
            name: "Servidor",
            caps: &c,
        };
        let n = ok(central, device("R1", &m));
        assert_eq!(n.engine, ThroughputEngine::Btest);
        assert_eq!(n.server, LinkTestSide::A, "a central serve");
        assert!(!n.require_auth, "o servidor nativo não autentica");
    }

    #[test]
    fn central_com_openwrt_dirige_iperf3() {
        let (c, o) = (Capabilities::central(), openwrt());
        let central = Party {
            kind: LinkTestEndpointKind::Central,
            name: "Servidor",
            caps: &c,
        };
        let n = ok(central, device("O1", &o));
        assert_eq!(n.engine, ThroughputEngine::Iperf3);
        assert_eq!(n.server, LinkTestSide::B, "o equipamento serve");
        assert_eq!(n.driver(), LinkTestSide::A);
    }

    #[test]
    fn agente_com_mikrotik_dirige_btest_sem_autenticacao() {
        let (g, m) = (Capabilities::agent(), mikrotik());
        let agent = Party {
            kind: LinkTestEndpointKind::Agent,
            name: "Agente Filial",
            caps: &g,
        };
        let n = ok(agent, device("R1", &m));
        assert_eq!(n.engine, ThroughputEngine::Btest);
        assert_eq!(n.server, LinkTestSide::B);
        assert!(!n.require_auth, "o cliente nativo só fala sem autenticação");
    }

    #[test]
    fn agente_com_openwrt_dirige_iperf3() {
        let (g, o) = (Capabilities::agent(), openwrt());
        let agent = Party {
            kind: LinkTestEndpointKind::Agent,
            name: "Agente",
            caps: &g,
        };
        let n = ok(device("O1", &o), agent);
        assert_eq!(n.engine, ThroughputEngine::Iperf3);
        assert_eq!(
            n.server,
            LinkTestSide::A,
            "o equipamento serve, o agente dirige"
        );
    }

    #[test]
    fn dois_equipamentos_que_servem_e_dirigem_preferem_b_servidor() {
        let (x, y) = (linux(), linux());
        assert_eq!(
            ok(device("L1", &x), device("L2", &y)).server,
            LinkTestSide::B
        );
    }

    #[test]
    fn sem_combinacao_explica_o_motivo() {
        let (m, l) = (mikrotik(), linux());
        let erro = negotiate(device("R1", &m), device("L1", &l), 5).unwrap_err();
        assert!(erro.contains("Sem motor em comum"), "{erro}");
        assert!(erro.contains("R1 serve btest e dirige btest"), "{erro}");
        assert!(erro.contains("L1 serve iperf3 e dirige iperf3"), "{erro}");

        let vazio = Capabilities::default();
        let erro = negotiate(device("R1", &m), device("Switch", &vazio), 5).unwrap_err();
        assert_eq!(
            erro,
            "Switch não tem plugin de teste de enlace ativo e instalado"
        );

        // Dois agentes/centrais não se medem: ninguém serve.
        let (g1, g2) = (Capabilities::agent(), Capabilities::agent());
        let erro = negotiate(
            Party {
                kind: LinkTestEndpointKind::Agent,
                name: "G1",
                caps: &g1,
            },
            Party {
                kind: LinkTestEndpointKind::Agent,
                name: "G2",
                caps: &g2,
            },
            5,
        )
        .unwrap_err();
        assert!(erro.contains("G1 só dirige iperf3, btest"), "{erro}");
    }

    #[test]
    fn traduz_sentido_do_motor_para_o_enlace() {
        use LinkTestFlow::{Forward, Reverse};
        use LinkTestSide::{A, B};
        assert_eq!(driver_direction(A, Forward), TestDirection::Send);
        assert_eq!(driver_direction(A, Reverse), TestDirection::Receive);
        assert_eq!(driver_direction(B, Forward), TestDirection::Receive);
        assert_eq!(driver_direction(B, Reverse), TestDirection::Send);
        assert_eq!(link_flow(A, Flow::Tx), Forward);
        assert_eq!(link_flow(A, Flow::Rx), Reverse);
        assert_eq!(link_flow(B, Flow::Tx), Reverse);
        assert_eq!(link_flow(B, Flow::Rx), Forward);
        assert_eq!(link_rates(A, 1, 2), (1, 2));
        assert_eq!(link_rates(B, 1, 2), (2, 1));
        // Ida e volta são coerentes: o sentido pedido volta como amostra do
        // mesmo sentido do enlace.
        for driver in [A, B] {
            for flow in [Forward, Reverse] {
                let sample = if driver_direction(driver, flow) == TestDirection::Send {
                    Flow::Tx
                } else {
                    Flow::Rx
                };
                assert_eq!(link_flow(driver, sample), flow);
            }
        }
    }
}
