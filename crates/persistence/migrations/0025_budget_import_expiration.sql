-- Issue #19: uma prévia expirada não impede nova conferência do mesmo arquivo.
-- Lotes cancelados permanecem no histórico; lotes aplicados conservam a unicidade.
-- A restrição UNIQUE original teve o nome gerado pelo PostgreSQL.
DO $$
DECLARE old_constraint NAME;
BEGIN
    SELECT conname INTO STRICT old_constraint
      FROM pg_constraint
     WHERE conrelid = 'canteiro.budget_import_batches'::regclass AND contype = 'u';
    EXECUTE format('ALTER TABLE canteiro.budget_import_batches DROP CONSTRAINT %I', old_constraint);
END $$;
DROP INDEX canteiro.budget_import_first_preview_unique;
CREATE UNIQUE INDEX budget_import_active_preview_unique
    ON canteiro.budget_import_batches
       (tenant_id,scope_id,actor_id,session_id,base_version,sha256)
    NULLS NOT DISTINCT
    WHERE status <> 'cancelled';
