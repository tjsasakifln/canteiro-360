//! Projetos independentes de obra ou contrato. Uma entrega aceita não tem efeito financeiro.
use crate::{CivilDate, EntityId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectDelivery {
    pub id: EntityId,
    pub title: String,
    pub responsible_id: Option<EntityId>,
    pub acceptance_criteria: String,
    pub due_on: Option<CivilDate>,
    pub milestone: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectSpec {
    pub title: String,
    pub discipline: String,
    pub objective: String,
    pub scope: String,
    pub external_reference: String,
    pub work_id: Option<EntityId>,
    pub stakeholder_ids: Vec<EntityId>,
    pub deliveries: Vec<ProjectDelivery>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    Draft,
    Approved,
    InProgress,
    Closed,
}

impl ProjectStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Draft => "Em elaboração",
            Self::Approved => "Escopo aprovado",
            Self::InProgress => "Em execução",
            Self::Closed => "Encerrado",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectSnapshot {
    pub spec: ProjectSpec,
    pub status: ProjectStatus,
    pub accepted_deliveries: Vec<EntityId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ProjectError {
    #[error("Informe título, disciplina, objetivo e escopo nos limites indicados.")]
    InvalidText,
    #[error("Use até 100 entregas e 50 partes interessadas, sem repetir identificações.")]
    InvalidList,
    #[error(
        "Cada compromisso precisa de responsável, prazo e critério de aceite antes da aprovação."
    )]
    IncompleteCommitment,
    #[error("A ação não está disponível neste estado. Confira o escopo aprovado e a versão atual.")]
    InvalidState,
    #[error("A entrega não pertence a esta versão ou já foi aceita.")]
    InvalidDelivery,
    #[error("Todas as entregas precisam ser aceitas antes do encerramento.")]
    PendingDelivery,
}

fn text(value: &str, minimum: usize, maximum: usize) -> bool {
    (minimum..=maximum).contains(&value.trim().chars().count())
        && !value
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
}

impl ProjectSpec {
    pub fn validate(&self, approval: bool) -> Result<(), ProjectError> {
        if !text(&self.title, 2, 160)
            || !text(&self.discipline, 2, 80)
            || !text(&self.objective, 2, 1000)
            || !text(&self.scope, 2, 8000)
            || !text(&self.external_reference, 0, 120)
        {
            return Err(ProjectError::InvalidText);
        }
        let deliveries: BTreeSet<_> = self.deliveries.iter().map(|d| d.id).collect();
        let stakeholders: BTreeSet<_> = self.stakeholder_ids.iter().copied().collect();
        if self.deliveries.len() > 100
            || self.stakeholder_ids.len() > 50
            || deliveries.len() != self.deliveries.len()
            || stakeholders.len() != self.stakeholder_ids.len()
        {
            return Err(ProjectError::InvalidList);
        }
        if approval && self.deliveries.is_empty() {
            return Err(ProjectError::IncompleteCommitment);
        }
        for d in &self.deliveries {
            if !text(&d.title, 2, 160) || !text(&d.acceptance_criteria, 0, 2000) {
                return Err(ProjectError::InvalidText);
            }
            if approval
                && (d.responsible_id.is_none()
                    || d.due_on.is_none()
                    || !text(&d.acceptance_criteria, 5, 2000))
            {
                return Err(ProjectError::IncompleteCommitment);
            }
        }
        Ok(())
    }
}

impl ProjectSnapshot {
    pub fn draft(spec: ProjectSpec) -> Result<Self, ProjectError> {
        spec.validate(false)?;
        Ok(Self {
            spec,
            status: ProjectStatus::Draft,
            accepted_deliveries: vec![],
        })
    }
    /// Uma edição sempre produz nova revisão em elaboração; o chamador preserva a anterior.
    pub fn revise(&self, spec: ProjectSpec) -> Result<Self, ProjectError> {
        if self.status == ProjectStatus::Closed {
            return Err(ProjectError::InvalidState);
        }
        Self::draft(spec)
    }
    pub fn approve(&self) -> Result<Self, ProjectError> {
        if self.status != ProjectStatus::Draft {
            return Err(ProjectError::InvalidState);
        }
        self.spec.validate(true)?;
        let mut next = self.clone();
        next.status = ProjectStatus::Approved;
        Ok(next)
    }
    pub fn start(&self) -> Result<Self, ProjectError> {
        if self.status != ProjectStatus::Approved {
            return Err(ProjectError::InvalidState);
        }
        let mut next = self.clone();
        next.status = ProjectStatus::InProgress;
        Ok(next)
    }
    pub fn accept(&self, delivery: EntityId) -> Result<Self, ProjectError> {
        if !matches!(
            self.status,
            ProjectStatus::Approved | ProjectStatus::InProgress
        ) {
            return Err(ProjectError::InvalidState);
        }
        if !self.spec.deliveries.iter().any(|d| d.id == delivery)
            || self.accepted_deliveries.contains(&delivery)
        {
            return Err(ProjectError::InvalidDelivery);
        }
        let mut next = self.clone();
        next.accepted_deliveries.push(delivery);
        next.accepted_deliveries.sort();
        Ok(next)
    }
    pub fn close(&self) -> Result<Self, ProjectError> {
        if !matches!(
            self.status,
            ProjectStatus::Approved | ProjectStatus::InProgress
        ) {
            return Err(ProjectError::InvalidState);
        }
        if self
            .spec
            .deliveries
            .iter()
            .any(|d| !self.accepted_deliveries.contains(&d.id))
        {
            return Err(ProjectError::PendingDelivery);
        }
        let mut next = self.clone();
        next.status = ProjectStatus::Closed;
        Ok(next)
    }
}
