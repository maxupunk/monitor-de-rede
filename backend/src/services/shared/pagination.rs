//! Envelope de paginação `{ data, meta }`.
//!
//! O Loco tem seu próprio `PaginationResponse`, e ainda assim não é ele que sai
//! daqui. Quem manda é o frontend: o `useInfiniteList`
//! (`frontend/src/composables/useInfiniteList.ts`) lê `response.data` e decide
//! o fim da lista por `meta.currentPage >= meta.lastPage`. Um envelope
//! diferente faria toda lista infinita parar na primeira página — ou nunca
//! parar.
//!
//! O contrato expõe os dados e metadados necessários para paginação por número
//! de página, sem acoplar controllers ao ORM ou à apresentação do frontend.
//!
//! Os tipos moram aqui, e não em `dtos/common.rs`, porque este módulo é o dono
//! da paginação; `crate::dtos::common` os reexporta para quem só precisa do DTO.

use sea_orm::{ConnectionTrait, EntityTrait, FromQueryResult, PaginatorTrait, Select};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::services::shared::errors::AppResult;

/// Itens por página quando o cliente não manda `limit`.
pub const DEFAULT_LIMIT: u64 = 20;
/// Teto de itens por página. Protege o banco de um `?limit=100000`.
pub const MAX_LIMIT: u64 = 100;

/// O `meta` do envelope de paginação.
///
/// O frontend usa `total`, `currentPage` e `lastPage`. Os demais campos são
/// emitidos por compatibilidade defensiva: telas antigas e integrações externas
/// podem ler as URLs, e omiti-las produziria `undefined` silencioso.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct PaginationMeta {
    #[ts(type = "number")]
    pub total: u64,
    #[ts(type = "number")]
    pub per_page: u64,
    #[ts(type = "number")]
    pub current_page: u64,
    #[ts(type = "number")]
    pub last_page: u64,
    #[ts(type = "number")]
    pub first_page: u64,
    pub first_page_url: String,
    pub last_page_url: String,
    pub next_page_url: Option<String>,
    pub previous_page_url: Option<String>,
}

impl PaginationMeta {
    /// Monta o `meta` a partir dos três números que o banco devolve.
    ///
    /// `last_page` nunca é 0, é 1 mesmo para conjunto vazio: o
    /// `useInfiniteList` compara `currentPage >= lastPage`, e com 0 ele pediria
    /// a página 1 para sempre.
    #[must_use]
    pub fn new(total: u64, per_page: u64, current_page: u64) -> Self {
        let per_page = per_page.max(1);
        let last_page = total.div_ceil(per_page).max(1);
        Self {
            total,
            per_page,
            current_page,
            last_page,
            first_page: 1,
            first_page_url: "/?page=1".to_string(),
            last_page_url: format!("/?page={last_page}"),
            next_page_url: (current_page < last_page)
                .then(|| format!("/?page={}", current_page + 1)),
            previous_page_url: (current_page > 1).then(|| format!("/?page={}", current_page - 1)),
        }
    }
}

/// `{ data, meta }` — o corpo que o `useInfiniteList` espera.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub meta: PaginationMeta,
}

/// Resposta dos endpoints de **modo dual**: array cru quando `?page` está
/// ausente, envelope paginado quando presente.
///
/// `untagged` faz o serde escolher pela forma do valor, sem campo
/// discriminador — o cliente distingue os dois casos olhando o JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MaybePaged<T> {
    Page(PaginatedResponse<T>),
    List(Vec<T>),
}

/// Aplica a regra de `limit`: default 20, teto 100, nunca 0.
#[must_use]
pub fn normalize_limit(limit: Option<u64>) -> u64 {
    limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
}

/// Página pedida, saneada. Página 0 (ou ausente) é a 1.
#[must_use]
pub fn normalize_page(page: Option<u64>) -> u64 {
    page.unwrap_or(1).max(1)
}

/// Executa `query` paginada e devolve o envelope `{ data, meta }`.
///
/// `map` converte a linha do banco no DTO de saída — a conversão acontece
/// **depois** da paginação, para o `total` continuar contando linhas do banco
/// e não itens filtrados.
///
/// # Errors
///
/// Propaga erro do banco em `AppError::Internal`.
pub async fn paginate<E, T, F, C>(
    db: &C,
    query: Select<E>,
    page: u64,
    limit: u64,
    map: F,
) -> AppResult<PaginatedResponse<T>>
where
    E: EntityTrait,
    E::Model: FromQueryResult + Send + Sync,
    C: ConnectionTrait,
    F: Fn(E::Model) -> T,
{
    let page = normalize_page(Some(page));
    let per_page = normalize_limit(Some(limit));

    let paginator = query.paginate(db, per_page);
    let total = paginator.num_items().await?;
    // O paginator do SeaORM é 0-based; o contrato HTTP é 1-based.
    let rows = paginator.fetch_page(page - 1).await?;

    Ok(PaginatedResponse {
        data: rows.into_iter().map(map).collect(),
        meta: PaginationMeta::new(total, per_page, page),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_reproduz_o_exemplo_do_roadmap() {
        let meta = PaginationMeta::new(137, 20, 3);
        assert_eq!(meta.total, 137);
        assert_eq!(meta.per_page, 20);
        assert_eq!(meta.current_page, 3);
        assert_eq!(meta.last_page, 7);
        assert_eq!(meta.first_page, 1);
        assert_eq!(meta.first_page_url, "/?page=1");
        assert_eq!(meta.last_page_url, "/?page=7");
        assert_eq!(meta.next_page_url.as_deref(), Some("/?page=4"));
        assert_eq!(meta.previous_page_url.as_deref(), Some("/?page=2"));
    }

    #[test]
    fn meta_serializa_em_camel_case() {
        let json = serde_json::to_value(PaginationMeta::new(137, 20, 3)).unwrap();
        for key in [
            "total",
            "perPage",
            "currentPage",
            "lastPage",
            "firstPage",
            "firstPageUrl",
            "lastPageUrl",
            "nextPageUrl",
            "previousPageUrl",
        ] {
            assert!(json.get(key).is_some(), "faltou `{key}` no meta");
        }
        assert!(json.get("per_page").is_none(), "vazou snake_case");
    }

    #[test]
    fn conjunto_vazio_tem_last_page_1() {
        let meta = PaginationMeta::new(0, 20, 1);
        assert_eq!(meta.last_page, 1);
        assert_eq!(meta.next_page_url, None);
        assert_eq!(meta.previous_page_url, None);
    }

    #[test]
    fn total_multiplo_exato_nao_cria_pagina_a_mais() {
        assert_eq!(PaginationMeta::new(40, 20, 1).last_page, 2);
        assert_eq!(PaginationMeta::new(41, 20, 1).last_page, 3);
    }

    /// Réplica da regra de parada do `useInfiniteList`:
    /// `isLastPage = !meta || meta.currentPage >= meta.lastPage`.
    fn is_last_page(meta: &PaginationMeta) -> bool {
        meta.current_page >= meta.last_page
    }

    #[test]
    fn use_infinite_list_percorre_todas_as_paginas_e_para() {
        let total = 137;
        let per_page = 20;
        let mut page = 1;
        let mut visitadas = 0;
        loop {
            let meta = PaginationMeta::new(total, per_page, page);
            visitadas += 1;
            if is_last_page(&meta) {
                break;
            }
            page += 1;
            assert!(visitadas < 50, "o laço do useInfiniteList não terminou");
        }
        assert_eq!(visitadas, 7);
    }

    #[test]
    fn use_infinite_list_para_na_primeira_pagina_quando_nao_ha_dados() {
        assert!(is_last_page(&PaginationMeta::new(0, 20, 1)));
    }

    #[test]
    fn pagina_curta_no_meio_nao_encerra_a_lista() {
        // O comentário do `useInfiniteList` diz que o fim vem do `meta`, não do
        // tamanho do lote — endpoints que filtram linhas depois de paginar
        // devolvem página curta no meio do conjunto.
        let meta = PaginationMeta::new(137, 20, 4);
        assert!(!is_last_page(&meta));
    }

    #[test]
    fn limite_e_pagina_sao_saneados_com_default_teto_e_minimo() {
        assert_eq!(normalize_limit(None), 20);
        assert_eq!(normalize_limit(Some(50)), 50);
        assert_eq!(normalize_limit(Some(1_000)), 100);
        assert_eq!(normalize_limit(Some(0)), 1);
        assert_eq!(normalize_page(None), 1);
        assert_eq!(normalize_page(Some(0)), 1);
        assert_eq!(normalize_page(Some(9)), 9);
    }

    #[test]
    fn maybe_paged_serializa_sem_tag_discriminadora() {
        let list: MaybePaged<i32> = MaybePaged::List(vec![1, 2, 3]);
        assert_eq!(
            serde_json::to_value(&list).unwrap(),
            serde_json::json!([1, 2, 3])
        );

        let paged = MaybePaged::Page(PaginatedResponse {
            data: vec![1],
            meta: PaginationMeta::new(1, 20, 1),
        });
        let json = serde_json::to_value(&paged).unwrap();
        assert_eq!(json["data"], serde_json::json!([1]));
        assert_eq!(json["meta"]["total"], 1);
    }
}
