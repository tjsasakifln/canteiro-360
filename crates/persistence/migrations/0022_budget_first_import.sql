-- Issue #19: a primeira importação também exige prévia durável e confirmação atômica.
-- Ausência de revisão-base significa exclusivamente criação da revisão 1.
-- A FK para orçamento histórico permanece obrigatória quando base_version existe.
ALTER TABLE canteiro.budget_import_batches
    ALTER COLUMN base_version DROP NOT NULL;

-- UNIQUE comum distingue NULLs: exigir idempotência também no primeiro lote,
-- sem alterar a unicidade das reimportações com versão-base existente.
CREATE UNIQUE INDEX budget_import_first_preview_unique
    ON canteiro.budget_import_batches
       (tenant_id,scope_id,actor_id,session_id,sha256)
    WHERE base_version IS NULL;
