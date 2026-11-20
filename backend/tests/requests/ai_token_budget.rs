//! Quanto o chat manda ao provedor.
//!
//! Um "oi" já custou 22 mil tokens: o método mandava começar consultando e o
//! catálogo inteiro ia em cada uma das quatro rodadas. O que se afirma aqui:
//! cortesia vai sem ferramenta e com prompt curto; pergunta de verdade leva só
//! o núcleo e o que ela obviamente pede, e o resto chega com `load_tools`, sem
//! aparecer na tela; rotina automática continua com o catálogo inteiro.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use backend::app::App;
use futures::StreamExt;
use loco_rs::{prelude::AppContext, testing::prelude::*};
use netmonitor_assistant::{
    dtos::{ChatMessageInput, TestConnectionResponse},
    {
        decider::config::AiDeciderSettings,
        drivers::traits::{
            AiChatChunk, AiChatOptions, AiChunkStream, AiDriver, AiMessage, AiTool, AiToolCall,
        },
        harness::{
            agent::{run_agent_loop, AgentRequest, ConversationMemory, HarnessEvent, ToolLoading},
            prompt::ChatContext,
            tools::{ToolPolicy, LOAD_TOOLS},
        },
        settings::{AiAutomationMode, AiSettings},
    },
};
use netmonitor_nucleo::services::shared::errors::AppResult;
use serde_json::json;
use serial_test::serial;
use tokio::sync::oneshot;

use super::fake_ollaya::{self, by_keyword, with_key};

/// O que o provedor recebeu numa rodada.
#[derive(Debug, Clone)]
struct Rodada {
    ferramentas: Vec<String>,
    prompt: String,
}

/// Responde o roteiro, uma resposta por rodada, e grava cada pedido.
struct Gravador {
    roteiro: Mutex<Vec<AiChatChunk>>,
    rodadas: Arc<Mutex<Vec<Rodada>>>,
}

impl Gravador {
    fn novo(roteiro: Vec<AiChatChunk>) -> (Self, Arc<Mutex<Vec<Rodada>>>) {
        let rodadas = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                roteiro: Mutex::new(roteiro.into_iter().rev().collect()),
                rodadas: rodadas.clone(),
            },
            rodadas,
        )
    }
}

fn texto(conteudo: &str) -> AiChatChunk {
    AiChatChunk {
        text_delta: Some(conteudo.into()),
        ..AiChatChunk::default()
    }
}

fn chamada(nome: &str, argumentos: serde_json::Value) -> AiChatChunk {
    AiChatChunk {
        tool_calls: vec![AiToolCall {
            id: format!("c-{nome}"),
            name: nome.into(),
            arguments: argumentos.to_string(),
        }],
        ..AiChatChunk::default()
    }
}

#[async_trait]
impl AiDriver for Gravador {
    fn id(&self) -> &'static str {
        "fake"
    }

    fn display_name(&self) -> &'static str {
        "Fake"
    }

    async fn chat_stream(
        &self,
        messages: &[AiMessage],
        tools: &[AiTool],
        _options: AiChatOptions,
    ) -> AppResult<AiChunkStream> {
        self.rodadas.lock().unwrap().push(Rodada {
            ferramentas: tools
                .iter()
                .map(|tool| tool.function.name.clone())
                .collect(),
            prompt: messages[0].content.clone().unwrap_or_default(),
        });
        let resposta = self
            .roteiro
            .lock()
            .unwrap()
            .pop()
            .unwrap_or_else(|| texto("fim"));
        Ok(Box::pin(futures::stream::iter(vec![Ok(resposta)])))
    }

    async fn test_connection(&self) -> AppResult<TestConnectionResponse> {
        Ok(TestConnectionResponse {
            success: true,
            latency_ms: 0.0,
            message: String::new(),
            model: None,
        })
    }
}

async fn conversar(
    ctx: &AppContext,
    pergunta: &str,
    roteiro: Vec<AiChatChunk>,
    carga: ToolLoading,
) -> (Vec<Rodada>, Vec<HarnessEvent>) {
    let (driver, rodadas) = Gravador::novo(roteiro);
    // O Decisor de teste só reconhece gráfico; o resto fica para o `load_tools`.
    with_key();
    let decisor = fake_ollaya::start(by_keyword(&[("gráfico", "charts")])).await;
    let settings = AiSettings {
        decider: AiDeciderSettings {
            enabled: true,
            base_url: decisor.base_url.clone(),
            model: "laya".into(),
            timeout_ms: 5_000,
            ..AiDeciderSettings::default()
        },
        // Modelo genérico: é ele que passa pelo roteamento do Decisor (o
        // padrão, `lfm3-netmonitor`, é treinado e recebe o prompt enxuto).
        ollama_model: Some("qwen2.5:7b".into()),
        ..AiSettings::default()
    };
    let eventos: Vec<HarnessEvent> = run_agent_loop(
        ctx.clone(),
        settings.clone(),
        Box::new(driver),
        AgentRequest {
            messages: vec![ChatMessageInput {
                role: "user".into(),
                content: pergunta.into(),
            }],
            context: ChatContext::default(),
            policy: ToolPolicy::from_settings(&settings),
            tool_loading: carga,
            memory: ConversationMemory::default(),
            actor: netmonitor_nucleo::services::audit::AuditActor::default(),
        },
    )
    .await
    .collect()
    .await;
    let rodadas = rodadas.lock().unwrap().clone();
    (rodadas, eventos)
}

#[tokio::test]
#[serial]
async fn oi_vai_sem_ferramenta_e_com_prompt_curto() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let (rodadas, eventos) = conversar(
            &ctx,
            "Oi",
            vec![texto("Oi! Em que posso ajudar?")],
            ToolLoading::OnDemand,
        )
        .await;

        assert_eq!(rodadas.len(), 1, "uma rodada só");
        assert!(
            rodadas[0].ferramentas.is_empty(),
            "{:?}",
            rodadas[0].ferramentas
        );
        assert!(
            rodadas[0].prompt.len() < 600,
            "prompt de cortesia sem método nem catálogo: {} caracteres",
            rodadas[0].prompt.len()
        );
        assert!(!eventos
            .iter()
            .any(|evento| matches!(evento, HarnessEvent::ToolCall { .. })));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn pergunta_leva_o_nucleo_e_carrega_o_resto_sob_demanda() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let roteiro = vec![
            chamada(LOAD_TOOLS, json!({ "groups": ["charts"] })),
            texto("Pronto."),
        ];
        let (rodadas, eventos) = conversar(
            &ctx,
            "quais dispositivos estão offline?",
            roteiro,
            ToolLoading::OnDemand,
        )
        .await;

        let primeira = &rodadas[0].ferramentas;
        assert!(primeira.contains(&"list_devices".to_string()));
        assert!(primeira.contains(&LOAD_TOOLS.to_string()));
        assert!(!primeira.contains(&"chart_monitor_latency".to_string()));
        assert!(rodadas[0].prompt.contains("FERRAMENTAS SOB DEMANDA"));
        assert!(rodadas[0].prompt.contains("charts:"));

        assert!(
            rodadas[1]
                .ferramentas
                .contains(&"chart_monitor_latency".to_string()),
            "carregado vale na rodada seguinte"
        );
        assert!(
            !eventos.iter().any(|evento| matches!(
                evento,
                HarnessEvent::ToolCall { name, .. } if name == LOAD_TOOLS
            )),
            "carregar ferramentas não aparece na tela"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn pergunta_que_pede_grafico_ja_chega_com_os_graficos() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let (rodadas, _) = conversar(
            &ctx,
            "mostre o gráfico de latência do gateway",
            vec![texto("ok")],
            ToolLoading::OnDemand,
        )
        .await;
        assert!(rodadas[0]
            .ferramentas
            .contains(&"chart_monitor_latency".to_string()));
        assert!(
            rodadas[0].prompt.contains("- charts:"),
            "o catálogo não encolhe ao carregar: o system prompt fica igual e o cache vale"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn rotina_automatica_recebe_o_catalogo_inteiro() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let (rodadas, _) = conversar(&ctx, "Oi", vec![texto("ok")], ToolLoading::Everything).await;
        let ferramentas = &rodadas[0].ferramentas;
        assert!(ferramentas.contains(&"analyze_root_cause".to_string()));
        assert!(!ferramentas.contains(&LOAD_TOOLS.to_string()));
        assert!(!rodadas[0].prompt.contains("FERRAMENTAS SOB DEMANDA"));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn chamada_direta_pelo_catalogo_carrega_o_grupo_sem_load_tools() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let roteiro = vec![chamada("get_logs_overview", json!({})), texto("Pronto.")];
        let (rodadas, _) = conversar(
            &ctx,
            "quais dispositivos estão offline?",
            roteiro,
            ToolLoading::OnDemand,
        )
        .await;

        assert!(!rodadas[0]
            .ferramentas
            .contains(&"get_logs_overview".to_string()));
        assert!(
            rodadas[1]
                .ferramentas
                .contains(&"get_logs_overview".to_string()),
            "o grupo da ferramenta chamada entra para a rodada seguinte ter o contrato"
        );
    })
    .await;
}

/// Modelo treinado (sem roteamento do Decisor) e ações com confirmação.
fn pedido_treinado(pergunta: &str, carga: ToolLoading) -> (AiSettings, AgentRequest) {
    let settings = AiSettings {
        enabled: true,
        action_mode: AiAutomationMode::Confirm,
        container_actions: AiAutomationMode::Confirm,
        decider: AiDeciderSettings {
            enabled: false,
            ..AiDeciderSettings::default()
        },
        ..AiSettings::default()
    };
    let request = AgentRequest {
        messages: vec![ChatMessageInput {
            role: "user".into(),
            content: pergunta.into(),
        }],
        context: ChatContext::default(),
        policy: ToolPolicy::from_settings(&settings),
        tool_loading: carga,
        memory: ConversationMemory::default(),
        actor: netmonitor_nucleo::services::audit::AuditActor::default(),
    };
    (settings, request)
}

async fn conversar_treinado(
    ctx: &AppContext,
    pergunta: &str,
    roteiro: Vec<AiChatChunk>,
    carga: ToolLoading,
) -> (Vec<Rodada>, Vec<HarnessEvent>) {
    let (settings, request) = pedido_treinado(pergunta, carga);
    rodar(ctx, settings, request, roteiro).await
}

/// Roda um pedido já montado contra o roteiro.
async fn rodar(
    ctx: &AppContext,
    settings: AiSettings,
    request: AgentRequest,
    roteiro: Vec<AiChatChunk>,
) -> (Vec<Rodada>, Vec<HarnessEvent>) {
    let (driver, rodadas) = Gravador::novo(roteiro);
    let eventos: Vec<HarnessEvent> =
        run_agent_loop(ctx.clone(), settings, Box::new(driver), request)
            .await
            .collect()
            .await;
    let rodadas = rodadas.lock().unwrap().clone();
    (rodadas, eventos)
}

fn cartoes(eventos: &[HarnessEvent]) -> usize {
    eventos
        .iter()
        .filter(|evento| matches!(evento, HarnessEvent::ConfirmationRequired { .. }))
        .count()
}

fn agendamento() -> AiChatChunk {
    chamada(
        "create_scheduled_task",
        json!({
            "name": "verificar_containers",
            "tool": "get_docker_containers",
            "cron": "0 9 * * *",
            "arguments": {}
        }),
    )
}

/// O modelo pequeno repete a proposta na rodada seguinte, apesar da nota:
/// cada repetição era mais um cartão igual na tela.
#[tokio::test]
#[serial]
async fn proposta_repetida_nao_vira_segundo_cartao() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let (rodadas, eventos) = conversar_treinado(
            &ctx,
            "Marque para amanhã às 9 horas verificar os contêineres.",
            vec![agendamento(), agendamento(), texto("Proposta enviada.")],
            ToolLoading::OnDemand,
        )
        .await;

        assert_eq!(
            cartoes(&eventos),
            1,
            "a mesma proposta pede confirmação uma vez só"
        );
        assert_eq!(
            rodadas.len(),
            3,
            "a repetição volta à IA, que fecha com texto"
        );
    })
    .await;
}

/// O cartão da pergunta anterior ainda espera a decisão e o usuário escreve
/// "pode fazer": a IA propõe de novo, mas a tela continua com um cartão só.
#[tokio::test]
#[serial]
async fn proposta_ainda_pendente_na_conversa_nao_vira_segundo_cartao() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let pedido = |pergunta: &str, conversa: &str| {
            let (settings, mut request) = pedido_treinado(pergunta, ToolLoading::OnDemand);
            request.actor.user_id = Some(4242);
            request.memory.conversation_key = Some(conversa.into());
            (settings, request)
        };
        let (settings, request) = pedido(
            "Marque para amanhã às 9 horas verificar os contêineres.",
            "c-1",
        );
        let (_, primeira) = rodar(
            &ctx,
            settings,
            request,
            vec![agendamento(), texto("Proposta enviada.")],
        )
        .await;
        assert_eq!(cartoes(&primeira), 1);

        let (settings, request) = pedido("pode marcar o agendamento", "c-1");
        let (_, segunda) = rodar(
            &ctx,
            settings,
            request,
            vec![agendamento(), texto("Confirme no cartão.")],
        )
        .await;
        assert_eq!(
            cartoes(&segunda),
            0,
            "o cartão da pergunta anterior continua valendo"
        );

        // Em outra conversa a mesma proposta é nova.
        let (settings, request) = pedido("pode marcar o agendamento", "c-2");
        let (_, outra) = rodar(
            &ctx,
            settings,
            request,
            vec![agendamento(), texto("Proposta enviada.")],
        )
        .await;
        assert_eq!(cartoes(&outra), 1);
    })
    .await;
}

/// Provedor que pensa para sempre. Avisa quando é chamado e, ao ser largado
/// (fim da resposta), fecha `largado`.
struct Pensativo {
    chamado: Mutex<Option<oneshot::Sender<()>>>,
    _largado: oneshot::Sender<()>,
}

#[async_trait]
impl AiDriver for Pensativo {
    fn id(&self) -> &'static str {
        "fake"
    }

    fn display_name(&self) -> &'static str {
        "Fake"
    }

    async fn chat_stream(
        &self,
        _messages: &[AiMessage],
        _tools: &[AiTool],
        _options: AiChatOptions,
    ) -> AppResult<AiChunkStream> {
        if let Some(chamado) = self.chamado.lock().unwrap().take() {
            let _ = chamado.send(());
        }
        futures::future::pending().await
    }

    async fn test_connection(&self) -> AppResult<TestConnectionResponse> {
        Ok(TestConnectionResponse {
            success: true,
            latency_ms: 0.0,
            message: String::new(),
            model: None,
        })
    }
}

/// Fechar a tela larga a resposta na hora, mesmo com o modelo pensando sem
/// escrever: antes, ela só parava ao tentar mandar o próximo evento.
#[tokio::test]
#[serial]
async fn tela_fechada_larga_o_provedor_que_ainda_pensa() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let (chamado, foi_chamado) = oneshot::channel();
        let (largado, foi_largado) = oneshot::channel::<()>();
        let driver = Pensativo {
            chamado: Mutex::new(Some(chamado)),
            _largado: largado,
        };
        let (settings, request) =
            pedido_treinado("quais dispositivos estão offline?", ToolLoading::OnDemand);
        let tela = run_agent_loop(ctx.clone(), settings, Box::new(driver), request).await;
        tokio::time::timeout(Duration::from_secs(5), foi_chamado)
            .await
            .expect("o provedor é chamado")
            .expect("o aviso chega");
        drop(tela);
        assert!(
            tokio::time::timeout(Duration::from_secs(5), foi_largado)
                .await
                .is_ok(),
            "a resposta para quando a tela fecha"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn acao_invalida_devolve_erro_ao_modelo_e_suprime_ask_user_no_mesmo_lote() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let lote_quebrado = AiChatChunk {
            tool_calls: vec![
                AiToolCall {
                    id: "c-bad-schedule".into(),
                    name: "create_scheduled_task".into(),
                    arguments: json!({
                        "name": "verificar_containers",
                        "tool": "docker_container_action",
                        "cron": "0 9 * * *",
                        "arguments": {}
                    })
                    .to_string(),
                },
                AiToolCall {
                    id: "c-premature-ask".into(),
                    name: "ask_user".into(),
                    arguments: json!({
                        "question": "Qual container Docker você deseja agendar?",
                        "options": ["zabbix-proxy-01", "redis-fila-arnoldo"]
                    })
                    .to_string(),
                },
            ],
            ..AiChatChunk::default()
        };
        let roteiro = vec![
            lote_quebrado,
            chamada(
                "create_scheduled_task",
                json!({
                    "name": "verificar_containers",
                    "tool": "get_docker_containers",
                    "cron": "0 9 * * *",
                    "arguments": {}
                }),
            ),
            texto("Proposta de agendamento enviada."),
        ];

        let (rodadas, eventos) = conversar_treinado(
            &ctx,
            "Marque para amanhã às 9 horas verificar se os contêiner está tudo ok e me notificar.",
            roteiro,
            ToolLoading::OnDemand,
        )
        .await;

        assert!(
            rodadas[0]
                .ferramentas
                .contains(&"create_scheduled_task".to_string()),
            "pergunta com 'marque' e 'amanhã' já traz o grupo schedules na primeira rodada"
        );
        assert!(
            rodadas[0]
                .ferramentas
                .contains(&"get_docker_containers".to_string()),
            "pergunta com 'contêiner' já traz o grupo docker na primeira rodada"
        );

        let confirmacoes: Vec<_> = eventos
            .iter()
            .filter_map(|ev| match ev {
                HarnessEvent::ConfirmationRequired { id, summary, .. } => {
                    Some((id.clone(), summary.clone()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            confirmacoes.len(),
            1,
            "a primeira chamada inválida não vira cartão de confirmação; só a segunda, corrigida, pede confirmação"
        );
        assert!(
            !confirmacoes[0].1.contains("Ação inválida"),
            "nenhum cartão de confirmação pode exibir 'Ação inválida'"
        );

        let pediu_ask_user = eventos.iter().any(|ev| {
            matches!(
                ev,
                HarnessEvent::ToolCall { name, .. } if name == "ask_user"
            )
        });
        assert!(
            !pediu_ask_user,
            "o ask_user prematuro no mesmo lote de uma ação inválida deve ser suprimido"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn ferramenta_inventada_devolve_erro_sem_cartao_quebrado_e_carrega_grupo() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let lote_alucinado = AiChatChunk {
            tool_calls: vec![
                AiToolCall {
                    id: "c-fake-sched".into(),
                    name: "scheduler_add".into(),
                    arguments: json!({"name": "reminder_link_chat"}).to_string(),
                },
                AiToolCall {
                    id: "c-fake-ask".into(),
                    name: "ask_user".into(),
                    arguments: json!({
                        "question": "Qual container Docker está para você parar?",
                        "options": ["nginx-recoisoir", "redis-fila"]
                    })
                    .to_string(),
                },
            ],
            ..AiChatChunk::default()
        };
        let roteiro = vec![
            lote_alucinado,
            texto("O agendador só executa ferramentas técnicas automatizadas, não lembretes em texto livre."),
        ];

        let (rodadas, eventos) = conversar_treinado(
            &ctx,
            "Me avise sobre revisar um item pendente",
            roteiro,
            ToolLoading::OnDemand,
        )
        .await;

        let mostrou_scheduler_add = eventos.iter().any(|ev| {
            matches!(
                ev,
                HarnessEvent::ToolCall { name, .. } if name == "scheduler_add"
            )
        });
        assert!(
            !mostrou_scheduler_add,
            "ferramenta inexistente não deve exibir cartão de falha vermelho para o usuário"
        );
        assert!(
            rodadas[1]
                .ferramentas
                .contains(&"create_scheduled_task".to_string()),
            "chamar 'scheduler_add' carrega automaticamente o grupo schedules para a rodada seguinte"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn ask_user_remove_opcoes_inventadas_e_preenche_todos_os_recursos_reais() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let settings = AiSettings {
            enabled: true,
            ..AiSettings::default()
        };
        let (driver, _) = Gravador::novo(vec![chamada(
            "ask_user",
            json!({
                "question": "Qual container Docker você deseja reiniciar?",
                "options": ["zabbix-proxy-01", "redis-fila-arnoldo"]
            }),
        )]);
        let historico = vec![
            ChatMessageInput {
                role: "user".into(),
                content: "Liste os containers".into(),
            },
            ChatMessageInput {
                role: "assistant".into(),
                content: "Containers ativos:\n- `nginx-reverso`\n- `postgres-telemetria`\n- `prometheus-stack`\n- `redis-fila`".into(),
            },
            ChatMessageInput {
                role: "user".into(),
                content: "Reinicie o container".into(),
            },
        ];
        let eventos: Vec<HarnessEvent> = run_agent_loop(
            ctx.clone(),
            settings.clone(),
            Box::new(driver),
            AgentRequest {
                messages: historico,
                context: ChatContext::default(),
                policy: ToolPolicy::from_settings(&settings),
                tool_loading: ToolLoading::Everything,
                memory: ConversationMemory::default(),
                actor: netmonitor_nucleo::services::audit::AuditActor::default(),
            },
        )
        .await
        .collect()
        .await;

        let resultado_ask = eventos
            .iter()
            .find_map(|ev| match ev {
                HarnessEvent::ToolResult { result, .. } => Some(result.clone()),
                _ => None,
            })
            .expect("deve emitir ToolResult para ask_user");
        let opcoes: Vec<String> = resultado_ask["options"]
            .as_array()
            .expect("options deve ser array")
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        assert_eq!(
            opcoes,
            vec![
                "nginx-reverso".to_string(),
                "postgres-telemetria".to_string(),
                "prometheus-stack".to_string(),
                "redis-fila".to_string(),
            ],
            "opções alucinadas ('zabbix-proxy-01', 'redis-fila-arnoldo') saem e entram todos os 4 containers reais"
        );
    })
    .await;
}
