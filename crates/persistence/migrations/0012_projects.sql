-- Projeto possui escopo próprio; não obriga criar obra ou contrato fictício.
ALTER TABLE canteiro.access_scopes DROP CONSTRAINT access_scopes_kind_check;
ALTER TABLE canteiro.access_scopes ADD CONSTRAINT access_scopes_kind_check CHECK(kind IN ('work','contract','budget','project'));
ALTER TABLE canteiro.access_scopes DROP CONSTRAINT access_scopes_check;
ALTER TABLE canteiro.access_scopes ADD CONSTRAINT access_scopes_check CHECK((kind IN ('work','project') AND parent_id IS NULL) OR (kind IN ('contract','budget') AND parent_id IS NOT NULL AND parent_id<>id));

CREATE TABLE canteiro.projects (
 tenant_id UUID NOT NULL, id UUID NOT NULL,
 current_version BIGINT NOT NULL CHECK(current_version>0),
 PRIMARY KEY(tenant_id,id), FOREIGN KEY(tenant_id,id) REFERENCES canteiro.access_scopes(tenant_id,id)
);
CREATE TABLE canteiro.project_versions (
 tenant_id UUID NOT NULL, project_id UUID NOT NULL, version BIGINT NOT NULL CHECK(version>0),
 payload TEXT NOT NULL CHECK(octet_length(payload)<=500000 AND jsonb_typeof(payload::jsonb)='object'),
 actor_id UUID NOT NULL, operation_key UUID NOT NULL,
 fingerprint BYTEA NOT NULL CHECK(octet_length(fingerprint)=32),
 reason TEXT NOT NULL CHECK(char_length(btrim(reason)) BETWEEN 2 AND 500),
 recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(tenant_id,project_id,version), UNIQUE(tenant_id,operation_key),
 FOREIGN KEY(tenant_id,project_id) REFERENCES canteiro.projects(tenant_id,id),
 FOREIGN KEY(tenant_id,actor_id) REFERENCES canteiro.organization_members(tenant_id,user_id)
);
ALTER TABLE canteiro.projects ADD CONSTRAINT project_current_version FOREIGN KEY(tenant_id,id,current_version) REFERENCES canteiro.project_versions(tenant_id,project_id,version) DEFERRABLE INITIALLY DEFERRED;
CREATE TABLE canteiro.project_party_links (
 tenant_id UUID NOT NULL,project_id UUID NOT NULL,version BIGINT NOT NULL,party_id UUID NOT NULL,
 PRIMARY KEY(tenant_id,project_id,version,party_id),
 FOREIGN KEY(tenant_id,project_id,version) REFERENCES canteiro.project_versions(tenant_id,project_id,version),
 FOREIGN KEY(tenant_id,party_id) REFERENCES canteiro.parties(tenant_id,id)
);
CREATE TABLE canteiro.project_work_links (
 tenant_id UUID NOT NULL,project_id UUID NOT NULL,version BIGINT NOT NULL,work_id UUID NOT NULL,
 PRIMARY KEY(tenant_id,project_id,version),
 FOREIGN KEY(tenant_id,project_id,version) REFERENCES canteiro.project_versions(tenant_id,project_id,version),
 FOREIGN KEY(tenant_id,work_id) REFERENCES canteiro.portfolio_works(tenant_id,id)
);
DO $$ DECLARE t TEXT; BEGIN
 FOREACH t IN ARRAY ARRAY['projects','project_versions','project_party_links','project_work_links'] LOOP
  EXECUTE format('ALTER TABLE canteiro.%I ENABLE ROW LEVEL SECURITY',t);
  EXECUTE format('ALTER TABLE canteiro.%I FORCE ROW LEVEL SECURITY',t);
  EXECUTE format('CREATE POLICY tenant_isolation ON canteiro.%I USING (tenant_id=NULLIF(current_setting(''canteiro.tenant_id'',true),'''')::UUID) WITH CHECK (tenant_id=NULLIF(current_setting(''canteiro.tenant_id'',true),'''')::UUID)',t);
  EXECUTE format('REVOKE ALL ON canteiro.%I FROM PUBLIC',t);
 END LOOP;
 FOREACH t IN ARRAY ARRAY['project_versions','project_party_links','project_work_links'] LOOP
  EXECUTE format('CREATE TRIGGER immutable_project_history BEFORE UPDATE OR DELETE OR TRUNCATE ON canteiro.%I FOR EACH STATEMENT EXECUTE FUNCTION canteiro.reject_history_rewrite()',t);
 END LOOP;
END $$;
CREATE POLICY project_author ON canteiro.project_versions AS RESTRICTIVE WITH CHECK(actor_id=NULLIF(current_setting('canteiro.actor_id',true),'')::UUID);
