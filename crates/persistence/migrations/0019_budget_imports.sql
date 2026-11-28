-- Issue 19: um lote inteiro é validado antes de qualquer alteração no orçamento.
-- A chave do lote associa prévia, revisão de origem, ator e sessão; não é autorização.
CREATE TABLE canteiro.budget_import_batches (
    tenant_id UUID NOT NULL,
    id UUID NOT NULL,
    scope_id UUID NOT NULL,
    actor_id UUID NOT NULL,
    session_id UUID NOT NULL,
    base_version BIGINT NOT NULL CHECK(base_version > 0),
    format TEXT NOT NULL CHECK(format IN ('CANTEIRO-ORCAMENTO-CSV-1','CANTEIRO-ORCAMENTO-XLSX-1')),
    locale TEXT NOT NULL CHECK(locale IN ('canonical','pt-BR')),
    sha256 BYTEA NOT NULL CHECK(octet_length(sha256)=32),
    preview JSONB NOT NULL
        CHECK(jsonb_typeof(preview)='object' AND octet_length(preview::text)<=80000000),
    included_count INTEGER NOT NULL CHECK(included_count>=0),
    changed_count INTEGER NOT NULL CHECK(changed_count>=0),
    removed_count INTEGER NOT NULL CHECK(removed_count>=0),
    status TEXT NOT NULL DEFAULT 'ready' CHECK(status IN ('ready','applied','cancelled')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    expires_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()+interval '2 hours',
    applied_at TIMESTAMPTZ,
    PRIMARY KEY(tenant_id,id),
    UNIQUE(tenant_id,scope_id,actor_id,session_id,base_version,sha256),
    FOREIGN KEY(tenant_id,scope_id,base_version)
        REFERENCES canteiro.budget_versions(tenant_id,scope_id,version),
    FOREIGN KEY(tenant_id,actor_id) REFERENCES canteiro.organization_members(tenant_id,user_id),
    FOREIGN KEY(session_id) REFERENCES canteiro_auth.sessions(id),
    CHECK((status='applied')=(applied_at IS NOT NULL)),
    CHECK(expires_at>created_at)
);
CREATE FUNCTION canteiro.budget_import_guard() RETURNS trigger
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
        OR NEW.expires_at IS DISTINCT FROM OLD.expires_at
        OR NEW.status NOT IN ('applied','cancelled') THEN
        RAISE EXCEPTION USING ERRCODE='23514',MESSAGE='O lote de importação não admite reinterpretação ou reaplicação.';
    END IF;
    IF NEW.status='applied' AND (OLD.expires_at<=clock_timestamp() OR NEW.applied_at IS NULL) THEN
        RAISE EXCEPTION USING ERRCODE='23514',MESSAGE='Revalide a importação antes de aplicar.';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER budget_import_guard BEFORE UPDATE ON canteiro.budget_import_batches
    FOR EACH ROW EXECUTE FUNCTION canteiro.budget_import_guard();
ALTER TABLE canteiro.budget_import_batches ENABLE ROW LEVEL SECURITY;
ALTER TABLE canteiro.budget_import_batches FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON canteiro.budget_import_batches
    USING(tenant_id=NULLIF(current_setting('canteiro.tenant_id',true),'')::UUID)
    WITH CHECK(tenant_id=NULLIF(current_setting('canteiro.tenant_id',true),'')::UUID);
CREATE POLICY own_budget_import ON canteiro.budget_import_batches AS RESTRICTIVE
    USING(actor_id=NULLIF(current_setting('canteiro.actor_id',true),'')::UUID)
    WITH CHECK(actor_id=NULLIF(current_setting('canteiro.actor_id',true),'')::UUID);
REVOKE ALL ON canteiro.budget_import_batches FROM PUBLIC;
