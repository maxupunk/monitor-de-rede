//! Tarefas agendadas e as áreas do produto (ADR 017).
//!
//! Uma tarefa cuja ferramenta é de uma área desligada não roda: fica `skipped`,
//! com o motivo, e volta sozinha quando a área volta. Nada é apagado.

use netmonitor_core::{
    areas::{Area, Consequence, EnabledAreas},
    shared::text::count_noun,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter};

use crate::{
    models::_entities::scheduled_tasks,
    services::{
        ai::harness::tools::{area_of_tool, tool_names_in_area},
        shared::errors::AppResult,
    },
};

/// A consequência de desligar a área para as tarefas agendadas, se houver
/// alguma ligada que use ferramenta dela.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn consequence(db: &DatabaseConnection, area: Area) -> AppResult<Option<Consequence>> {
    let tools = tool_names_in_area(area);
    if tools.is_empty() {
        return Ok(None);
    }
    let count = scheduled_tasks::Entity::find()
        .filter(scheduled_tasks::Column::Enabled.eq(true))
        .filter(scheduled_tasks::Column::Tool.is_in(tools))
        .count(db)
        .await?;
    Ok((count > 0).then(|| {
        Consequence::new(format!(
            "{} usa{} ferramenta desta área e será pulada, com o motivo, até a área voltar.",
            count_noun(count, "tarefa agendada", "tarefas agendadas"),
            if count == 1 { "" } else { "m" }
        ))
    }))
}

/// A área desligada da ferramenta, se for o caso — o motivo do `skipped`.
#[must_use]
pub fn disabled_area(enabled: &EnabledAreas, tool: &str) -> Option<Area> {
    area_of_tool(tool).filter(|area| !enabled.is_enabled(*area))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ferramenta_de_area_desligada_e_pulada() {
        let mut enabled = EnabledAreas::all_on();
        assert_eq!(disabled_area(&enabled, "get_docker_containers"), None);
        enabled.set(Area::Servers, false);
        assert_eq!(
            disabled_area(&enabled, "get_docker_containers"),
            Some(Area::Servers)
        );
        assert_eq!(disabled_area(&enabled, "list_devices"), None, "núcleo");
        assert_eq!(disabled_area(&enabled, "nao_existe"), None);
    }
}
