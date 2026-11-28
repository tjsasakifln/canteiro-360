-- Issue #19: impedir recibos fictícios por INSERT ou UPDATE direto.
-- A revisão vigente deve ser exatamente a prévia e usar a identidade do lote.
CREATE OR REPLACE FUNCTION canteiro.budget_import_guard() RETURNS trigger
LANGUAGE plpgsql SET search_path=pg_catalog AS $$
BEGIN
    IF TG_OP='INSERT' THEN
        IF NEW.status <> 'ready' OR NEW.applied_at IS NOT NULL THEN
            RAISE EXCEPTION USING ERRCODE='23514',
                MESSAGE='O lote deve iniciar como prévia, sem recibo de aplicação.';
        END IF;
        RETURN NEW;
    END IF;
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
        AND (OLD.expires_at<=clock_timestamp() OR NEW.applied_at IS NULL
            OR NOT EXISTS (
                SELECT 1 FROM canteiro.budget_versions v
                JOIN canteiro.budget_records r
                  ON r.tenant_id=v.tenant_id AND r.scope_id=v.scope_id
                 AND r.current_version=v.version
                WHERE v.tenant_id=OLD.tenant_id AND v.scope_id=OLD.scope_id
                  AND v.version=COALESCE(OLD.base_version,0)+1
                  AND v.actor_id=OLD.actor_id AND v.operation_key=OLD.id
                  AND v.payload=OLD.preview AND v.state='draft'
            )) THEN
        RAISE EXCEPTION USING ERRCODE='23514',
            MESSAGE='Revalide a importação antes de aplicar.';
    END IF;
    IF NEW.status='applied' THEN
        NEW.applied_at := clock_timestamp();
    END IF;
    RETURN NEW;
END $$;
DROP TRIGGER budget_import_guard ON canteiro.budget_import_batches;
CREATE TRIGGER budget_import_guard BEFORE INSERT OR UPDATE ON canteiro.budget_import_batches
    FOR EACH ROW EXECUTE FUNCTION canteiro.budget_import_guard();
