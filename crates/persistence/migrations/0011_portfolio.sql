-- Uma referência de acesso a obra origina no máximo uma linha na carteira.
CREATE TABLE canteiro.portfolio_works (
 tenant_id UUID NOT NULL,id UUID NOT NULL,
 title TEXT NOT NULL CHECK(char_length(btrim(title)) BETWEEN 2 AND 160),
 status TEXT NOT NULL CHECK(status IN ('planned','active','paused','completed')),
 data TEXT NOT NULL CHECK(octet_length(data)<=100000),
 version BIGINT NOT NULL DEFAULT 1 CHECK(version>0),
 created_by UUID NOT NULL REFERENCES canteiro_auth.users(id),created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(tenant_id,id),FOREIGN KEY(tenant_id,id) REFERENCES canteiro.access_scopes(tenant_id,id)
);
CREATE TABLE canteiro.portfolio_contract_links (
 tenant_id UUID NOT NULL,work_id UUID NOT NULL,contract_id UUID NOT NULL,
 PRIMARY KEY(tenant_id,work_id,contract_id),FOREIGN KEY(tenant_id,work_id) REFERENCES canteiro.portfolio_works(tenant_id,id),
 FOREIGN KEY(tenant_id,contract_id) REFERENCES canteiro.access_scopes(tenant_id,id)
);
CREATE TABLE canteiro.portfolio_history (
 tenant_id UUID NOT NULL,work_id UUID NOT NULL,version BIGINT NOT NULL CHECK(version>0),
 before_data TEXT,after_data TEXT NOT NULL,actor_id UUID NOT NULL REFERENCES canteiro_auth.users(id),
 reason TEXT NOT NULL CHECK(char_length(btrim(reason)) BETWEEN 2 AND 200),recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(tenant_id,work_id,version),FOREIGN KEY(tenant_id,work_id) REFERENCES canteiro.portfolio_works(tenant_id,id)
);
CREATE FUNCTION canteiro.require_portfolio_scope() RETURNS trigger LANGUAGE plpgsql SET search_path=pg_catalog AS $$
BEGIN
 IF TG_TABLE_NAME='portfolio_works' THEN
  IF NOT EXISTS(SELECT 1 FROM canteiro.access_scopes s WHERE s.tenant_id=NEW.tenant_id AND s.id=NEW.id AND s.kind='work') THEN
   RAISE EXCEPTION USING ERRCODE='23514',MESSAGE='Selecione uma referência de obra da mesma empresa.';
  END IF;
 ELSIF TG_TABLE_NAME='portfolio_contract_links' THEN
  IF NOT EXISTS(SELECT 1 FROM canteiro.access_scopes s WHERE s.tenant_id=NEW.tenant_id AND s.id=NEW.contract_id AND s.kind='contract') THEN
   RAISE EXCEPTION USING ERRCODE='23514',MESSAGE='Selecione uma referência de contrato da mesma empresa.';
  END IF;
 END IF;
 RETURN NEW;
END $$;
REVOKE ALL ON FUNCTION canteiro.require_portfolio_scope() FROM PUBLIC;
CREATE TRIGGER portfolio_scope BEFORE INSERT OR UPDATE ON canteiro.portfolio_works FOR EACH ROW EXECUTE FUNCTION canteiro.require_portfolio_scope();
CREATE TRIGGER portfolio_link_scope BEFORE INSERT OR UPDATE ON canteiro.portfolio_contract_links FOR EACH ROW EXECUTE FUNCTION canteiro.require_portfolio_scope();
CREATE TRIGGER portfolio_history_immutable BEFORE UPDATE OR DELETE OR TRUNCATE ON canteiro.portfolio_history FOR EACH STATEMENT EXECUTE FUNCTION canteiro.reject_history_rewrite();
DO $$ DECLARE table_name TEXT; BEGIN
 FOREACH table_name IN ARRAY ARRAY['portfolio_works','portfolio_contract_links','portfolio_history'] LOOP
  EXECUTE format('ALTER TABLE canteiro.%I ENABLE ROW LEVEL SECURITY',table_name);
  EXECUTE format('ALTER TABLE canteiro.%I FORCE ROW LEVEL SECURITY',table_name);
  EXECUTE format('CREATE POLICY tenant_isolation ON canteiro.%I USING (tenant_id=NULLIF(current_setting(''canteiro.tenant_id'',true),'''')::UUID) WITH CHECK (tenant_id=NULLIF(current_setting(''canteiro.tenant_id'',true),'''')::UUID)',table_name);
  EXECUTE format('REVOKE ALL ON canteiro.%I FROM PUBLIC',table_name);
 END LOOP;
END $$;
