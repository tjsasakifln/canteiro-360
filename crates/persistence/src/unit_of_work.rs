use crate::{Database, PersistenceError, PersistenceErrorKind, error::operation_id};
use canteiro_domain::{
    ActorId, CivilDate, EntityId, EntityVersion, OperationId, TenantId, UtcInstant,
};
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{Postgres, Transaction, postgres::PgArguments, query::Query};
use tokio::time::{Instant, timeout, timeout_at};
use uuid::Uuid;

/// Somente código do servidor deve construir este contexto após autorizar empresa e ato.
/// A autenticação e a verificação dos vínculos serão ligadas nas issues 6 a 8.
#[derive(Clone, Copy, Debug)]
pub struct TransactionContext {
    tenant_id: TenantId,
    actor_id: ActorId,
    operation_id: OperationId,
}
impl TransactionContext {
    pub fn new(tenant_id: TenantId, actor_id: ActorId) -> Self {
        Self {
            tenant_id,
            actor_id,
            operation_id: operation_id(),
        }
    }
    pub fn operation_id(&self) -> OperationId {
        self.operation_id
    }
    pub fn tenant_id(&self) -> TenantId {
        self.tenant_id
    }
}

/// Resultado disponível somente depois de confirmação positiva do COMMIT.
#[derive(Debug)]
#[must_use]
pub struct Committed<T> {
    value: T,
    operation_id: OperationId,
}
impl<T> Committed<T> {
    pub fn into_inner(self) -> T {
        self.value
    }
    pub fn operation_id(&self) -> OperationId {
        self.operation_id
    }
}

#[derive(Debug)]
pub struct NewUnit<'a> {
    pub id: EntityId,
    pub parent_id: Option<EntityId>,
    pub name: &'a str,
    pub effective_on: Option<CivilDate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitRecord {
    pub tenant_id: TenantId,
    pub id: EntityId,
    pub parent_id: Option<EntityId>,
    pub name: String,
    pub effective_on: Option<CivilDate>,
    pub version: EntityVersion,
    pub created_at: UtcInstant,
    pub updated_at: UtcInstant,
    pub created_by: ActorId,
    pub updated_by: ActorId,
}

/// Transação que não expõe conexão ou executor. Erro envenena a unidade, inclusive
/// quando o chamador ignora o Result. Drop/cancelamento herda o rollback do SQLx.
#[must_use = "Conclua com commit ou rollback; descartar nunca grava"]
pub struct UnitOfWork {
    pub(crate) transaction: Option<Transaction<'static, Postgres>>,
    context: TransactionContext,
    deadline: Instant,
    rollback_limit: std::time::Duration,
    failure: Option<PersistenceError>,
    in_flight: bool,
}

impl Database {
    pub async fn begin(&self, context: TransactionContext) -> Result<UnitOfWork, PersistenceError> {
        let transaction = timeout(self.limits.acquire, self.pool.begin())
            .await
            .map_err(|_| {
                PersistenceError::new(PersistenceErrorKind::Timeout, context.operation_id)
            })?
            .map_err(|e| PersistenceError::from_sql(&e, context.operation_id))?;
        let mut work = UnitOfWork {
            transaction: Some(transaction),
            context,
            deadline: Instant::now() + self.limits.transaction,
            rollback_limit: self.limits.statement,
            failure: None,
            in_flight: false,
        };
        work.execute(sqlx::query(
            "SET TRANSACTION ISOLATION LEVEL READ COMMITTED",
        ))
        .await?;
        work.execute(sqlx::query(
            "SELECT set_config('canteiro.tenant_id', $1, true), set_config('canteiro.actor_id', $2, true), \
             set_config('canteiro.operation_id', $3, true), set_config('statement_timeout', $4, true), \
             set_config('lock_timeout', $5, true), set_config('idle_in_transaction_session_timeout', $6, true)")
            .bind(context.tenant_id.to_string()).bind(context.actor_id.to_string()).bind(context.operation_id.to_string())
            .bind(format!("{}ms", self.limits.statement.as_millis()))
            .bind(format!("{}ms", self.limits.lock.as_millis()))
            .bind(format!("{}ms", self.limits.transaction.as_millis()))).await?;
        Ok(work)
    }
}

impl UnitOfWork {
    pub fn operation_id(&self) -> OperationId {
        self.context.operation_id
    }

    async fn guard(&mut self) -> Result<(), PersistenceError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        // Um futuro emprestando &mut self pode ter sido cancelado enquanto PostgreSQL
        // ainda executava a consulta. Nunca permitir commit dessa operação interrompida.
        if self.in_flight || Instant::now() >= self.deadline {
            return Err(self
                .abort(PersistenceError::new(
                    PersistenceErrorKind::Timeout,
                    self.context.operation_id,
                ))
                .await);
        }
        Ok(())
    }

    async fn abort(&mut self, error: PersistenceError) -> PersistenceError {
        self.failure = Some(error);
        if let Some(tx) = self.transaction.take() {
            // Nenhum COMMIT foi emitido. Se a confirmação do rollback falhar, o Drop
            // descarta/limpa a conexão; não há repetição automática do comando de negócio.
            let _ = timeout(self.rollback_limit, tx.rollback()).await;
        }
        error
    }

    pub(crate) async fn execute(
        &mut self,
        query: Query<'_, Postgres, PgArguments>,
    ) -> Result<u64, PersistenceError> {
        self.guard().await?;
        let tx = self
            .transaction
            .as_mut()
            .expect("transação ativa após guard");
        self.in_flight = true;
        let result = timeout_at(self.deadline, query.execute(&mut **tx)).await;
        self.in_flight = false;
        match result {
            Ok(Ok(result)) => Ok(result.rows_affected()),
            Ok(Err(error)) => Err(self
                .abort(PersistenceError::from_sql(
                    &error,
                    self.context.operation_id,
                ))
                .await),
            Err(_) => Err(self
                .abort(PersistenceError::new(
                    PersistenceErrorKind::Timeout,
                    self.context.operation_id,
                ))
                .await),
        }
    }

    /// O registro devolvido é provisório até commit(). Nenhuma rota pública é criada aqui.
    pub async fn insert_unit(
        &mut self,
        input: NewUnit<'_>,
    ) -> Result<UnitRecord, PersistenceError> {
        self.validate_name(input.name).await?;
        self.execute(sqlx::query("INSERT INTO canteiro.organization_units \
            (tenant_id, id, parent_id, name, effective_on, created_by, updated_by) VALUES ($1,$2,$3,$4,$5,$6,$6)")
            .bind(self.context.tenant_id.as_uuid()).bind(input.id.as_uuid())
            .bind(input.parent_id.map(|id| id.as_uuid())).bind(input.name)
            .bind(input.effective_on.map(|date| date.as_naive_date())).bind(self.context.actor_id.as_uuid())).await?;
        self.unit(input.id).await
    }

    pub async fn unit(&mut self, id: EntityId) -> Result<UnitRecord, PersistenceError> {
        self.guard().await?;
        type Row = (
            Uuid,
            Option<Uuid>,
            String,
            Option<NaiveDate>,
            i64,
            DateTime<Utc>,
            DateTime<Utc>,
            Uuid,
            Uuid,
        );
        let tx = self
            .transaction
            .as_mut()
            .expect("transação ativa após guard");
        self.in_flight = true;
        let result = timeout_at(self.deadline, sqlx::query_as::<_, Row>(
            "SELECT id, parent_id, name, effective_on, version, created_at, updated_at, created_by, updated_by \
             FROM canteiro.organization_units WHERE tenant_id = $1 AND id = $2")
            .bind(self.context.tenant_id.as_uuid()).bind(id.as_uuid()).fetch_one(&mut **tx)).await;
        self.in_flight = false;
        let row = match result {
            Ok(Ok(row)) => row,
            Ok(Err(error)) => {
                return Err(self
                    .abort(PersistenceError::from_sql(
                        &error,
                        self.context.operation_id,
                    ))
                    .await);
            }
            Err(_) => {
                return Err(self
                    .abort(PersistenceError::new(
                        PersistenceErrorKind::Timeout,
                        self.context.operation_id,
                    ))
                    .await);
            }
        };
        let convert = || -> Result<UnitRecord, canteiro_domain::InvalidValue> {
            Ok(UnitRecord {
                tenant_id: self.context.tenant_id,
                id: EntityId::from_uuid(row.0)?,
                parent_id: row.1.map(EntityId::from_uuid).transpose()?,
                name: row.2,
                effective_on: row.3.map(CivilDate::from_naive_date).transpose()?,
                version: EntityVersion::new(row.4)?,
                created_at: UtcInstant::from_datetime(row.5)?,
                updated_at: UtcInstant::from_datetime(row.6)?,
                created_by: ActorId::from_uuid(row.7)?,
                updated_by: ActorId::from_uuid(row.8)?,
            })
        };
        match convert() {
            Ok(record) => Ok(record),
            Err(_) => Err(self
                .abort(PersistenceError::new(
                    PersistenceErrorKind::Internal,
                    self.context.operation_id,
                ))
                .await),
        }
    }

    pub async fn rename_unit(
        &mut self,
        id: EntityId,
        expected: EntityVersion,
        name: &str,
    ) -> Result<UnitRecord, PersistenceError> {
        self.validate_name(name).await?;
        let next = match expected.next() {
            Ok(next) => next,
            Err(_) => {
                return Err(self
                    .abort(PersistenceError::new(
                        PersistenceErrorKind::InvalidInput,
                        self.context.operation_id,
                    ))
                    .await);
            }
        };
        let changed = self
            .execute(
                sqlx::query(
                    "UPDATE canteiro.organization_units SET name = $1, \
            version = $2, updated_by = $3, updated_at = GREATEST(clock_timestamp(), created_at) \
            WHERE tenant_id = $4 AND id = $5 AND version = $6",
                )
                .bind(name)
                .bind(next.get())
                .bind(self.context.actor_id.as_uuid())
                .bind(self.context.tenant_id.as_uuid())
                .bind(id.as_uuid())
                .bind(expected.get()),
            )
            .await?;
        if changed != 1 {
            // Ausência e acesso a outra empresa não provam versão desatualizada.
            // A consulta usa o mesmo tenant e preserva a resposta de indisponibilidade.
            self.unit(id).await?;
            return Err(self
                .abort(PersistenceError::new(
                    PersistenceErrorKind::VersionConflict,
                    self.context.operation_id,
                ))
                .await);
        }
        self.unit(id).await
    }

    async fn validate_name(&mut self, name: &str) -> Result<(), PersistenceError> {
        self.guard().await?;
        if name.trim().is_empty()
            || name.chars().count() > 160
            || name.chars().any(char::is_control)
        {
            return Err(self
                .abort(PersistenceError::new(
                    PersistenceErrorKind::InvalidInput,
                    self.context.operation_id,
                ))
                .await);
        }
        Ok(())
    }

    pub async fn rollback(mut self) -> Result<(), PersistenceError> {
        if let Some(tx) = self.transaction.take() {
            match timeout(self.rollback_limit, tx.rollback()).await {
                Ok(Ok(())) => Ok(()),
                Ok(Err(error)) => Err(PersistenceError::from_sql(
                    &error,
                    self.context.operation_id,
                )),
                Err(_) => Err(PersistenceError::new(
                    PersistenceErrorKind::Timeout,
                    self.context.operation_id,
                )),
            }
        } else {
            Ok(())
        }
    }

    pub async fn commit<T>(mut self, value: T) -> Result<Committed<T>, PersistenceError> {
        self.guard().await?;
        // Um SELECT dentro da transação detecta inclusive erro que um futuro adaptador
        // SQL tenha deixado de propagar. COMMIT de transação abortada não é sucesso.
        self.execute(sqlx::query("SELECT 1")).await?;
        let tx = self.transaction.take().expect("transação ativa após guard");
        match timeout_at(self.deadline, tx.commit()).await {
            Ok(Ok(())) => Ok(Committed {
                value,
                operation_id: self.context.operation_id,
            }),
            // Conservador: após iniciar COMMIT não inferir rollback nem reenviar.
            _ => Err(PersistenceError::new(
                PersistenceErrorKind::OutcomeUnknown,
                self.context.operation_id,
            )),
        }
    }
}
