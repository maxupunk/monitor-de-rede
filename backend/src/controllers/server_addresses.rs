//! Lista de endereços por onde os equipamentos alcançam este servidor.
//!
//! Controller extrai, valida, delega e serializa: o conceito, a detecção e a
//! validação vivem em [`crate::services::server_addresses`].

use loco_rs::prelude::*;

use crate::{
    dtos::server_addresses::{
        SaveServerAddressesInput, ServerAddressEntry, ServerAddressesResponse,
    },
    services::{
        server_addresses::{self, CustomAddress, ServerAddress, StoredAddresses},
        shared::errors::AppResult,
    },
};

/// `GET /api/server-addresses` — a lista resolvida.
async fn index(State(ctx): State<AppContext>) -> AppResult<Response> {
    let lista = server_addresses::list(&ctx.db, &server_addresses::nat_detector(&ctx)).await?;
    let documento = server_addresses::stored(&ctx.db).await?;
    Ok(format::json(ServerAddressesResponse {
        data: lista.into_iter().map(serializa).collect(),
        preferred_id: documento.preferred_id,
    })?)
}

/// `PUT /api/server-addresses` — grava o documento do operador.
async fn save(
    State(ctx): State<AppContext>,
    Json(entrada): Json<SaveServerAddressesInput>,
) -> AppResult<Response> {
    server_addresses::save(
        &ctx.db,
        StoredAddresses {
            overrides: entrada.overrides,
            custom: entrada
                .custom
                .into_iter()
                .map(|item| CustomAddress {
                    id: item.id,
                    label: item.label,
                    value: item.value,
                })
                .collect(),
            preferred_id: entrada
                .preferred_id
                .map(|id| id.trim().to_owned())
                .filter(|id| !id.is_empty()),
        },
    )
    .await?;

    // Devolve a lista já resolvida: a tela precisa dos ids que o servidor
    // sorteou para os itens novos, e de um segundo `GET` a menos.
    index(State(ctx)).await
}

fn serializa(endereco: ServerAddress) -> ServerAddressEntry {
    ServerAddressEntry {
        id: endereco.id,
        kind: endereco.kind.id().to_owned(),
        label: endereco.label,
        description: endereco.description,
        value: endereco.value,
        detected: endereco.detected,
        overridden: endereco.overridden,
        source: endereco.source,
    }
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/server-addresses")
        .add("/", get(index).put(save))
}
