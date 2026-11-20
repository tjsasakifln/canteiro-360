-- Pessoas e empresas não concedem contas ou permissões de acesso.
CREATE TABLE canteiro.parties (
 tenant_id UUID NOT NULL REFERENCES canteiro.organizations(tenant_id), id UUID NOT NULL,
 kind TEXT NOT NULL CHECK(kind IN ('person','company')), tax_id TEXT,
 name TEXT NOT NULL CHECK(char_length(btrim(name)) BETWEEN 2 AND 160),
 customer BOOLEAN NOT NULL, supplier BOOLEAN NOT NULL, professional BOOLEAN NOT NULL,
 active BOOLEAN NOT NULL, council TEXT NOT NULL DEFAULT '', registration TEXT NOT NULL DEFAULT '', uf TEXT NOT NULL DEFAULT '',
 valid_from DATE, valid_until DATE, data TEXT NOT NULL CHECK(octet_length(data)<=30000),
 version BIGINT NOT NULL DEFAULT 1 CHECK(version>0),
 created_by UUID NOT NULL REFERENCES canteiro_auth.users(id), created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(tenant_id,id), UNIQUE(tenant_id,tax_id),
 CHECK(customer OR supplier OR professional), CHECK(NOT professional OR kind='person'),
 CHECK(tax_id IS NULL OR (kind='person' AND tax_id~'^[0-9]{11}$') OR (kind='company' AND tax_id~'^[A-Z0-9]{12}[0-9]{2}$')),
 CHECK(valid_until IS NULL OR (valid_from IS NOT NULL AND valid_until>=valid_from))
);
CREATE INDEX parties_name ON canteiro.parties(tenant_id,lower(name),id);
CREATE TABLE canteiro.party_history (
 tenant_id UUID NOT NULL, party_id UUID NOT NULL, version BIGINT NOT NULL CHECK(version>0),
 before_data TEXT, after_data TEXT NOT NULL, actor_id UUID NOT NULL REFERENCES canteiro_auth.users(id),
 reason TEXT NOT NULL CHECK(char_length(btrim(reason)) BETWEEN 2 AND 200), recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(tenant_id,party_id,version), FOREIGN KEY(tenant_id,party_id) REFERENCES canteiro.parties(tenant_id,id)
);
CREATE TABLE canteiro.party_bank_versions (
 tenant_id UUID NOT NULL, party_id UUID NOT NULL, version BIGINT NOT NULL CHECK(version>0),
 account TEXT NOT NULL CHECK(octet_length(account)<=3000), actor_id UUID NOT NULL REFERENCES canteiro_auth.users(id),
 reason TEXT NOT NULL CHECK(char_length(btrim(reason)) BETWEEN 2 AND 200), recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(tenant_id,party_id,version), FOREIGN KEY(tenant_id,party_id) REFERENCES canteiro.parties(tenant_id,id)
);
CREATE TABLE canteiro.registry_operations (
 tenant_id UUID NOT NULL, act TEXT NOT NULL CHECK(act IN ('party_save','party_import','party_bank','portfolio_save')),
 operation_key UUID NOT NULL, actor_id UUID NOT NULL, fingerprint BYTEA NOT NULL CHECK(octet_length(fingerprint)=32),
 result TEXT NOT NULL CHECK(octet_length(result)<=30000), recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(tenant_id,act,operation_key), FOREIGN KEY(tenant_id,actor_id) REFERENCES canteiro.organization_members(tenant_id,user_id)
);
DO $$ DECLARE table_name TEXT; BEGIN
 FOREACH table_name IN ARRAY ARRAY['parties','party_history','party_bank_versions','registry_operations'] LOOP
  EXECUTE format('ALTER TABLE canteiro.%I ENABLE ROW LEVEL SECURITY',table_name);
  EXECUTE format('ALTER TABLE canteiro.%I FORCE ROW LEVEL SECURITY',table_name);
  EXECUTE format('CREATE POLICY tenant_isolation ON canteiro.%I USING (tenant_id=NULLIF(current_setting(''canteiro.tenant_id'',true),'''')::UUID) WITH CHECK (tenant_id=NULLIF(current_setting(''canteiro.tenant_id'',true),'''')::UUID)',table_name);
  EXECUTE format('REVOKE ALL ON canteiro.%I FROM PUBLIC',table_name);
 END LOOP;
 FOREACH table_name IN ARRAY ARRAY['party_history','party_bank_versions','registry_operations'] LOOP
  EXECUTE format('CREATE TRIGGER immutable_registry BEFORE UPDATE OR DELETE OR TRUNCATE ON canteiro.%I FOR EACH STATEMENT EXECUTE FUNCTION canteiro.reject_history_rewrite()',table_name);
 END LOOP;
END $$;
-- Mesmo uma consulta SQL com empresa válida não revela conta bancária a membro comum.
CREATE POLICY bank_administrator ON canteiro.party_bank_versions AS RESTRICTIVE
 USING (EXISTS(SELECT 1 FROM canteiro.organization_members m WHERE m.tenant_id=party_bank_versions.tenant_id AND m.user_id=NULLIF(current_setting('canteiro.actor_id',true),'')::UUID AND m.role='administrator' AND m.active AND (m.valid_until IS NULL OR m.valid_until>clock_timestamp())))
 WITH CHECK (actor_id=NULLIF(current_setting('canteiro.actor_id',true),'')::UUID AND EXISTS(SELECT 1 FROM canteiro.organization_members m WHERE m.tenant_id=party_bank_versions.tenant_id AND m.user_id=party_bank_versions.actor_id AND m.role='administrator' AND m.active AND (m.valid_until IS NULL OR m.valid_until>clock_timestamp())));
CREATE POLICY receipt_actor ON canteiro.registry_operations AS RESTRICTIVE
 USING(actor_id=NULLIF(current_setting('canteiro.actor_id',true),'')::UUID)
 WITH CHECK(actor_id=NULLIF(current_setting('canteiro.actor_id',true),'')::UUID);
