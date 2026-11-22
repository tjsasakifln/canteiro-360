-- Issue #19: o lote identifica uma prévia imutável, não apenas seu conteúdo.
-- A revisão e o histórico de auditoria precisam ser preservados na mesma transação.
-- A função anterior não verificava o campo created_at em um UPDATE.
CREATE OR REPLACE FUNCTION canteiro.budget_import_guard() RETURNS trigger
LANGUAGE plpgsql SET search_path=pg_catalog AS $$
BEGIN
    IF OLD.status <> 'ready'
        OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
        OR NEW.id IS DISTINCT FROM OLD.id
        OR NEW.scope_id IS DISTINCT FROM OLD.scope_id
        OR NEW.actor_id IS DISTINCT FROM OLD.actor_id
        OR NEW.session_id IS DISTINCT FROM OLD.session_id
        OR NEW.base_version IS DISTINCT FROM OLD.base_version
        OR NEW.format IS DISTINCT FROM OLD.format
        OR NEW.locale IS DISTINCT FROM OLD.locale
        OR NEW.sha256 IS DISTINCT FROM OLD.sha256
        OR NEW.preview IS DISTINCT FROM OLD.preview
        OR NEW.included_count IS DISTINCT FROM OLD.included_count
        OR NEW.changed_count IS DISTINCT FROM OLD.changed_count
        OR NEW.removed_count IS DISTINCT FROM OLD.removed_count
        OR NEW.created_at IS DISTINCT FROM OLD.created_at
        OR NEW.expires_at IS DISTINCT FROM OLD.expires_at
        OR NEW.status NOT IN ('applied','cancelled') THEN
        RAISE EXCEPTION USING ERRCODE='23514',
            MESSAGE='O lote de importação não admite reinterpretação ou reaplicação.';
    END IF;
    IF NEW.status='applied'
        AND (OLD.expires_at<=clock_timestamp() OR NEW.applied_at IS NULL) THEN
        RAISE EXCEPTION USING ERRCODE='23514',
            MESSAGE='Revalide a importação antes de aplicar.';
    END IF;
    RETURN NEW;
END $$;
