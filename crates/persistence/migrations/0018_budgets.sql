-- Issue 18: revisão orçamentária integral, imutável, com controle de concorrência.
-- O escopo "budget" preexiste e possui contrato-pai; a seleção de obra não dá direito automático.
CREATE TABLE canteiro.budget_records (
    tenant_id UUID NOT NULL,
    scope_id UUID NOT NULL,
    current_version BIGINT NOT NULL CHECK(current_version>0),
    PRIMARY KEY(tenant_id,scope_id),
    FOREIGN KEY(tenant_id,scope_id) REFERENCES canteiro.access_scopes(tenant_id,id)
);
CREATE TABLE canteiro.budget_versions (
    tenant_id UUID NOT NULL,
    scope_id UUID NOT NULL,
    version BIGINT NOT NULL CHECK(version>0),
    payload JSONB NOT NULL
        CHECK(jsonb_typeof(payload)='object' AND octet_length(payload::text)<=80000000),
    state TEXT NOT NULL CHECK(state IN ('draft','review','approved')),
    policy TEXT NOT NULL CHECK(policy='C360-DECIMAL-1'),
    actor_id UUID NOT NULL,
    operation_key UUID NOT NULL,
    fingerprint BYTEA NOT NULL CHECK(octet_length(fingerprint)=32),
    reason TEXT NOT NULL CHECK(char_length(btrim(reason)) BETWEEN 2 AND 500),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY(tenant_id,scope_id,version),
    UNIQUE(tenant_id,operation_key),
    FOREIGN KEY(tenant_id,scope_id) REFERENCES canteiro.budget_records(tenant_id,scope_id),
    FOREIGN KEY(tenant_id,actor_id) REFERENCES canteiro.organization_members(tenant_id,user_id)
);
ALTER TABLE canteiro.budget_records ADD CONSTRAINT budget_current_version
    FOREIGN KEY(tenant_id,scope_id,current_version)
    REFERENCES canteiro.budget_versions(tenant_id,scope_id,version)
    DEFERRABLE INITIALLY DEFERRED;
CREATE INDEX budget_history_by_scope
    ON canteiro.budget_versions(tenant_id,scope_id,version DESC);
CREATE TABLE canteiro.budget_edit_leases (
    tenant_id UUID NOT NULL,
    scope_id UUID NOT NULL,
    actor_id UUID NOT NULL,
    session_id UUID NOT NULL,
    lease_id UUID NOT NULL,
    version BIGINT NOT NULL CHECK(version>0),
    expires_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY(tenant_id,scope_id),
    FOREIGN KEY(tenant_id,scope_id) REFERENCES canteiro.access_scopes(tenant_id,id),
    FOREIGN KEY(tenant_id,actor_id) REFERENCES canteiro.organization_members(tenant_id,user_id),
    FOREIGN KEY(session_id) REFERENCES canteiro_auth.sessions(id)
);
CREATE FUNCTION canteiro.budget_revision_guard() RETURNS trigger
LANGUAGE plpgsql SET search_path=pg_catalog AS $$
DECLARE before_state TEXT;
BEGIN
    IF NEW.version>1 THEN
      SELECT previous.state INTO before_state FROM canteiro.budget_versions previous
      WHERE previous.tenant_id=NEW.tenant_id AND previous.scope_id=NEW.scope_id
        AND previous.version=NEW.version-1;
      IF before_state IS NULL THEN
        RAISE EXCEPTION USING ERRCODE='23514',MESSAGE='A revisão anterior do orçamento não existe.';
      END IF;
      IF before_state='approved' THEN
        RAISE EXCEPTION USING ERRCODE='23514',MESSAGE='Crie nova alteração formal para revisão definitiva.';
      END IF;
    END IF;
    IF NEW.state='approved' AND NEW.version=1 THEN
        RAISE EXCEPTION USING ERRCODE='23514',MESSAGE='Conferência separada é necessária antes da aprovação.';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER budget_revision_guard BEFORE INSERT ON canteiro.budget_versions
    FOR EACH ROW EXECUTE FUNCTION canteiro.budget_revision_guard();
CREATE TRIGGER immutable_budget_versions BEFORE UPDATE OR DELETE OR TRUNCATE
    ON canteiro.budget_versions
    FOR EACH STATEMENT EXECUTE FUNCTION canteiro.reject_history_rewrite();
DO $$ DECLARE item TEXT; BEGIN
    FOREACH item IN ARRAY ARRAY[
        'budget_records','budget_versions','budget_edit_leases'
    ] LOOP
      EXECUTE format('ALTER TABLE canteiro.%I ENABLE ROW LEVEL SECURITY',item);
      EXECUTE format('ALTER TABLE canteiro.%I FORCE ROW LEVEL SECURITY',item);
      EXECUTE format('CREATE POLICY tenant_isolation ON canteiro.%I USING (tenant_id=NULLIF(current_setting(''canteiro.tenant_id'',true),'''')::UUID) WITH CHECK (tenant_id=NULLIF(current_setting(''canteiro.tenant_id'',true),'''')::UUID)',item);
      EXECUTE format('REVOKE ALL ON canteiro.%I FROM PUBLIC',item);
    END LOOP;
END $$;
