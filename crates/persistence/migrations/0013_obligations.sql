CREATE TABLE canteiro.technical_obligations (
 tenant_id UUID NOT NULL,id UUID NOT NULL,scope_id UUID NOT NULL,current_version BIGINT NOT NULL CHECK(current_version>0),
 PRIMARY KEY(tenant_id,id),UNIQUE(tenant_id,id,scope_id),
 FOREIGN KEY(tenant_id,scope_id) REFERENCES canteiro.access_scopes(tenant_id,id)
);
CREATE TABLE canteiro.obligation_versions (
 tenant_id UUID NOT NULL,obligation_id UUID NOT NULL,scope_id UUID NOT NULL,version BIGINT NOT NULL CHECK(version>0),
 payload TEXT NOT NULL CHECK(octet_length(payload)<=20000 AND jsonb_typeof(payload::jsonb)='object'),confirmed BOOLEAN NOT NULL,
 professional_id UUID, professional_name TEXT, document_id UUID, document_version BIGINT,document_sha256 TEXT,
 replaces_id UUID, actor_id UUID NOT NULL,operation_key UUID NOT NULL,fingerprint BYTEA NOT NULL CHECK(octet_length(fingerprint)=32),
 reason TEXT NOT NULL CHECK(char_length(btrim(reason)) BETWEEN 2 AND 500),recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(tenant_id,obligation_id,version),UNIQUE(tenant_id,operation_key),
 FOREIGN KEY(tenant_id,obligation_id,scope_id) REFERENCES canteiro.technical_obligations(tenant_id,id,scope_id),
 FOREIGN KEY(tenant_id,professional_id) REFERENCES canteiro.parties(tenant_id,id),
 FOREIGN KEY(tenant_id,document_id,document_version,document_sha256) REFERENCES canteiro.document_versions(tenant_id,document_id,version,sha256),
 FOREIGN KEY(tenant_id,document_id,scope_id) REFERENCES canteiro.documents(tenant_id,id,scope_id),
 FOREIGN KEY(tenant_id,replaces_id,scope_id) REFERENCES canteiro.technical_obligations(tenant_id,id,scope_id),
 FOREIGN KEY(tenant_id,actor_id) REFERENCES canteiro.organization_members(tenant_id,user_id),
 CHECK((document_id IS NULL AND document_version IS NULL AND document_sha256 IS NULL) OR (document_id IS NOT NULL AND document_version>0 AND document_sha256 IS NOT NULL)),
 CHECK(NOT confirmed OR document_id IS NOT NULL),CHECK(replaces_id IS NULL OR replaces_id<>obligation_id)
);
ALTER TABLE canteiro.technical_obligations ADD CONSTRAINT obligation_current_version FOREIGN KEY(tenant_id,id,current_version) REFERENCES canteiro.obligation_versions(tenant_id,obligation_id,version) DEFERRABLE INITIALLY DEFERRED;
-- A substituição não atualiza os dados ou a vigência do documento anterior.
CREATE TABLE canteiro.obligation_replacements (
 tenant_id UUID NOT NULL,previous_id UUID NOT NULL,new_id UUID NOT NULL,new_version BIGINT NOT NULL,
 PRIMARY KEY(tenant_id,previous_id),UNIQUE(tenant_id,new_id),CHECK(previous_id<>new_id),
 FOREIGN KEY(tenant_id,previous_id) REFERENCES canteiro.technical_obligations(tenant_id,id),
 FOREIGN KEY(tenant_id,new_id,new_version) REFERENCES canteiro.obligation_versions(tenant_id,obligation_id,version)
);
DO $$ DECLARE t TEXT; BEGIN
 FOREACH t IN ARRAY ARRAY['technical_obligations','obligation_versions','obligation_replacements'] LOOP
  EXECUTE format('ALTER TABLE canteiro.%I ENABLE ROW LEVEL SECURITY',t);
  EXECUTE format('ALTER TABLE canteiro.%I FORCE ROW LEVEL SECURITY',t);
  EXECUTE format('CREATE POLICY tenant_isolation ON canteiro.%I USING (tenant_id=NULLIF(current_setting(''canteiro.tenant_id'',true),'''')::UUID) WITH CHECK (tenant_id=NULLIF(current_setting(''canteiro.tenant_id'',true),'''')::UUID)',t);
  EXECUTE format('REVOKE ALL ON canteiro.%I FROM PUBLIC',t);
 END LOOP;
 FOREACH t IN ARRAY ARRAY['obligation_versions','obligation_replacements'] LOOP
  EXECUTE format('CREATE TRIGGER immutable_obligation_history BEFORE UPDATE OR DELETE OR TRUNCATE ON canteiro.%I FOR EACH STATEMENT EXECUTE FUNCTION canteiro.reject_history_rewrite()',t);
 END LOOP;
END $$;
CREATE POLICY obligation_author ON canteiro.obligation_versions AS RESTRICTIVE WITH CHECK(actor_id=NULLIF(current_setting('canteiro.actor_id',true),'')::UUID);
