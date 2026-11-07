//! Registro das fontes vistas — conhecidas e desconhecidas.
//!
//! É o contrapeso da regra "fonte desconhecida não grava". Sem esta lista, o
//! usuário configura o roteador, nada aparece na tela e não há onde olhar: o
//! sistema estaria descartando em silêncio, que é o pior desfecho possível para
//! um recurso de diagnóstico.
//!
//! Vive em memória e zera no restart, de propósito. É um painel de "o que está
//! chegando agora", não histórico — gravar cada fonte no banco custaria uma
//! escrita por linha para responder a uma pergunta que só interessa enquanto
//! alguém está configurando o parque.

use std::{
    collections::HashMap,
    sync::{Mutex, PoisonError},
};

use chrono::{DateTime, Utc};

use super::resolver::Resolution;

/// Teto de fontes lembradas.
///
/// Um remetente que forja o IP de origem faria o mapa crescer sem limite. O
/// despejo é do menos visto: quem mandou uma linha só é ruído, quem manda
/// milhares é o roteador que o usuário está tentando configurar.
const MAX_SOURCES: usize = 1024;

/// Como o resolvedor classificou uma origem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Device,
    Network,
    Ambiguous,
    Unknown,
}

impl From<&Resolution> for SourceKind {
    fn from(resolucao: &Resolution) -> Self {
        match resolucao {
            Resolution::Device(_) => Self::Device,
            Resolution::Network(_) => Self::Network,
            Resolution::Ambiguous(_) => Self::Ambiguous,
            Resolution::Unknown => Self::Unknown,
        }
    }
}

/// O que se sabe de uma origem.
#[derive(Debug, Clone)]
pub struct SeenSource {
    pub source_ip: String,
    /// A chave que a tela devolve ao vincular — o IP, ou `host:<hostname>`
    /// quando a origem está mascarada por NAT. São coisas diferentes de
    /// propósito: atrás do Docker o IP é o mesmo para o parque inteiro, e
    /// vincular por ele atribuiria todos os roteadores ao mesmo dispositivo.
    pub bind_key: String,
    /// Se o endereço de origem é gateway de NAT em vez do remetente real.
    pub masked: bool,
    pub kind: SourceKind,
    pub device_id: Option<i64>,
    /// Candidatos quando o IP existe em mais de um dispositivo — o que a tela
    /// oferece para o *bind* manual.
    pub candidates: Vec<i64>,
    pub hostname: Option<String>,
    pub message_count: u64,
    /// Quantas foram descartadas por não resolver. Só cresce em `Unknown`.
    pub dropped_count: u64,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    /// A última mensagem, para a tela mostrar que tipo de equipamento é.
    pub last_message: String,
}

impl SeenSource {
    /// Se esta origem ainda depende de alguém agir para deixar de ser
    /// descartada.
    ///
    /// Atrás de NAT a resolução por IP é pulada de propósito — o endereço é o
    /// gateway e vale para o parque inteiro —, então uma origem mascarada só
    /// chega a `Device` por vínculo manual ou por casamento de hostname.
    /// Qualquer outra classificação ali significa "ainda falta fazer algo", e é
    /// isso que o aviso da tela pede.
    #[must_use]
    pub fn needs_binding(&self) -> bool {
        self.masked && self.kind != SourceKind::Device
    }
}

/// O registro, compartilhado entre os listeners e a API.
#[derive(Default)]
pub struct SourceRegistry {
    fontes: Mutex<HashMap<String, SeenSource>>,
    /// Confirmações efêmeras aguardadas por ativações em andamento. O valor só
    /// é preenchido pela linha que contém o marcador aleatório daquela sessão.
    confirmacoes: Mutex<HashMap<String, Option<SeenSource>>>,
}

impl SourceRegistry {
    #[must_use]
    pub fn create() -> Self {
        Self::default()
    }

    /// Anota uma linha recebida.
    ///
    /// `masked` diz que o `source_ip` é gateway de NAT, não remetente. Nesse
    /// caso a chave do mapa passa a ser o hostname: sem isso os trinta
    /// roteadores atrás do Docker apareceriam como **uma** linha na tela, e o
    /// operador não teria como vincular um sem vincular todos.
    pub fn record(
        &self,
        source_ip: &str,
        masked: bool,
        resolucao: &Resolution,
        hostname: Option<&str>,
        mensagem: &str,
        agora: DateTime<Utc>,
    ) {
        let kind = SourceKind::from(resolucao);
        let nome = hostname.map(str::trim).filter(|valor| !valor.is_empty());
        let bind_key = match (masked, nome) {
            (true, Some(nome)) => super::resolver::hostname_bind_key(nome),
            // Mascarada e sem hostname: não há o que distinguir. Vira uma linha
            // só, e a tela explica por quê — é o caso em que só `network_mode:
            // host` resolve.
            _ => source_ip.to_owned(),
        };
        let mut fontes = self.fontes.lock().unwrap_or_else(PoisonError::into_inner);

        if fontes.len() >= MAX_SOURCES && !fontes.contains_key(&bind_key) {
            despeja_menos_vista(&mut fontes);
        }

        let entrada = fontes
            .entry(bind_key.clone())
            .or_insert_with(|| SeenSource {
                source_ip: source_ip.to_owned(),
                bind_key,
                masked,
                kind,
                device_id: resolucao.device_id(),
                candidates: Vec::new(),
                hostname: nome.map(str::to_owned),
                message_count: 0,
                dropped_count: 0,
                first_seen_at: agora,
                last_seen_at: agora,
                last_message: String::new(),
            });

        entrada.kind = kind;
        entrada.masked = masked;
        entrada.source_ip = source_ip.to_owned();
        entrada.device_id = resolucao.device_id();
        entrada.candidates = match resolucao {
            Resolution::Ambiguous(ids) => ids.clone(),
            _ => Vec::new(),
        };
        if let Some(nome) = nome {
            entrada.hostname = Some(nome.to_owned());
        }
        entrada.message_count += 1;
        if kind == SourceKind::Unknown {
            entrada.dropped_count += 1;
        }
        entrada.last_seen_at = agora;
        // Truncar aqui, e não na tela: guardar linha de firewall inteira de mil
        // fontes é memória gasta para mostrar as primeiras palavras.
        entrada.last_message = mensagem.chars().take(200).collect();
        let fonte_atual = entrada.clone();
        drop(fontes);

        // O registro acontece no caminho de ingestão, antes que outra mensagem
        // possa substituir `last_message`. Assim a confirmação não perde a
        // linha de teste para um log emitido logo depois pelo próprio logd.
        let mut confirmacoes = self
            .confirmacoes
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        for (marcador, confirmada) in confirmacoes.iter_mut() {
            if confirmada.is_none() && mensagem.contains(marcador) {
                *confirmada = Some(fonte_atual.clone());
            }
        }
    }

    /// Começa a observar uma mensagem correlacionada a uma ativação.
    pub fn expect_confirmation(&self, marker: &str) {
        self.confirmacoes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(marker.to_owned(), None);
    }

    /// Resultado atual, sem encerrar a observação.
    #[must_use]
    pub fn confirmation(&self, marker: &str) -> Option<SeenSource> {
        self.confirmacoes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(marker)
            .cloned()
            .flatten()
    }

    /// Encerra a observação, com sucesso ou timeout.
    pub fn forget_confirmation(&self, marker: &str) {
        self.confirmacoes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(marker);
    }

    /// O que se sabe de uma origem específica.
    ///
    /// Existe para o *bind* manual poder reclassificar sem adivinhar o endereço
    /// e o nome: quando a chave é `host:<nome>`, o IP de origem só está aqui.
    #[must_use]
    pub fn snapshot(&self, bind_key: &str) -> Option<SeenSource> {
        let bind_key = super::resolver::normalize_bind_key(bind_key);
        self.fontes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&bind_key)
            .cloned()
    }

    /// Reaplica uma resolução a uma origem já vista, sem esperar a próxima
    /// mensagem.
    ///
    /// É o que faz o *bind* manual valer **agora**. Sem isto, o registro
    /// continuaria dizendo o que valia quando a última linha chegou: a tela
    /// mostrava "Descartando" e o seletor voltava a vazio logo depois de o
    /// operador escolher o dispositivo — porque a resposta do servidor, honesta,
    /// ainda trazia `deviceId` nulo. Num roteador que fala de hora em hora, o
    /// vínculo parecia simplesmente não ter pegado.
    ///
    /// As contagens **não** são tocadas: as mensagens que foram descartadas
    /// foram descartadas de verdade, e zerar o número apagaria a única pista de
    /// que houve perda antes do vínculo.
    ///
    /// Devolve se a origem existia.
    pub fn reclassify(&self, bind_key: &str, resolucao: &Resolution) -> bool {
        let bind_key = super::resolver::normalize_bind_key(bind_key);
        let mut fontes = self.fontes.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(entrada) = fontes.get_mut(&bind_key) else {
            return false;
        };
        entrada.kind = SourceKind::from(resolucao);
        entrada.device_id = resolucao.device_id();
        entrada.candidates = match resolucao {
            Resolution::Ambiguous(ids) => ids.clone(),
            _ => Vec::new(),
        };
        true
    }

    /// Todas as fontes, da mais recente para a mais antiga.
    #[must_use]
    pub fn list(&self) -> Vec<SeenSource> {
        let fontes = self.fontes.lock().unwrap_or_else(PoisonError::into_inner);
        let mut lista: Vec<SeenSource> = fontes.values().cloned().collect();
        lista.sort_by_key(|fonte| std::cmp::Reverse(fonte.last_seen_at));
        lista
    }

    /// Quantas fontes desconhecidas estão descartando mensagem.
    ///
    /// É o número do banner da tela de logs — a peça que impede a regra de
    /// virar buraco negro de suporte.
    #[must_use]
    pub fn unknown_count(&self) -> usize {
        self.fontes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
            .filter(|fonte| fonte.kind == SourceKind::Unknown)
            .count()
    }

    /// Quantas origens chegam com o endereço reescrito pelo NAT.
    ///
    /// É o gatilho do aviso que explica por que o log "sumiu" depois de subir
    /// no Docker. Sem ele o operador vê fontes desconhecidas e vai tentar
    /// vincular o IP do gateway — que é justamente o que não se pode fazer.
    #[must_use]
    pub fn masked_count(&self) -> usize {
        self.fontes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
            .filter(|fonte| fonte.masked)
            .count()
    }
}

/// Remove a fonte com menos mensagens: ruído sai antes de sinal.
fn despeja_menos_vista(fontes: &mut HashMap<String, SeenSource>) {
    if let Some(chave) = fontes
        .values()
        .min_by_key(|fonte| (fonte.message_count, fonte.last_seen_at))
        .map(|fonte| fonte.bind_key.clone())
    {
        fontes.remove(&chave);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agora() -> DateTime<Utc> {
        Utc::now()
    }

    #[test]
    fn a_fonte_desconhecida_conta_o_que_foi_descartado() {
        let registro = SourceRegistry::create();
        for _ in 0..3 {
            registro.record(
                "203.0.113.9",
                false,
                &Resolution::Unknown,
                None,
                "oi",
                agora(),
            );
        }
        let fontes = registro.list();
        assert_eq!(fontes.len(), 1);
        assert_eq!(fontes[0].kind, SourceKind::Unknown);
        assert_eq!(fontes[0].message_count, 3);
        assert_eq!(fontes[0].dropped_count, 3);
        assert_eq!(registro.unknown_count(), 1);
    }

    #[test]
    fn a_confirmacao_captura_a_linha_antes_que_o_log_seguinte_a_substitua() {
        let registro = SourceRegistry::create();
        registro.expect_confirmation("sessao-123");
        registro.record(
            "172.18.0.1",
            true,
            &Resolution::Unknown,
            Some("roteador-real"),
            "netmonitor: teste de envio de log [sessao-123]",
            agora(),
        );
        registro.record(
            "172.18.0.1",
            true,
            &Resolution::Unknown,
            Some("roteador-real"),
            "log emitido logo depois",
            agora(),
        );

        let confirmada = registro.confirmation("sessao-123").expect("confirmação");
        assert_eq!(confirmada.bind_key, "host:roteador-real");
        assert!(confirmada.last_message.contains("sessao-123"));
        registro.forget_confirmation("sessao-123");
        assert!(registro.confirmation("sessao-123").is_none());
    }

    #[test]
    fn a_fonte_conhecida_nao_conta_descarte() {
        let registro = SourceRegistry::create();
        registro.record(
            "192.168.88.1",
            false,
            &Resolution::Device(7),
            Some("MikroTik"),
            "linha",
            agora(),
        );
        let fontes = registro.list();
        assert_eq!(fontes[0].device_id, Some(7));
        assert_eq!(fontes[0].dropped_count, 0);
        assert_eq!(fontes[0].hostname.as_deref(), Some("MikroTik"));
        assert_eq!(registro.unknown_count(), 0);
    }

    #[test]
    fn a_fonte_ambigua_guarda_os_candidatos_para_o_bind_manual() {
        let registro = SourceRegistry::create();
        registro.record(
            "192.168.1.1",
            false,
            &Resolution::Ambiguous(vec![3, 9]),
            None,
            "linha",
            agora(),
        );
        let fontes = registro.list();
        assert_eq!(fontes[0].kind, SourceKind::Ambiguous);
        assert_eq!(fontes[0].candidates, vec![3, 9]);
        assert_eq!(fontes[0].device_id, None, "ambígua não chuta vínculo");
    }

    #[test]
    fn a_reclassificacao_e_registrada_sem_perder_a_contagem() {
        // O usuário cadastra o dispositivo enquanto o roteador já está
        // mandando: a fonte passa de desconhecida a vinculada.
        let registro = SourceRegistry::create();
        registro.record(
            "192.168.88.1",
            false,
            &Resolution::Unknown,
            None,
            "a",
            agora(),
        );
        registro.record(
            "192.168.88.1",
            false,
            &Resolution::Device(4),
            None,
            "b",
            agora(),
        );
        let fontes = registro.list();
        assert_eq!(fontes[0].kind, SourceKind::Device);
        assert_eq!(fontes[0].message_count, 2);
        assert_eq!(fontes[0].dropped_count, 1, "o que já se perdeu continua lá");
        assert_eq!(registro.unknown_count(), 0);
    }

    #[test]
    fn o_vinculo_manual_vale_antes_da_proxima_mensagem() {
        // O bug que isto fecha: o registro só aprendia com mensagem nova, então
        // logo depois de vincular a tela relia "Descartando" e o seletor voltava
        // a vazio. Num roteador que fala de hora em hora, o vínculo parecia não
        // ter pegado.
        let registro = SourceRegistry::create();
        for _ in 0..9 {
            registro.record(
                "172.24.0.1",
                true,
                &Resolution::Unknown,
                Some("Bpi-r3-borda"),
                "linha",
                agora(),
            );
        }

        let chave = "host:Bpi-r3-borda";
        let fonte = registro.snapshot(chave).expect("a origem foi vista");
        assert_eq!(fonte.source_ip, "172.24.0.1", "o IP real sai daqui");
        assert_eq!(fonte.device_id, None);

        assert!(registro.reclassify(chave, &Resolution::Device(12)));
        let fonte = registro.snapshot(chave).expect("continua lá");
        assert_eq!(fonte.kind, SourceKind::Device);
        assert_eq!(fonte.device_id, Some(12));
        assert_eq!(registro.unknown_count(), 0);

        // As contagens são história e não podem ser reescritas: aquelas nove
        // mensagens foram descartadas de verdade, e zerar o número apagaria a
        // única pista de que houve perda antes do vínculo.
        assert_eq!(fonte.message_count, 9);
        assert_eq!(fonte.dropped_count, 9);
    }

    #[test]
    fn o_desvinculo_devolve_a_origem_ao_que_a_heuristica_disser() {
        // E não a "desconhecida" por decreto: a origem pode cair numa rede
        // cadastrada, e afirmar o contrário seria inventar um descarte.
        let registro = SourceRegistry::create();
        registro.record(
            "192.168.1.50",
            false,
            &Resolution::Device(3),
            None,
            "linha",
            agora(),
        );
        registro.reclassify("192.168.1.50", &Resolution::Network(8));

        let fonte = registro.snapshot("192.168.1.50").expect("na lista");
        assert_eq!(fonte.kind, SourceKind::Network);
        assert_eq!(fonte.device_id, None);
    }

    #[test]
    fn o_aviso_de_nat_cala_quando_todas_as_origens_estao_vinculadas() {
        // O mascaramento não some com o vínculo — mas deixa de custar alguma
        // coisa. Um aviso permanente sobre um problema já resolvido treina o
        // operador a ignorar avisos, e aí ele ignora o que importa.
        let registro = SourceRegistry::create();
        for nome in ["Bpi-r3-borda", "RB922-terraco"] {
            registro.record(
                "172.24.0.1",
                true,
                &Resolution::Unknown,
                Some(nome),
                "linha",
                agora(),
            );
        }
        assert_eq!(
            registro.list().iter().filter(|f| f.needs_binding()).count(),
            2
        );

        registro.reclassify("host:Bpi-r3-borda", &Resolution::Device(1));
        assert_eq!(
            registro.list().iter().filter(|f| f.needs_binding()).count(),
            1,
            "com uma pendente o aviso ainda tem o que dizer"
        );

        registro.reclassify("host:RB922-terraco", &Resolution::Device(2));
        assert_eq!(
            registro.list().iter().filter(|f| f.needs_binding()).count(),
            0
        );
        // E o mascaramento continua sendo verdade: a coluna "Endereço" segue
        // mostrando o gateway, e é o aviso por linha que explica isso.
        assert_eq!(registro.masked_count(), 2);
    }

    #[test]
    fn origem_sem_nat_nunca_pede_vinculo_pelo_aviso() {
        // Fonte desconhecida sem mascaramento é outro problema, com outro aviso
        // — o de origens descartando. Misturar os dois faria o texto do NAT
        // aparecer para quem não está atrás de NAT nenhum.
        let registro = SourceRegistry::create();
        registro.record(
            "203.0.113.9",
            false,
            &Resolution::Unknown,
            None,
            "linha",
            agora(),
        );
        assert!(!registro.list()[0].needs_binding());
        assert_eq!(registro.unknown_count(), 1);
    }

    #[test]
    fn mascarada_sem_nome_continua_pedindo_atencao() {
        // É o caso que só `network_mode: host` resolve, e o aviso é o único
        // lugar onde isso está escrito. Calá-lo aqui esconderia a saída.
        let registro = SourceRegistry::create();
        registro.record("172.24.0.1", true, &Resolution::Unknown, None, "x", agora());
        assert!(registro.list()[0].needs_binding());
    }

    #[test]
    fn reclassificar_origem_que_nunca_chegou_nao_inventa_linha() {
        let registro = SourceRegistry::create();
        assert!(!registro.reclassify("host:fantasma", &Resolution::Device(1)));
        assert!(registro.list().is_empty());
        assert!(registro.snapshot("host:fantasma").is_none());
    }

    #[test]
    fn o_registro_tem_teto_e_despeja_o_ruido_primeiro() {
        let registro = SourceRegistry::create();
        // Uma fonte com volume, muitas com uma linha só.
        for _ in 0..50 {
            registro.record(
                "10.0.0.1",
                false,
                &Resolution::Unknown,
                None,
                "sinal",
                agora(),
            );
        }
        for indice in 0..(MAX_SOURCES + 100) {
            let ip = format!("10.9.{}.{}", indice / 256, indice % 256);
            registro.record(&ip, false, &Resolution::Unknown, None, "ruído", agora());
        }
        let fontes = registro.list();
        assert!(fontes.len() <= MAX_SOURCES, "cresceu para {}", fontes.len());
        assert!(
            fontes.iter().any(|fonte| fonte.source_ip == "10.0.0.1"),
            "a fonte com volume não pode ser despejada pelo ruído"
        );
    }

    #[test]
    fn atras_do_nat_cada_hostname_ganha_a_propria_linha() {
        // O caso do Docker: trinta roteadores, um IP de origem. Colapsar tudo
        // numa linha só deixaria o operador sem como vincular um sem vincular
        // todos.
        let registro = SourceRegistry::create();
        registro.record(
            "172.17.0.1",
            true,
            &Resolution::Device(1),
            Some("MikroTik-Borda"),
            "a",
            agora(),
        );
        registro.record(
            "172.17.0.1",
            true,
            &Resolution::Unknown,
            Some("MikroTik-Filial"),
            "b",
            agora(),
        );

        let fontes = registro.list();
        assert_eq!(fontes.len(), 2, "um IP, dois remetentes");
        assert_eq!(registro.masked_count(), 2);
        for fonte in &fontes {
            assert_eq!(fonte.source_ip, "172.17.0.1", "o IP real continua visível");
            assert!(fonte.masked);
        }
        // A chave de vínculo é o nome, não o endereço — é ela que a tela
        // devolve no POST de bind.
        let mut chaves: Vec<&str> = fontes.iter().map(|f| f.bind_key.as_str()).collect();
        chaves.sort_unstable();
        assert_eq!(chaves, vec!["host:mikrotik-borda", "host:mikrotik-filial"]);
    }

    #[test]
    fn mascarada_sem_hostname_continua_sendo_uma_linha_so() {
        // Roteador que não manda HOSTNAME é indistinguível atrás do NAT.
        // Fingir o contrário inventaria fontes que não existem.
        let registro = SourceRegistry::create();
        for _ in 0..5 {
            registro.record("172.17.0.1", true, &Resolution::Unknown, None, "x", agora());
        }
        let fontes = registro.list();
        assert_eq!(fontes.len(), 1);
        assert_eq!(fontes[0].bind_key, "172.17.0.1");
        assert_eq!(fontes[0].message_count, 5);
        assert!(fontes[0].masked);
    }

    #[test]
    fn fonte_normal_nao_e_contada_como_mascarada() {
        let registro = SourceRegistry::create();
        registro.record(
            "192.168.88.1",
            false,
            &Resolution::Device(7),
            Some("MikroTik"),
            "linha",
            agora(),
        );
        assert_eq!(registro.masked_count(), 0);
        assert_eq!(registro.list()[0].bind_key, "192.168.88.1");
    }

    #[test]
    fn a_mensagem_guardada_e_truncada() {
        let registro = SourceRegistry::create();
        let longa = "x".repeat(5000);
        registro.record(
            "10.0.0.1",
            false,
            &Resolution::Unknown,
            None,
            &longa,
            agora(),
        );
        assert_eq!(registro.list()[0].last_message.chars().count(), 200);
    }
}
