//! Lotes de importação identificados, vinculados a sessão e revisão histórica.
//! O chamador mantém a transação aberta até persistir a revisão e concluir o lote.
use crate::auth::{AuthStoreError, AuthTransaction};
use canteiro_domain::budgets::{BudgetRevision, BudgetState};
use serde_json::Value;
use sqlx::types::Json;
use uuid::Uuid;

/// Uma prévia validada. O SHA-256 deve ser calculado sobre o arquivo integral
/// recebido, não sobre apenas as linhas aprovadas.
pub struct NewBudgetImport<'a> {
    pub tenant: Uuid,
    pub id: Uuid,
    pub scope: Uuid,
    pub actor: Uuid,
    pub session: Uuid,
    pub base_version: Option<i64>,
    pub format: &'a str,
    pub locale: &'a str,
    pub sha256: &'a [u8; 32],
    pub preview: &'a str,
    pub included: i32,
    pub changed: i32,
    pub removed: i32,
}

impl AuthTransaction {
    /// Cria uma prévia durável sem editar o orçamento. Repetição idêntica
    /// retorna o mesmo identificador; conteúdo divergente jamais é reutilizado.
    pub async fn budget_import_stage(
        &mut self,
        batch: &NewBudgetImport<'_>,
    ) -> Result<Option<Uuid>, AuthStoreError> {
        let payload: Value = serde_json::from_str(batch.preview).map_err(|_| AuthStoreError)?;
        if !payload.is_object()
            || batch.base_version.is_some_and(|version| version <= 0)
            || batch.included < 0
            || batch.changed < 0
            || batch.removed < 0
            || !matches!(batch.locale, "canonical" | "pt-BR")
            || !matches!(
                batch.format,
                canteiro_domain::budget_csv::FORMAT_VERSION
                    | canteiro_domain::budget_xlsx::XLSX_VERSION
            )
        {
            return Err(AuthStoreError);
        }
        // A gravação de uma prévia não deve aceitar um JSON arbitrário,
        // mesmo quando o chamador afirma que já verificou o arquivo.
        // A revisão vinculada ao lote é sempre um rascunho do escopo esperado.
        let expected_revision = batch
            .base_version
            .unwrap_or(0)
            .checked_add(1)
            .and_then(|value| u32::try_from(value).ok());
        let Ok(revision) = serde_json::from_str::<BudgetRevision>(batch.preview) else {
            return Ok(None);
        };
        if Some(revision.revision) != expected_revision
            || revision.budget_id != batch.scope.to_string()
            || revision.status != BudgetState::Draft
            || revision.summarize().is_err()
        {
            return Ok(None);
        }
        // O relatório persistido precisa corresponder à revisão-base real,
        // inclusive para chamadas que não passam pela prévia HTTP.
        let current = self.budget_current(batch.tenant, batch.scope).await?;
        if current.as_ref().map(|stored| stored.version) != batch.base_version {
            return Ok(None);
        }
        let difference = if let Some(stored) = current {
            if stored.state == "approved" {
                return Ok(None);
            }
            let previous: BudgetRevision =
                serde_json::from_str(&stored.payload).map_err(|_| AuthStoreError)?;
            canteiro_domain::budget_csv::compare(&previous, &revision)
        } else {
            canteiro_domain::budget_csv::ImportDifference {
                included: revision.lines.len(),
                changed: 0,
                removed: 0,
            }
        };
        if (difference.included, difference.changed, difference.removed)
            != (
                batch.included as usize,
                batch.changed as usize,
                batch.removed as usize,
            )
        {
            return Ok(None);
        }
        // Expiração não autoriza reaplicar o lote antigo. Cancela somente
        // a prévia vencida da mesma sessão e mantém seu histórico imutável.
        sqlx::query(
            "UPDATE canteiro.budget_import_batches SET status='cancelled'
             WHERE tenant_id=$1 AND scope_id=$2 AND actor_id=$3 AND session_id=$4
               AND base_version IS NOT DISTINCT FROM $5 AND sha256=$6
               AND format=$7 AND status='ready' AND expires_at<=clock_timestamp()",
        )
        .bind(batch.tenant)
        .bind(batch.scope)
        .bind(batch.actor)
        .bind(batch.session)
        .bind(batch.base_version)
        .bind(batch.sha256.as_slice())
        .bind(batch.format)
        .execute(&mut *self.tx)
        .await?;
        let inserted: Option<Uuid> = sqlx::query_scalar(
            "INSERT INTO canteiro.budget_import_batches
               (tenant_id,id,scope_id,actor_id,session_id,base_version,format,locale,
                sha256,preview,included_count,changed_count,removed_count)
             VALUES ($1,$2,$3,$4,$5,$6,$13,$7,$8,$9,$10,$11,$12)
             ON CONFLICT DO NOTHING RETURNING id",
        )
        .bind(batch.tenant)
        .bind(batch.id)
        .bind(batch.scope)
        .bind(batch.actor)
        .bind(batch.session)
        .bind(batch.base_version)
        .bind(batch.locale)
        .bind(batch.sha256.as_slice())
        .bind(Json(&payload))
        .bind(batch.included)
        .bind(batch.changed)
        .bind(batch.removed)
        .bind(batch.format)
        .fetch_optional(&mut *self.tx)
        .await?;
        if inserted.is_some() {
            return Ok(inserted);
        }
        // Conflito de chave não transforma dados diferentes em importação válida.
        let prior: Option<(Uuid, Json<Value>, String, i32, i32, i32)> = sqlx::query_as(
            "SELECT id,preview,status,included_count,changed_count,removed_count
             FROM canteiro.budget_import_batches
             WHERE tenant_id=$1 AND scope_id=$2 AND actor_id=$3 AND session_id=$4
               AND base_version IS NOT DISTINCT FROM $5 AND sha256=$6 AND format=$8
               AND locale=$7 AND status='ready' AND expires_at>clock_timestamp()",
        )
        .bind(batch.tenant)
        .bind(batch.scope)
        .bind(batch.actor)
        .bind(batch.session)
        .bind(batch.base_version)
        .bind(batch.sha256.as_slice())
        .bind(batch.locale)
        .bind(batch.format)
        .fetch_optional(&mut *self.tx)
        .await?;
        match prior {
            Some((id, Json(existing), status, included, changed, removed))
                if status == "ready"
                    && existing == payload
                    && (included, changed, removed)
                        == (batch.included, batch.changed, batch.removed) =>
            {
                Ok(Some(id))
            }
            _ => Ok(None),
        }
    }

    /// Recupera somente a prévia ainda disponível da sessão proprietária.
    /// O chamador revalida a permissão e a revisão-base antes de apresentar dados.
    pub async fn budget_import_ready_snapshot(
        &mut self,
        tenant: Uuid,
        scope: Uuid,
        actor: Uuid,
        session: Uuid,
        batch: Uuid,
    ) -> Result<Option<(Option<i64>, String)>, AuthStoreError> {
        Ok(sqlx::query_as(
            "SELECT base_version,preview::TEXT
             FROM canteiro.budget_import_batches
             WHERE tenant_id=$1 AND scope_id=$2 AND actor_id=$3
               AND session_id=$4 AND id=$5 AND status='ready'
               AND expires_at>clock_timestamp()",
        )
        .bind(tenant)
        .bind(scope)
        .bind(actor)
        .bind(session)
        .bind(batch)
        .fetch_optional(&mut *self.tx)
        .await?)
    }

    /// Identifica uma operação de importação da sessão sem confundi-la com um
    /// comando de edição ordinário.
    pub async fn budget_import_exists(
        &mut self,
        tenant: Uuid,
        scope: Uuid,
        actor: Uuid,
        session: Uuid,
        id: Uuid,
    ) -> Result<bool, AuthStoreError> {
        Ok(sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (
                SELECT 1 FROM canteiro.budget_import_batches
                WHERE tenant_id=$1 AND scope_id=$2 AND actor_id=$3
                  AND session_id=$4 AND id=$5
            )",
        )
        .bind(tenant)
        .bind(scope)
        .bind(actor)
        .bind(session)
        .bind(id)
        .fetch_one(&mut *self.tx)
        .await?)
    }

    /// Só confirma o lote se a revisão imediatamente posterior existe NA MESMA
    /// transação, pertence ao autor e possui o snapshot validado, sem substituições.
    /// Uma falha posterior deve cancelar a transação inteira do chamador.
    pub async fn budget_import_mark_applied(
        &mut self,
        tenant: Uuid,
        scope: Uuid,
        actor: Uuid,
        session: Uuid,
        batch: Uuid,
        operation: Uuid,
    ) -> Result<bool, AuthStoreError> {
        Ok(sqlx::query_scalar::<_, Uuid>(
            "UPDATE canteiro.budget_import_batches b
             SET status='applied',applied_at=clock_timestamp()
             WHERE b.tenant_id=$1 AND b.scope_id=$2 AND b.actor_id=$3
               AND b.session_id=$4 AND b.id=$5
               AND b.status='ready' AND b.expires_at>clock_timestamp() AND b.id=$6
               AND EXISTS (
                   SELECT 1 FROM canteiro.budget_versions v
                    JOIN canteiro.budget_records r
                      ON r.tenant_id=v.tenant_id AND r.scope_id=v.scope_id
                     AND r.current_version=v.version
                    WHERE v.tenant_id=b.tenant_id AND v.scope_id=b.scope_id
                      AND v.version=COALESCE(b.base_version,0)+1 AND v.actor_id=b.actor_id
                      AND v.operation_key=$6 AND v.payload=b.preview AND v.state='draft'
               )
             RETURNING b.id",
        )
        .bind(tenant)
        .bind(scope)
        .bind(actor)
        .bind(session)
        .bind(batch)
        .bind(operation)
        .fetch_optional(&mut *self.tx)
        .await?
        .is_some())
    }
}
