//! Execuções das tarefas agendadas (`services::scheduled_tasks`).

use sea_orm::entity::prelude::*;

pub use super::_entities::scheduled_task_runs::{ActiveModel, Column, Entity, Model};

impl ActiveModelBehavior for ActiveModel {}
