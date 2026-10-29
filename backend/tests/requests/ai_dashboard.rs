//! Layout do dashboard: as rotas da tela publicam `dashboard:layout_updated`,
//! e a IA lê o layout e só o muda depois da confirmação — a cada chamada,
//! também no modo automático.

use backend::{
    app::App,
    models::_entities::event_outbox,
    services::ai::{
        harness::tools::{ToolArgs, ToolPolicy, ToolRegistry, Verdict},
        settings::AiAutomationMode,
    },
};
use loco_rs::{app::AppContext, testing::prelude::*};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde_json::{json, Value};
use serial_test::serial;

use super::prepare_data;

const EVENTO: &str = "dashboard:layout_updated";

fn politica(actions: AiAutomationMode) -> ToolPolicy {
    ToolPolicy {
        allow_active: false,
        actions,
        confirm_active: false,
        interactive: true,
        container_actions: AiAutomationMode::Off,
    }
}

/// O `data` dos eventos de layout publicados, do mais antigo ao mais novo.
async fn eventos(ctx: &AppContext) -> Vec<Value> {
    event_outbox::Entity::find()
        .filter(event_outbox::Column::Type.eq(EVENTO))
        .order_by_asc(event_outbox::Column::Id)
        .all(&ctx.db)
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.payload["data"].clone())
        .collect()
}

async fn layout_gravado(request: &loco_rs::TestServer) -> Value {
    let resposta = request.get("/api/dashboard/layout").await;
    assert_eq!(resposta.status_code(), 200);
    serde_json::from_str::<Value>(&resposta.text()).unwrap()
}

#[tokio::test]
#[serial]
async fn salvar_pela_tela_publica_o_layout_com_o_client_id() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let vazio = layout_gravado(&request).await;
        assert!(vazio["layout"].is_null(), "nada salvo ainda: {vazio}");

        let layout = json!([{ "id": "stat_cards", "type": "stat_cards", "order": 0, "pinned": 1 }]);
        let salvo = request
            .post("/api/dashboard/layout")
            .json(&json!({ "layout": layout, "clientId": "aba-1" }))
            .await;
        assert_eq!(salvo.status_code(), 200);
        assert_eq!(
            serde_json::from_str::<Value>(&salvo.text()).unwrap()["success"],
            true
        );

        let lido = layout_gravado(&request).await;
        assert_eq!(
            lido["layout"], layout,
            "a tela recebe o que gravou, intacto"
        );
        assert!(lido["updatedAt"].is_string());

        assert_eq!(
            eventos(&ctx).await,
            vec![json!({ "layout": layout, "clientId": "aba-1" })]
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn ia_le_o_layout_padrao_e_o_catalogo() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let dados = ToolRegistry::new(ToolPolicy::passive())
            .execute(&ctx, "get_dashboard_layout", "{}")
            .await
            .unwrap()
            .data;
        let cards = dados["widgets"].as_array().unwrap();
        assert_eq!(cards.len(), 9, "sem nada salvo vale o padrão");
        assert_eq!(cards[0]["id"], "stat_cards");
        assert_eq!(cards[0]["position"], 0);
        assert_eq!(cards[0]["size"], "full");
        assert_eq!(cards[8]["id"], "docker_summary");
        let cpu = dados["catalog"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["type"] == "cpu_usage")
            .unwrap();
        assert_eq!(cpu["multiple"], true);
        assert_eq!(cpu["default_size"], "half");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn ia_so_muda_o_layout_confirmada_e_avisa_as_abas() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let argumentos = json!({ "operations": [
            { "op": "remove", "id": "events_feed" },
            { "op": "add", "type": "cpu_usage", "position": 1, "config": { "deviceId": 4, "timeframe": "1h" } },
            { "op": "resize", "id": "active_alerts", "size": "third" },
            { "op": "move", "id": "network_monitors", "position": 0 }
        ]})
        .to_string();

        for modo in [AiAutomationMode::Confirm, AiAutomationMode::Auto] {
            let registro = ToolRegistry::new(politica(modo));
            let avaliacao = registro
                .assess(&ctx, "update_dashboard_layout", &ToolArgs::parse(&argumentos))
                .await
                .unwrap();
            assert!(
                matches!(avaliacao.verdict, Verdict::Ask),
                "{modo:?}: o layout é de todos, sempre pergunta"
            );
            assert!(registro
                .execute(&ctx, "update_dashboard_layout", &argumentos)
                .await
                .is_err());
        }
        assert!(
            !ToolRegistry::new(politica(AiAutomationMode::Off))
                .tool_names(backend::services::ai::harness::tools::ToolGroup::Dashboard)
                .contains(&"update_dashboard_layout"),
            "ações desligadas: só a leitura"
        );

        let registro = ToolRegistry::new(politica(AiAutomationMode::Confirm));
        let resumo = registro
            .preview(&ctx, "update_dashboard_layout", &argumentos)
            .await
            .unwrap();
        assert!(resumo.contains("1. Ocultar o card \"Feed de Eventos Realtime\""), "{resumo}");
        assert!(resumo.contains("2. Adicionar o card \"Uso de CPU\""), "{resumo}");
        assert!(resumo.contains("período 1h"), "{resumo}");
        assert!(resumo.contains("3. Mudar \"Alertas Críticos Ativos\" para um terço"), "{resumo}");
        assert!(resumo.contains("4. Mover \"Monitores de Rede\" para a posição 1"), "{resumo}");
        assert!(eventos(&ctx).await.is_empty(), "a prévia não grava nem publica");
        assert!(layout_gravado(&request).await["layout"].is_null());

        let feito = registro
            .execute_confirmed(&ctx, "update_dashboard_layout", ToolArgs::parse(&argumentos))
            .await
            .unwrap()
            .data;
        assert_eq!(feito["done"], true);
        assert_eq!(feito["changes"].as_array().unwrap().len(), 4);

        let gravado = layout_gravado(&request).await["layout"].clone();
        let lista = gravado.as_array().unwrap();
        let card = |id: &str| lista.iter().find(|item| item["id"] == id).unwrap().clone();
        assert_eq!(card("events_feed")["visible"], false);
        assert_eq!(card("active_alerts")["lg"], 4);
        assert_eq!(card("network_monitors")["order"], 0);
        let cpu = lista
            .iter()
            .find(|item| item["type"] == "cpu_usage")
            .expect("card de CPU gravado");
        assert!(cpu["id"].as_str().unwrap().starts_with("cpu_usage_"));
        assert_eq!(cpu["config"], json!({ "deviceId": 4, "timeframe": "1h" }));

        let publicados = eventos(&ctx).await;
        assert_eq!(publicados.len(), 1);
        assert_eq!(publicados[0]["layout"], gravado);
        assert!(publicados[0]["clientId"].is_null(), "a mudança não veio de aba nenhuma");

        let invalido = json!({ "operations": [{ "op": "add", "type": "stat_cards" }] }).to_string();
        let recusa = registro
            .preview(&ctx, "update_dashboard_layout", &invalido)
            .await
            .unwrap();
        assert!(recusa.starts_with("Ação inválida"), "{recusa}");
        let sem_efeito = registro
            .execute_confirmed(&ctx, "update_dashboard_layout", ToolArgs::parse(&invalido))
            .await
            .unwrap();
        assert!(sem_efeito.data["error"].is_string(), "erro volta à IA como dado");
        assert_eq!(eventos(&ctx).await.len(), 1, "nada gravado");
    })
    .await;
}
