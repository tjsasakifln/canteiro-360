//! Conferência documental interna. Não autentica documentos de conselhos nem concede dispensa legal.
use crate::{CivilDate, EntityId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObligationKind {
    Art,
    Rrt,
    Designation,
    License,
    Insurance,
    Guarantee,
    Cno,
}
impl ObligationKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Art => "ART",
            Self::Rrt => "RRT",
            Self::Designation => "Designação",
            Self::License => "Licença",
            Self::Insurance => "Seguro",
            Self::Guarantee => "Garantia",
            Self::Cno => "CNO",
        }
    }
    pub const fn requires_professional(self) -> bool {
        matches!(self, Self::Art | Self::Rrt | Self::Designation)
    }
    pub const fn source(self) -> (&'static str, &'static str) {
        match self {
            Self::Art => (
                "https://normativos.confea.org.br/Ementas/Visualizar?id=76099",
                "CONFEA 1.137/2023 consolidada, arts. 2, 4, 10, 12, 27 a 30; consulta 19/11/2026",
            ),
            Self::Rrt => (
                "https://transparencia.caubr.gov.br/resolucao184/",
                "CAU 91/2014, art. 2, redação da Resolução 184/2019; consulta 19/11/2026",
            ),
            Self::Cno => (
                "https://www.gov.br/receitafederal/pt-br/assuntos/construcao-civil/cno/cno",
                "Receita Federal, Cadastro Nacional de Obras; cadastro interno não substitui inscrição",
            ),
            _ => (
                "",
                "Requisito específico da atividade ou do instrumento indicado pelo responsável",
            ),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TechnicalActivity {
    Budget,
    Design,
    Execution,
    Inspection,
    Other,
}
impl TechnicalActivity {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Budget => "Orçamento",
            Self::Design => "Projeto",
            Self::Execution => "Execução",
            Self::Inspection => "Fiscalização",
            Self::Other => "Outra atividade",
        }
    }
}

/// Evento normativo informado conforme o art. 2º da Resolução CAU 91/2014.
/// Para o inciso II, deve ser o primeiro evento aplicável, não uma data de escolha.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RrtEvent {
    ProjectCompletion,
    FinalDelivery,
    Submission,
    Publication,
    OtherActivityCompletion,
}
impl RrtEvent {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ProjectCompletion => "Término do projeto (art. 2º, II)",
            Self::FinalDelivery => "Entrega final dos documentos (art. 2º, II)",
            Self::Submission => "Protocolo para análise ou aprovação (art. 2º, II)",
            Self::Publication => "Publicação ou divulgação (art. 2º, II)",
            Self::OtherActivityCompletion => "Término de outra atividade (art. 2º, III)",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RrtTiming {
    pub framework: RrtFramework,
    pub event: RrtEvent,
    pub event_on: CivilDate,
}
/// Enquadramento declarado segundo os grupos e atividades da Resolução CAU 21/2012.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RrtFramework {
    Project,
    EnvironmentPlanning,
    ProjectCoordination,
    SafetySystemDesign,
    FireProtectionDesign,
    OtherActivities,
}
impl RrtFramework {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Project => "Grupo 1: Projeto, incluindo orçamento 1.7.3 ou 1.10.3",
            Self::EnvironmentPlanning => "Grupo 4: Meio Ambiente e Planejamento Regional e Urbano",
            Self::ProjectCoordination => {
                "Atividade 3.1: Coordenação e Compatibilização de Projetos"
            }
            Self::SafetySystemDesign => "Atividade 7.8.12: Projeto de Sistema de Segurança",
            Self::FireProtectionDesign => "Atividade 7.8.13: Projeto de Proteção Contra Incêndios",
            Self::OtherActivities => "Demais atividades: art. 2º, III, incluindo fiscalização 3.6",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TechnicalException {
    pub source_reference: String,
    pub source_url: String,
    pub reasoning: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationSpec {
    pub kind: ObligationKind,
    pub activity: TechnicalActivity,
    pub discipline: String,
    pub description: String,
    pub professional_id: Option<EntityId>,
    pub number: String,
    pub issuer: String,
    pub valid_from: CivilDate,
    pub valid_until: Option<CivilDate>,
    pub activity_on: CivilDate,
    pub registered_on: Option<CivilDate>,
    pub rrt_timing: Option<RrtTiming>,
    pub document_id: Option<EntityId>,
    pub document_version: Option<i64>,
    pub exception: Option<TechnicalException>,
    pub replaces_id: Option<EntityId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ObligationError {
    #[error(
        "Informe disciplina, descrição, emissor e identificação do documento nos limites indicados."
    )]
    InvalidText,
    #[error(
        "Confira a vigência e a data da atividade. O documento precisa cobrir o evento informado."
    )]
    InvalidPeriod,
    #[error("Indique o profissional responsável pela atividade.")]
    ProfessionalRequired,
    #[error(
        "Vincule uma versão de arquivo aceita. Número provisório, exemplo ou texto genérico não comprova a obrigação."
    )]
    EvidenceRequired,
    #[error(
        "A exceção exige fonte identificável e fundamento específico. Uma dispensa de campo no SICOP não é dispensa legal."
    )]
    InvalidException,
    #[error(
        "O registro ultrapassa o evento aplicável ou o prazo de 30 dias das demais atividades. Confira o enquadramento e as datas."
    )]
    RegistrationTiming,
    #[error(
        "Informe o primeiro evento aplicável ao RRT de projeto ou o término da outra atividade. Confira o art. 2º da Resolução CAU 91/2014."
    )]
    RrtEventRequired,
    #[error("Informe CREA para ART e CAU para RRT, conforme o documento do conselho.")]
    WrongCouncil,
}

fn bounded(s: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&s.trim().chars().count())
        && !s.chars().any(|c| c.is_control() && c != '\n' && c != '\t')
}
pub fn placeholder(s: &str) -> bool {
    let value = s.trim().to_uppercase();
    value.is_empty()
        || [
            "TESTE",
            "TEST",
            "EXEMPLO",
            "FICT",
            "PENDENTE",
            "PROVIS",
            "A EMITIR",
            "EM EMISS",
            "N/A",
            "NAO SE APLICA",
            "NÃO SE APLICA",
            "SEM NUMERO",
            "SEM NÚMERO",
        ]
        .iter()
        .any(|p| value.contains(p))
        || value
            .chars()
            .all(|c| c == '0' || c == '-' || c == '/' || c == '.' || c.is_whitespace())
}
impl ObligationSpec {
    pub fn validate(&self, documented: bool) -> Result<(), ObligationError> {
        if !bounded(&self.discipline, 2, 80)
            || !bounded(&self.description, 5, 2000)
            || !bounded(&self.number, 0, 80)
            || !bounded(&self.issuer, 0, 160)
        {
            return Err(ObligationError::InvalidText);
        }
        if self
            .valid_until
            .is_some_and(|until| until < self.valid_from)
            || self.activity_on < self.valid_from
            || self
                .valid_until
                .is_some_and(|until| self.activity_on > until)
        {
            return Err(ObligationError::InvalidPeriod);
        }
        if self.document_id.is_some() != self.document_version.is_some()
            || self.document_version.is_some_and(|v| v < 1)
        {
            return Err(ObligationError::EvidenceRequired);
        }
        if let Some(ex) = &self.exception
            && !ex.source_url.is_empty()
            && (!ex.source_url.starts_with("https://")
                || ex.source_url.len() > 500
                || ex.source_url.chars().any(char::is_whitespace))
        {
            return Err(ObligationError::InvalidException);
        }
        if !documented {
            return Ok(());
        }
        if self.kind.requires_professional() && self.professional_id.is_none() {
            return Err(ObligationError::ProfessionalRequired);
        }
        if self.document_id.is_none() {
            return Err(ObligationError::EvidenceRequired);
        }
        if let Some(ex) = &self.exception {
            if !bounded(&ex.source_reference, 12, 240)
                || !bounded(&ex.reasoning, 40, 2000)
                || placeholder(&ex.reasoning)
                || !ex.source_url.starts_with("https://")
                || ex.source_url.len() > 500
                || ex.source_url.chars().any(char::is_whitespace)
                || ex.reasoning.to_lowercase().contains("sicop dispensa")
            {
                return Err(ObligationError::InvalidException);
            }
            return Ok(());
        }
        if !bounded(&self.number, 3, 80)
            || placeholder(&self.number)
            || !bounded(&self.issuer, 3, 160)
        {
            return Err(ObligationError::EvidenceRequired);
        }
        let issuer = self.issuer.trim().to_uppercase();
        if (self.kind == ObligationKind::Art && !issuer.starts_with("CREA"))
            || (self.kind == ObligationKind::Rrt && !issuer.starts_with("CAU"))
        {
            return Err(ObligationError::WrongCouncil);
        }
        if matches!(self.kind, ObligationKind::Art | ObligationKind::Rrt)
            && self.registered_on.is_none()
        {
            return Err(ObligationError::EvidenceRequired);
        }
        if (self.kind == ObligationKind::Art
            || (self.kind == ObligationKind::Rrt && self.activity == TechnicalActivity::Execution))
            && self
                .registered_on
                .is_some_and(|date| date > self.activity_on)
        {
            return Err(ObligationError::RegistrationTiming);
        }
        if self.kind == ObligationKind::Rrt && self.activity != TechnicalActivity::Execution {
            let timing = self
                .rrt_timing
                .as_ref()
                .ok_or(ObligationError::RrtEventRequired)?;
            let other = timing.framework == RrtFramework::OtherActivities;
            if timing.event_on < self.activity_on
                || other != (timing.event == RrtEvent::OtherActivityCompletion)
                || (self.activity == TechnicalActivity::Budget
                    && timing.framework != RrtFramework::Project)
                || (self.activity == TechnicalActivity::Design && other)
                || (self.activity == TechnicalActivity::Inspection && !other)
            {
                return Err(ObligationError::RrtEventRequired);
            }
            let registered = self
                .registered_on
                .ok_or(ObligationError::EvidenceRequired)?;
            if registered > timing.event_on {
                return Err(ObligationError::RegistrationTiming);
            }
            if other
                && registered
                    .as_naive_date()
                    .signed_duration_since(self.activity_on.as_naive_date())
                    .num_days()
                    > 30
            {
                return Err(ObligationError::RegistrationTiming);
            }
        }
        Ok(())
    }
    /// Não usa outra atividade ou disciplina como prova automática.
    pub fn covers(&self, activity: TechnicalActivity, discipline: &str, at: CivilDate) -> bool {
        self.validate(true).is_ok()
            && self.activity == activity
            && self
                .discipline
                .trim()
                .eq_ignore_ascii_case(discipline.trim())
            && at >= self.valid_from
            && self.valid_until.is_none_or(|until| at <= until)
    }
}
