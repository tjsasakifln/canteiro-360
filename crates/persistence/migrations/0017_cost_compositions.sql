-- Issue 17: composições são versões de cálculo com políticas e insumos identificados.
CREATE TABLE canteiro.cost_compositions (
    tenant_id UUID NOT NULL,
    scope_id UUID NOT NULL,
    code TEXT NOT NULL CHECK(char_length(btrim(code)) BETWEEN 1 AND 80),
    current_version BIGINT NOT NULL CHECK(current_version>0),
    PRIMARY KEY(tenant_id,scope_id,code),
    FOREIGN KEY(tenant_id,scope_id) REFERENCES canteiro.access_scopes(tenant_id,id)
);
CREATE TABLE canteiro.cost_composition_versions (
    tenant_id UUID NOT NULL,
    scope_id UUID NOT NULL,
    code TEXT NOT NULL,
    version BIGINT NOT NULL CHECK(version>0),
    payload JSONB NOT NULL CHECK(jsonb_typeof(payload)='object' AND octet_length(payload::text)<=2000000),
    policy TEXT NOT NULL CHECK(policy='C360-DECIMAL-1'),
    actor_id UUID NOT NULL,
    operation_key UUID NOT NULL,
    fingerprint BYTEA NOT NULL CHECK(octet_length(fingerprint)=32),
    reason TEXT NOT NULL CHECK(char_length(btrim(reason)) BETWEEN 2 AND 500),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY(tenant_id,scope_id,code,version),
    UNIQUE(tenant_id,operation_key),
    FOREIGN KEY(tenant_id,scope_id,code) REFERENCES canteiro.cost_compositions(tenant_id,scope_id,code),
    FOREIGN KEY(tenant_id,actor_id) REFERENCES canteiro.organization_members(tenant_id,user_id)
);
ALTER TABLE canteiro.cost_compositions
    ADD CONSTRAINT composition_current_version
    FOREIGN KEY(tenant_id,scope_id,code,current_version)
    REFERENCES canteiro.cost_composition_versions(tenant_id,scope_id,code,version)
    DEFERRABLE INITIALLY DEFERRED;
CREATE TABLE canteiro.cost_composition_catalog_bindings (
    tenant_id UUID NOT NULL,
    scope_id UUID NOT NULL,
    composition_code TEXT NOT NULL,
    composition_version BIGINT NOT NULL,
    catalog_id TEXT NOT NULL,
    catalog_edition INTEGER NOT NULL,
    PRIMARY KEY(tenant_id,scope_id,composition_code,composition_version,catalog_id,catalog_edition),
    FOREIGN KEY(tenant_id,scope_id,composition_code,composition_version)
        REFERENCES canteiro.cost_composition_versions(tenant_id,scope_id,code,version),
    FOREIGN KEY(tenant_id,catalog_id,catalog_edition)
        REFERENCES canteiro.cost_catalog_editions(tenant_id,catalog_id,edition)
);
DO $$ DECLARE item TEXT; BEGIN
    FOREACH item IN ARRAY ARRAY[
      'cost_compositions','cost_composition_versions','cost_composition_catalog_bindings'
    ] LOOP
      EXECUTE format('ALTER TABLE canteiro.%I ENABLE ROW LEVEL SECURITY',item);
      EXECUTE format('ALTER TABLE canteiro.%I FORCE ROW LEVEL SECURITY',item);
      EXECUTE format('CREATE POLICY tenant_isolation ON canteiro.%I USING (tenant_id=NULLIF(current_setting(''canteiro.tenant_id'',true),'''')::UUID) WITH CHECK (tenant_id=NULLIF(current_setting(''canteiro.tenant_id'',true),'''')::UUID)',item);
      EXECUTE format('REVOKE ALL ON canteiro.%I FROM PUBLIC',item);
    END LOOP;
END $$;
CREATE TRIGGER immutable_composition_versions
    BEFORE UPDATE OR DELETE OR TRUNCATE ON canteiro.cost_composition_versions
    FOR EACH STATEMENT EXECUTE FUNCTION canteiro.reject_history_rewrite();
CREATE TRIGGER immutable_composition_catalog_bindings
    BEFORE UPDATE OR DELETE OR TRUNCATE ON canteiro.cost_composition_catalog_bindings
    FOR EACH STATEMENT EXECUTE FUNCTION canteiro.reject_history_rewrite();
