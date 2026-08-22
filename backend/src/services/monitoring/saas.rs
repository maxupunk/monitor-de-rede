//! Catálogo e provisionamento de serviços SaaS para monitoramento de latência e QoE (§2.2.2).

use loco_rs::app::AppContext;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::json;

use crate::{
    dtos::saas::{
        SaasPreset, SaasPresetsResponse, SaasProvisionRequest, SaasProvisionResponse,
        SaasThresholds,
    },
    models::{monitor_results, monitors},
    services::shared::errors::{AppError, AppResult},
};

/// Definição estática dos presets curados de serviços SaaS e Bancos.
pub fn get_curated_saas_definitions() -> Vec<SaasPreset> {
    vec![
        // === BANCOS & FINANÇAS ===
        SaasPreset {
            id: "nubank-http".into(),
            name: "Nubank".into(),
            provider: "Nubank".into(),
            category: "finance".into(),
            icon: "mdi-credit-card-outline".into(),
            color: "#820AD1".into(),
            description: "Disponibilidade e latência HTTP para os canais e serviços do Nubank."
                .into(),
            check_type: "http".into(),
            target: "https://www.nubank.com.br".into(),
            port: None,
            http_method: Some("HEAD".into()),
            accepted_status_codes: Some(vec![200, 301, 302, 204]),
            interval_seconds: 60,
            timeout_seconds: 5,
            suggested_thresholds: SaasThresholds {
                warning_latency_ms: 80.0,
                critical_latency_ms: 250.0,
                max_packet_loss_percent: None,
            },
            is_provisioned: false,
            monitor_id: None,
            current_status: None,
            current_latency_ms: None,
        },
        SaasPreset {
            id: "itau-http".into(),
            name: "Itaú Unibanco".into(),
            provider: "Itaú".into(),
            category: "finance".into(),
            icon: "mdi-bank".into(),
            color: "#EC7000".into(),
            description: "Tempo de resposta HTTP para o portal e internet banking do Itaú.".into(),
            check_type: "http".into(),
            target: "https://www.itau.com.br".into(),
            port: None,
            http_method: Some("HEAD".into()),
            accepted_status_codes: Some(vec![200, 301, 302, 204]),
            interval_seconds: 60,
            timeout_seconds: 5,
            suggested_thresholds: SaasThresholds {
                warning_latency_ms: 90.0,
                critical_latency_ms: 280.0,
                max_packet_loss_percent: None,
            },
            is_provisioned: false,
            monitor_id: None,
            current_status: None,
            current_latency_ms: None,
        },
        SaasPreset {
            id: "bradesco-http".into(),
            name: "Banco Bradesco".into(),
            provider: "Bradesco".into(),
            category: "finance".into(),
            icon: "mdi-bank".into(),
            color: "#CC092F".into(),
            description: "Disponibilidade HTTP para o portal institucional e canais Bradesco."
                .into(),
            check_type: "http".into(),
            target: "https://www.bradesco.com.br".into(),
            port: None,
            http_method: Some("HEAD".into()),
            accepted_status_codes: Some(vec![200, 301, 302, 204]),
            interval_seconds: 60,
            timeout_seconds: 5,
            suggested_thresholds: SaasThresholds {
                warning_latency_ms: 90.0,
                critical_latency_ms: 280.0,
                max_packet_loss_percent: None,
            },
            is_provisioned: false,
            monitor_id: None,
            current_status: None,
            current_latency_ms: None,
        },
        SaasPreset {
            id: "bb-http".into(),
            name: "Banco do Brasil".into(),
            provider: "Banco do Brasil".into(),
            category: "finance".into(),
            icon: "mdi-bank".into(),
            color: "#FCDB00".into(),
            description: "Latência de resposta HTTP para o portal de serviços do Banco do Brasil."
                .into(),
            check_type: "http".into(),
            target: "https://www.bb.com.br".into(),
            port: None,
            http_method: Some("HEAD".into()),
            accepted_status_codes: Some(vec![200, 301, 302, 204]),
            interval_seconds: 60,
            timeout_seconds: 5,
            suggested_thresholds: SaasThresholds {
                warning_latency_ms: 90.0,
                critical_latency_ms: 280.0,
                max_packet_loss_percent: None,
            },
            is_provisioned: false,
            monitor_id: None,
            current_status: None,
            current_latency_ms: None,
        },
        SaasPreset {
            id: "caixa-http".into(),
            name: "Caixa Econômica Federal".into(),
            provider: "Caixa".into(),
            category: "finance".into(),
            icon: "mdi-bank".into(),
            color: "#006699".into(),
            description: "Tempo de resposta HTTP para os canais digitais e serviços da Caixa."
                .into(),
            check_type: "http".into(),
            target: "https://www.caixa.gov.br".into(),
            port: None,
            http_method: Some("HEAD".into()),
            accepted_status_codes: Some(vec![200, 301, 302, 204]),
            interval_seconds: 60,
            timeout_seconds: 5,
            suggested_thresholds: SaasThresholds {
                warning_latency_ms: 100.0,
                critical_latency_ms: 300.0,
                max_packet_loss_percent: None,
            },
            is_provisioned: false,
            monitor_id: None,
            current_status: None,
            current_latency_ms: None,
        },
        SaasPreset {
            id: "santander-http".into(),
            name: "Banco Santander".into(),
            provider: "Santander".into(),
            category: "finance".into(),
            icon: "mdi-bank".into(),
            color: "#EA1D25".into(),
            description: "Disponibilidade e tempo de resposta HTTP do portal Santander Brasil."
                .into(),
            check_type: "http".into(),
            target: "https://www.santander.com.br".into(),
            port: None,
            http_method: Some("HEAD".into()),
            accepted_status_codes: Some(vec![200, 301, 302, 204]),
            interval_seconds: 60,
            timeout_seconds: 5,
            suggested_thresholds: SaasThresholds {
                warning_latency_ms: 90.0,
                critical_latency_ms: 280.0,
                max_packet_loss_percent: None,
            },
            is_provisioned: false,
            monitor_id: None,
            current_status: None,
            current_latency_ms: None,
        },
        SaasPreset {
            id: "inter-http".into(),
            name: "Banco Inter".into(),
            provider: "Inter".into(),
            category: "finance".into(),
            icon: "mdi-credit-card-chip-outline".into(),
            color: "#FF7A00".into(),
            description: "Disponibilidade e latência HTTP para o portal e Super App do Inter."
                .into(),
            check_type: "http".into(),
            target: "https://www.bancointer.com.br".into(),
            port: None,
            http_method: Some("HEAD".into()),
            accepted_status_codes: Some(vec![200, 301, 302, 204]),
            interval_seconds: 60,
            timeout_seconds: 5,
            suggested_thresholds: SaasThresholds {
                warning_latency_ms: 80.0,
                critical_latency_ms: 250.0,
                max_packet_loss_percent: None,
            },
            is_provisioned: false,
            monitor_id: None,
            current_status: None,
            current_latency_ms: None,
        },
        SaasPreset {
            id: "mercadopago-http".into(),
            name: "Mercado Pago".into(),
            provider: "Mercado Livre".into(),
            category: "finance".into(),
            icon: "mdi-cash-multiple".into(),
            color: "#009EE3".into(),
            description: "Latência de resposta HTTP para checkout e pagamentos Mercado Pago."
                .into(),
            check_type: "http".into(),
            target: "https://www.mercadopago.com.br".into(),
            port: None,
            http_method: Some("HEAD".into()),
            accepted_status_codes: Some(vec![200, 301, 302, 204]),
            interval_seconds: 60,
            timeout_seconds: 5,
            suggested_thresholds: SaasThresholds {
                warning_latency_ms: 80.0,
                critical_latency_ms: 250.0,
                max_packet_loss_percent: None,
            },
            is_provisioned: false,
            monitor_id: None,
            current_status: None,
            current_latency_ms: None,
        },
        SaasPreset {
            id: "pagbank-http".into(),
            name: "PagBank / PagSeguro".into(),
            provider: "UOL".into(),
            category: "finance".into(),
            icon: "mdi-credit-card-check-outline".into(),
            color: "#00A868".into(),
            description: "Tempo de resposta HTTP para serviços e processamento PagBank.".into(),
            check_type: "http".into(),
            target: "https://pagseguro.uol.com.br".into(),
            port: None,
            http_method: Some("HEAD".into()),
            accepted_status_codes: Some(vec![200, 301, 302, 204]),
            interval_seconds: 60,
            timeout_seconds: 5,
            suggested_thresholds: SaasThresholds {
                warning_latency_ms: 80.0,
                critical_latency_ms: 250.0,
                max_packet_loss_percent: None,
            },
            is_provisioned: false,
            monitor_id: None,
            current_status: None,
            current_latency_ms: None,
        },
        SaasPreset {
            id: "stone-http".into(),
            name: "Stone Pagamentos".into(),
            provider: "StoneCo".into(),
            category: "finance".into(),
            icon: "mdi-contactless-payment".into(),
            color: "#00A868".into(),
            description: "Disponibilidade e conectividade HTTP da infraestrutura Stone.".into(),
            check_type: "http".into(),
            target: "https://www.stone.com.br".into(),
            port: None,
            http_method: Some("HEAD".into()),
            accepted_status_codes: Some(vec![200, 301, 302, 204]),
            interval_seconds: 60,
            timeout_seconds: 5,
            suggested_thresholds: SaasThresholds {
                warning_latency_ms: 80.0,
                critical_latency_ms: 250.0,
                max_packet_loss_percent: None,
            },
            is_provisioned: false,
            monitor_id: None,
            current_status: None,
            current_latency_ms: None,
        },
        SaasPreset {
            id: "stripe-http".into(),
            name: "Stripe Payments".into(),
            provider: "Stripe".into(),
            category: "finance".into(),
            icon: "mdi-credit-card-outline".into(),
            color: "#635BFF".into(),
            description: "Latência de conectividade para o gateway de pagamentos Stripe.".into(),
            check_type: "http".into(),
            target: "https://api.stripe.com".into(),
            port: None,
            http_method: Some("HEAD".into()),
            accepted_status_codes: Some(vec![200, 301, 302, 204]),
            interval_seconds: 60,
            timeout_seconds: 5,
            suggested_thresholds: SaasThresholds {
                warning_latency_ms: 80.0,
                critical_latency_ms: 250.0,
                max_packet_loss_percent: None,
            },
            is_provisioned: false,
            monitor_id: None,
            current_status: None,
            current_latency_ms: None,
        },
        // === PRODUTIVIDADE & IA ===
        SaasPreset {
            id: "google-http".into(),
            name: "Google Search".into(),
            provider: "Google".into(),
            category: "productivity".into(),
            icon: "mdi-google".into(),
            color: "#4285F4".into(),
            description: "Tempo de resposta HTTP HEAD para o portal de busca do Google.".into(),
            check_type: "http".into(),
            target: "https://www.google.com".into(),
            port: None,
            http_method: Some("HEAD".into()),
            accepted_status_codes: Some(vec![200, 301, 302, 204]),
            interval_seconds: 60,
            timeout_seconds: 5,
            suggested_thresholds: SaasThresholds {
                warning_latency_ms: 80.0,
                critical_latency_ms: 250.0,
                max_packet_loss_percent: None,
            },
            is_provisioned: false,
            monitor_id: None,
            current_status: None,
            current_latency_ms: None,
        },
        SaasPreset {
            id: "google-workspace-http".into(),
            name: "Google Workspace / Gmail".into(),
            provider: "Google".into(),
            category: "productivity".into(),
            icon: "mdi-google".into(),
            color: "#EA4335".into(),
            description: "Latência de conexão HTTP para o Google Workspace e Gmail.".into(),
            check_type: "http".into(),
            target: "https://workspace.google.com".into(),
            port: None,
            http_method: Some("HEAD".into()),
            accepted_status_codes: Some(vec![200, 301, 302, 204]),
            interval_seconds: 60,
            timeout_seconds: 5,
            suggested_thresholds: SaasThresholds {
                warning_latency_ms: 80.0,
                critical_latency_ms: 250.0,
                max_packet_loss_percent: None,
            },
            is_provisioned: false,
            monitor_id: None,
            current_status: None,
            current_latency_ms: None,
        },
        SaasPreset {
            id: "openai-http".into(),
            name: "OpenAI / ChatGPT".into(),
            provider: "OpenAI".into(),
            category: "productivity".into(),
            icon: "mdi-robot-outline".into(),
            color: "#10A37F".into(),
            description: "Tempo de resposta HTTP para a plataforma de IA da OpenAI.".into(),
            check_type: "http".into(),
            target: "https://chatgpt.com".into(),
            port: None,
            http_method: Some("HEAD".into()),
            accepted_status_codes: Some(vec![200, 301, 302, 307, 204]),
            interval_seconds: 60,
            timeout_seconds: 5,
            suggested_thresholds: SaasThresholds {
                warning_latency_ms: 100.0,
                critical_latency_ms: 300.0,
                max_packet_loss_percent: None,
            },
            is_provisioned: false,
            monitor_id: None,
            current_status: None,
            current_latency_ms: None,
        },
        SaasPreset {
            id: "microsoft365-http".into(),
            name: "Microsoft 365".into(),
            provider: "Microsoft".into(),
            category: "productivity".into(),
            icon: "mdi-microsoft-office".into(),
            color: "#D83B01".into(),
            description: "Latência de conexão para o portal principal do Microsoft 365 / Office."
                .into(),
            check_type: "http".into(),
            target: "https://www.microsoft365.com".into(),
            port: None,
            http_method: Some("HEAD".into()),
            accepted_status_codes: Some(vec![200, 301, 302, 307, 204]),
            interval_seconds: 60,
            timeout_seconds: 5,
            suggested_thresholds: SaasThresholds {
                warning_latency_ms: 90.0,
                critical_latency_ms: 300.0,
                max_packet_loss_percent: None,
            },
            is_provisioned: false,
            monitor_id: None,
            current_status: None,
            current_latency_ms: None,
        },
        // === NUVEM & INFRAESTRUTURA ===
        SaasPreset {
            id: "cloudflare-http".into(),
            name: "Cloudflare Edge CDN".into(),
            provider: "Cloudflare".into(),
            category: "cloud".into(),
            icon: "mdi-cloud-outline".into(),
            color: "#F38020".into(),
            description: "Tempo de resposta HTTP HEAD na borda Anycast da Cloudflare.".into(),
            check_type: "http".into(),
            target: "https://www.cloudflare.com".into(),
            port: None,
            http_method: Some("HEAD".into()),
            accepted_status_codes: Some(vec![200, 301, 302, 204]),
            interval_seconds: 60,
            timeout_seconds: 5,
            suggested_thresholds: SaasThresholds {
                warning_latency_ms: 70.0,
                critical_latency_ms: 200.0,
                max_packet_loss_percent: None,
            },
            is_provisioned: false,
            monitor_id: None,
            current_status: None,
            current_latency_ms: None,
        },
        SaasPreset {
            id: "aws-http".into(),
            name: "Amazon Web Services (AWS)".into(),
            provider: "Amazon".into(),
            category: "cloud".into(),
            icon: "mdi-aws".into(),
            color: "#FF9900".into(),
            description: "Tempo de resposta HTTP para a infraestrutura global da AWS.".into(),
            check_type: "http".into(),
            target: "https://aws.amazon.com".into(),
            port: None,
            http_method: Some("HEAD".into()),
            accepted_status_codes: Some(vec![200, 301, 302, 307, 204]),
            interval_seconds: 60,
            timeout_seconds: 5,
            suggested_thresholds: SaasThresholds {
                warning_latency_ms: 90.0,
                critical_latency_ms: 280.0,
                max_packet_loss_percent: None,
            },
            is_provisioned: false,
            monitor_id: None,
            current_status: None,
            current_latency_ms: None,
        },
        SaasPreset {
            id: "azure-http".into(),
            name: "Microsoft Azure".into(),
            provider: "Microsoft".into(),
            category: "cloud".into(),
            icon: "mdi-microsoft-azure".into(),
            color: "#0078D4".into(),
            description: "Disponibilidade e latência HTTP para o portal Microsoft Azure.".into(),
            check_type: "http".into(),
            target: "https://portal.azure.com".into(),
            port: None,
            http_method: Some("HEAD".into()),
            accepted_status_codes: Some(vec![200, 301, 302, 307, 204]),
            interval_seconds: 60,
            timeout_seconds: 5,
            suggested_thresholds: SaasThresholds {
                warning_latency_ms: 90.0,
                critical_latency_ms: 280.0,
                max_packet_loss_percent: None,
            },
            is_provisioned: false,
            monitor_id: None,
            current_status: None,
            current_latency_ms: None,
        },
        // === COMUNICAÇÃO ===
        SaasPreset {
            id: "whatsapp-http".into(),
            name: "WhatsApp Web".into(),
            provider: "Meta".into(),
            category: "communication".into(),
            icon: "mdi-whatsapp".into(),
            color: "#25D366".into(),
            description: "Latência de conexão HTTP para os servidores do WhatsApp Web.".into(),
            check_type: "http".into(),
            target: "https://web.whatsapp.com".into(),
            port: None,
            http_method: Some("HEAD".into()),
            accepted_status_codes: Some(vec![200, 301, 302, 204]),
            interval_seconds: 60,
            timeout_seconds: 5,
            suggested_thresholds: SaasThresholds {
                warning_latency_ms: 80.0,
                critical_latency_ms: 250.0,
                max_packet_loss_percent: None,
            },
            is_provisioned: false,
            monitor_id: None,
            current_status: None,
            current_latency_ms: None,
        },
        SaasPreset {
            id: "telegram-http".into(),
            name: "Telegram Web".into(),
            provider: "Telegram".into(),
            category: "communication".into(),
            icon: "mdi-send".into(),
            color: "#229ED9".into(),
            description: "Disponibilidade e tempo de resposta HTTP do Telegram Web.".into(),
            check_type: "http".into(),
            target: "https://web.telegram.org".into(),
            port: None,
            http_method: Some("HEAD".into()),
            accepted_status_codes: Some(vec![200, 301, 302, 204]),
            interval_seconds: 60,
            timeout_seconds: 5,
            suggested_thresholds: SaasThresholds {
                warning_latency_ms: 80.0,
                critical_latency_ms: 250.0,
                max_packet_loss_percent: None,
            },
            is_provisioned: false,
            monitor_id: None,
            current_status: None,
            current_latency_ms: None,
        },
        SaasPreset {
            id: "microsoft-teams-http".into(),
            name: "Microsoft Teams".into(),
            provider: "Microsoft".into(),
            category: "communication".into(),
            icon: "mdi-microsoft-teams".into(),
            color: "#6264A7".into(),
            description: "Disponibilidade e latência HTTP para o Microsoft Teams.".into(),
            check_type: "http".into(),
            target: "https://teams.microsoft.com".into(),
            port: None,
            http_method: Some("HEAD".into()),
            accepted_status_codes: Some(vec![200, 301, 302, 307, 204]),
            interval_seconds: 60,
            timeout_seconds: 5,
            suggested_thresholds: SaasThresholds {
                warning_latency_ms: 100.0,
                critical_latency_ms: 300.0,
                max_packet_loss_percent: None,
            },
            is_provisioned: false,
            monitor_id: None,
            current_status: None,
            current_latency_ms: None,
        },
        SaasPreset {
            id: "zoom-http".into(),
            name: "Zoom Video".into(),
            provider: "Zoom".into(),
            category: "communication".into(),
            icon: "mdi-video-outline".into(),
            color: "#2D8CFF".into(),
            description: "Disponibilidade e latência HTTP para a infraestrutura do Zoom.".into(),
            check_type: "http".into(),
            target: "https://zoom.us".into(),
            port: None,
            http_method: Some("HEAD".into()),
            accepted_status_codes: Some(vec![200, 301, 302, 204]),
            interval_seconds: 60,
            timeout_seconds: 5,
            suggested_thresholds: SaasThresholds {
                warning_latency_ms: 90.0,
                critical_latency_ms: 280.0,
                max_packet_loss_percent: None,
            },
            is_provisioned: false,
            monitor_id: None,
            current_status: None,
            current_latency_ms: None,
        },
        SaasPreset {
            id: "slack-http".into(),
            name: "Slack".into(),
            provider: "Salesforce".into(),
            category: "communication".into(),
            icon: "mdi-slack".into(),
            color: "#4A154B".into(),
            description: "Latência de conexão HTTP com o portal e APIs do Slack.".into(),
            check_type: "http".into(),
            target: "https://slack.com".into(),
            port: None,
            http_method: Some("HEAD".into()),
            accepted_status_codes: Some(vec![200, 301, 302, 204]),
            interval_seconds: 60,
            timeout_seconds: 5,
            suggested_thresholds: SaasThresholds {
                warning_latency_ms: 90.0,
                critical_latency_ms: 280.0,
                max_packet_loss_percent: None,
            },
            is_provisioned: false,
            monitor_id: None,
            current_status: None,
            current_latency_ms: None,
        },
        // === DESENVOLVIMENTO & APIs ===
        SaasPreset {
            id: "github-http".into(),
            name: "GitHub".into(),
            provider: "GitHub".into(),
            category: "developer".into(),
            icon: "mdi-github".into(),
            color: "#24292F".into(),
            description: "Latência de requisição HTTP HEAD para os serviços do GitHub.".into(),
            check_type: "http".into(),
            target: "https://github.com".into(),
            port: None,
            http_method: Some("HEAD".into()),
            accepted_status_codes: Some(vec![200, 301, 302, 204]),
            interval_seconds: 60,
            timeout_seconds: 5,
            suggested_thresholds: SaasThresholds {
                warning_latency_ms: 100.0,
                critical_latency_ms: 300.0,
                max_packet_loss_percent: None,
            },
            is_provisioned: false,
            monitor_id: None,
            current_status: None,
            current_latency_ms: None,
        },
        // === STREAMING & MÍDIA ===
        SaasPreset {
            id: "netflix-fast-http".into(),
            name: "Netflix / Fast.com".into(),
            provider: "Netflix".into(),
            category: "streaming".into(),
            icon: "mdi-netflix".into(),
            color: "#E50914".into(),
            description: "Conectividade HTTP HEAD com a CDN de streaming Fast.com/Netflix.".into(),
            check_type: "http".into(),
            target: "https://fast.com".into(),
            port: None,
            http_method: Some("HEAD".into()),
            accepted_status_codes: Some(vec![200, 301, 302, 204]),
            interval_seconds: 60,
            timeout_seconds: 5,
            suggested_thresholds: SaasThresholds {
                warning_latency_ms: 80.0,
                critical_latency_ms: 250.0,
                max_packet_loss_percent: None,
            },
            is_provisioned: false,
            monitor_id: None,
            current_status: None,
            current_latency_ms: None,
        },
        SaasPreset {
            id: "spotify-http".into(),
            name: "Spotify".into(),
            provider: "Spotify".into(),
            category: "streaming".into(),
            icon: "mdi-spotify".into(),
            color: "#1DB954".into(),
            description: "Disponibilidade e latência de resposta HTTP do Spotify.".into(),
            check_type: "http".into(),
            target: "https://www.spotify.com".into(),
            port: None,
            http_method: Some("HEAD".into()),
            accepted_status_codes: Some(vec![200, 301, 302, 204]),
            interval_seconds: 60,
            timeout_seconds: 5,
            suggested_thresholds: SaasThresholds {
                warning_latency_ms: 80.0,
                critical_latency_ms: 250.0,
                max_packet_loss_percent: None,
            },
            is_provisioned: false,
            monitor_id: None,
            current_status: None,
            current_latency_ms: None,
        },
        // === GOVERNO & UTILIDADES ===
        SaasPreset {
            id: "govbr-http".into(),
            name: "Portal Gov.br".into(),
            provider: "Governo Federal".into(),
            category: "government".into(),
            icon: "mdi-shield-account".into(),
            color: "#1351B4".into(),
            description: "Latência e disponibilidade HTTP do portal único Gov.br.".into(),
            check_type: "http".into(),
            target: "https://www.gov.br".into(),
            port: None,
            http_method: Some("HEAD".into()),
            accepted_status_codes: Some(vec![200, 301, 302, 204]),
            interval_seconds: 60,
            timeout_seconds: 5,
            suggested_thresholds: SaasThresholds {
                warning_latency_ms: 100.0,
                critical_latency_ms: 300.0,
                max_packet_loss_percent: None,
            },
            is_provisioned: false,
            monitor_id: None,
            current_status: None,
            current_latency_ms: None,
        },
    ]
}

/// Lista os presets SaaS enriquecendo com informações de monitores existentes no banco.
pub async fn get_saas_catalog<C>(db: &C) -> AppResult<SaasPresetsResponse>
where
    C: ConnectionTrait,
{
    let mut definitions = get_curated_saas_definitions();
    let existing_monitors = monitors::Entity::find().all(db).await?;

    let mut provisioned_count = 0;

    for preset in &mut definitions {
        // Encontra monitor por `saasService` no configuration ou por alvo correspondente
        let matching = existing_monitors.iter().find(|m| {
            let config_saas_id = m.configuration.get("saasPresetId").and_then(|v| v.as_str());
            let target = m.target();
            config_saas_id == Some(&preset.id)
                || (!target.is_empty()
                    && (target == preset.target
                        || target == preset.target.trim_start_matches("https://")
                        || target == preset.target.trim_start_matches("http://")))
        });

        if let Some(m) = matching {
            preset.is_provisioned = true;
            preset.monitor_id = Some(m.id);
            preset.current_status = Some(m.status.clone());
            provisioned_count += 1;

            // Busca a última latência medida
            let latest = monitor_results::Entity::find()
                .filter(monitor_results::Column::MonitorId.eq(m.id))
                .order_by_desc(monitor_results::Column::StartedAt)
                .one(db)
                .await?;
            if let Some(res) = latest {
                preset.current_latency_ms = res.latency_ms;
            }
        }
    }

    let total = definitions.len();
    Ok(SaasPresetsResponse {
        presets: definitions,
        total_presets: total,
        provisioned_count,
    })
}

/// Provisiona os presets de SaaS solicitados, criando novos monitores ou reabilitando existentes.
pub async fn provision_saas_presets(
    ctx: &AppContext,
    request: SaasProvisionRequest,
) -> AppResult<SaasProvisionResponse> {
    if request.preset_ids.is_empty() {
        return Err(AppError::validation(
            "Selecione ao menos um serviço SaaS para provisionar",
        ));
    }

    let definitions = get_curated_saas_definitions();
    let mut created_monitor_ids = Vec::new();
    let mut existing_monitor_ids = Vec::new();

    for preset_id in &request.preset_ids {
        let Some(preset) = definitions.iter().find(|d| &d.id == preset_id) else {
            continue;
        };

        // Verifica se já existe um monitor para este preset
        let existing = monitors::Entity::find().all(&ctx.db).await?;
        let matching = existing.iter().find(|m| {
            let config_saas_id = m.configuration.get("saasPresetId").and_then(|v| v.as_str());
            let target = m.target();
            config_saas_id == Some(&preset.id)
                || (!target.is_empty()
                    && (target == preset.target
                        || target == preset.target.trim_start_matches("https://")
                        || target == preset.target.trim_start_matches("http://")))
        });

        if let Some(m) = matching {
            existing_monitor_ids.push(m.id);
            // Se estiver desabilitado, reativa
            if !m.enabled {
                let mut active: monitors::ActiveModel = m.clone().into();
                active.enabled = Set(true);
                active.update(&ctx.db).await?;
            }
            continue;
        }

        // Constrói a configuração JSON específica
        let interval = request
            .interval_seconds
            .unwrap_or(preset.interval_seconds)
            .max(5);
        let timeout = request.timeout_seconds.unwrap_or(preset.timeout_seconds);

        let mut config_map = serde_json::Map::new();
        config_map.insert("isSaas".into(), json!(true));
        config_map.insert("saasPresetId".into(), json!(preset.id));
        config_map.insert("saasService".into(), json!(preset.provider.to_lowercase()));
        config_map.insert("saasCategory".into(), json!(preset.category));
        config_map.insert(
            "warningThresholdMs".into(),
            json!(preset.suggested_thresholds.warning_latency_ms),
        );
        config_map.insert(
            "criticalThresholdMs".into(),
            json!(preset.suggested_thresholds.critical_latency_ms),
        );

        match preset.check_type.as_str() {
            "ping" => {
                config_map.insert("host".into(), json!(preset.target));
            }
            "http" => {
                config_map.insert("url".into(), json!(preset.target));
                config_map.insert(
                    "method".into(),
                    json!(preset.http_method.as_deref().unwrap_or("HEAD")),
                );
                config_map.insert(
                    "acceptedStatusCodes".into(),
                    json!(preset
                        .accepted_status_codes
                        .clone()
                        .unwrap_or_else(|| vec![200, 301, 302, 204])),
                );
                config_map.insert("validateCertificate".into(), json!(true));
            }
            _ => {
                config_map.insert("host".into(), json!(preset.target));
            }
        }

        let now = chrono::Utc::now();
        let new_monitor = monitors::ActiveModel {
            name: Set(preset.name.clone()),
            r#type: Set(preset.check_type.clone()),
            configuration: Set(serde_json::Value::Object(config_map)),
            interval_seconds: Set(interval),
            timeout_seconds: Set(timeout),
            retry_count: Set(2),
            enabled: Set(true),
            status: Set("unknown".into()),
            next_run_at: Set(Some(now.into())),
            last_run_at: Set(None),
            device_id: Set(None),
            probe_id: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };

        let inserted = new_monitor.insert(&ctx.db).await?;
        created_monitor_ids.push(inserted.id);
    }

    let provisioned_count = created_monitor_ids.len() + existing_monitor_ids.len();
    let message = format!(
        "{} monitor(es) SaaS provisionado(s) com sucesso ({} novo(s), {} já existente(s)).",
        provisioned_count,
        created_monitor_ids.len(),
        existing_monitor_ids.len()
    );

    Ok(SaasProvisionResponse {
        provisioned_count,
        created_monitor_ids,
        existing_monitor_ids,
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_possuem_dados_consistentes() {
        let presets = get_curated_saas_definitions();
        assert!(presets.len() >= 8);

        for p in &presets {
            assert!(!p.id.is_empty());
            assert!(!p.name.is_empty());
            assert!(!p.provider.is_empty());
            assert!(!p.category.is_empty());
            assert!(!p.target.is_empty());
            assert!(p.interval_seconds >= 5);
            assert!(p.suggested_thresholds.warning_latency_ms > 0.0);
            assert!(
                p.suggested_thresholds.critical_latency_ms
                    >= p.suggested_thresholds.warning_latency_ms
            );
        }
    }
}
