-- Direitos explícitos por obra/contrato. Uma referência de acesso não formaliza um contrato.
CREATE TABLE canteiro.access_scopes (
    tenant_id UUID NOT NULL REFERENCES canteiro.organizations(tenant_id),
    id UUID NOT NULL, kind TEXT NOT NULL CHECK (kind IN ('work','contract','budget')),
    name TEXT NOT NULL CHECK (char_length(btrim(name)) BETWEEN 2 AND 160), parent_id UUID,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL REFERENCES canteiro_auth.users(id), created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (tenant_id,id), FOREIGN KEY (tenant_id,parent_id) REFERENCES canteiro.access_scopes(tenant_id,id),
    CHECK ((kind='work' AND parent_id IS NULL) OR (kind IN ('contract','budget') AND parent_id IS NOT NULL AND parent_id<>id))
);
CREATE TABLE canteiro.access_grants (
    tenant_id UUID NOT NULL, id UUID NOT NULL, scope_id UUID NOT NULL, user_id UUID NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('viewer','manager','budgeter','inspector','technical_responsible','approver','finance','supplier','client','support')),
    actions TEXT[] NOT NULL CHECK (cardinality(actions) BETWEEN 1 AND 11 AND actions <@ ARRAY['view','edit_budget','review_budget','record_execution','review_measurement','propose_change','approve_budget','approve_change','authorize_payment','record_payment','technical_accept']::TEXT[]),
    max_amount NUMERIC NOT NULL CHECK (max_amount>=0 AND max_amount<1000000000000000000 AND scale(max_amount)<=2),
    valid_from TIMESTAMPTZ NOT NULL, valid_until TIMESTAMPTZ, revoked_at TIMESTAMPTZ,
    source_grant_id UUID, substitution BOOLEAN NOT NULL DEFAULT false,
    reason TEXT NOT NULL DEFAULT '' CHECK(char_length(reason)<=200),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version>0),
    created_by UUID NOT NULL REFERENCES canteiro_auth.users(id), created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (tenant_id,id), FOREIGN KEY (tenant_id,scope_id) REFERENCES canteiro.access_scopes(tenant_id,id),
    FOREIGN KEY (tenant_id,user_id) REFERENCES canteiro.organization_members(tenant_id,user_id),
    FOREIGN KEY (tenant_id,source_grant_id) REFERENCES canteiro.access_grants(tenant_id,id),
    CHECK (valid_until IS NULL OR valid_until>valid_from), CHECK (source_grant_id IS NULL OR (source_grant_id<>id AND valid_until IS NOT NULL)),
    CHECK (source_grant_id IS NULL OR char_length(btrim(reason))>=2), CHECK (NOT substitution OR source_grant_id IS NOT NULL), CHECK (role<>'support' OR valid_until IS NOT NULL)
);
CREATE FUNCTION canteiro.require_work_parent() RETURNS trigger LANGUAGE plpgsql SET search_path=pg_catalog AS $$
BEGIN
    IF NEW.kind IN ('contract','budget') AND NOT EXISTS(SELECT 1 FROM canteiro.access_scopes s WHERE s.tenant_id=NEW.tenant_id AND s.id=NEW.parent_id AND s.kind=CASE NEW.kind WHEN 'contract' THEN 'work' ELSE 'contract' END) THEN
        RAISE EXCEPTION USING ERRCODE='23514', MESSAGE='O contrato exige uma obra; o orçamento exige um contrato da mesma empresa.';
    END IF;
    IF TG_OP='UPDATE' AND (NEW.tenant_id IS DISTINCT FROM OLD.tenant_id OR NEW.id IS DISTINCT FROM OLD.id OR NEW.kind IS DISTINCT FROM OLD.kind OR NEW.parent_id IS DISTINCT FROM OLD.parent_id) THEN
        RAISE EXCEPTION USING ERRCODE='23514', MESSAGE='A identidade e a relação da referência de acesso são permanentes.';
    END IF;
    RETURN NEW;
END $$;
REVOKE ALL ON FUNCTION canteiro.require_work_parent() FROM PUBLIC;
CREATE TRIGGER require_work_parent BEFORE INSERT OR UPDATE ON canteiro.access_scopes FOR EACH ROW EXECUTE FUNCTION canteiro.require_work_parent();
CREATE INDEX access_grants_person ON canteiro.access_grants(tenant_id,user_id,scope_id);
CREATE TABLE canteiro.access_requirements (
    tenant_id UUID NOT NULL, scope_id UUID NOT NULL, role TEXT NOT NULL,
    action TEXT NOT NULL, min_amount NUMERIC NOT NULL CHECK(min_amount>=0 AND min_amount<1000000000000000000 AND scale(min_amount)<=2),
    PRIMARY KEY (tenant_id,scope_id,role,action), FOREIGN KEY (tenant_id,scope_id) REFERENCES canteiro.access_scopes(tenant_id,id)
);
CREATE TABLE canteiro.access_policies (
    tenant_id UUID NOT NULL, scope_id UUID NOT NULL,
    action TEXT NOT NULL CHECK (action IN ('approve_budget','approve_change','authorize_payment','technical_accept')),
    threshold NUMERIC NOT NULL CHECK (threshold>=0 AND threshold<1000000000000000000 AND scale(threshold)<=2),
    approvals_above_threshold SMALLINT NOT NULL CHECK (approvals_above_threshold BETWEEN 1 AND 2),
    segregate_proposer BOOLEAN NOT NULL, version BIGINT NOT NULL DEFAULT 1 CHECK (version>0),
    PRIMARY KEY (tenant_id,scope_id,action), FOREIGN KEY (tenant_id,scope_id) REFERENCES canteiro.access_scopes(tenant_id,id)
);
CREATE TABLE canteiro.access_requests (
    tenant_id UUID NOT NULL, id UUID NOT NULL, scope_id UUID NOT NULL,
    current_version BIGINT NOT NULL DEFAULT 1 CHECK (current_version>0),
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','approved','rejected')),
    valid_approvals SMALLINT NOT NULL DEFAULT 0 CHECK (valid_approvals BETWEEN 0 AND 2),
    created_by UUID NOT NULL, created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(), updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (tenant_id,id), FOREIGN KEY (tenant_id,scope_id) REFERENCES canteiro.access_scopes(tenant_id,id),
    FOREIGN KEY (tenant_id,created_by) REFERENCES canteiro.organization_members(tenant_id,user_id)
);
CREATE TABLE canteiro.access_request_versions (
    tenant_id UUID NOT NULL, request_id UUID NOT NULL, version BIGINT NOT NULL CHECK (version>0),
    title TEXT NOT NULL CHECK (char_length(btrim(title)) BETWEEN 2 AND 200),
    action TEXT NOT NULL CHECK (action IN ('approve_budget','approve_change','authorize_payment','technical_accept')),
    increase NUMERIC NOT NULL CHECK (increase>=0 AND increase<1000000000000000000 AND scale(increase)<=2),
    decrease NUMERIC NOT NULL CHECK (decrease>=0 AND decrease<1000000000000000000 AND scale(decrease)<=2),
    policy_version BIGINT NOT NULL CHECK (policy_version>0),
    threshold NUMERIC NOT NULL CHECK (threshold>=0 AND threshold<1000000000000000000 AND scale(threshold)<=2),
    approvals_above_threshold SMALLINT NOT NULL CHECK (approvals_above_threshold BETWEEN 1 AND 2),
    segregate_proposer BOOLEAN NOT NULL, conflicted_users UUID[] NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (tenant_id,request_id,version), FOREIGN KEY (tenant_id,request_id) REFERENCES canteiro.access_requests(tenant_id,id)
);
CREATE TABLE canteiro.access_approvals (
    tenant_id UUID NOT NULL, request_id UUID NOT NULL, request_version BIGINT NOT NULL,
    actor_id UUID NOT NULL, grant_id UUID NOT NULL, grant_version BIGINT NOT NULL CHECK(grant_version>0),
    approved BOOLEAN NOT NULL, created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (tenant_id,request_id,request_version,actor_id),
    FOREIGN KEY (tenant_id,request_id,request_version) REFERENCES canteiro.access_request_versions(tenant_id,request_id,version),
    FOREIGN KEY (tenant_id,grant_id) REFERENCES canteiro.access_grants(tenant_id,id),
    FOREIGN KEY (tenant_id,actor_id) REFERENCES canteiro.organization_members(tenant_id,user_id)
);
DO $$ DECLARE table_name TEXT; BEGIN
    FOREACH table_name IN ARRAY ARRAY['access_scopes','access_grants','access_requirements','access_policies','access_requests','access_request_versions','access_approvals'] LOOP
        EXECUTE format('ALTER TABLE canteiro.%I ENABLE ROW LEVEL SECURITY',table_name);
        EXECUTE format('ALTER TABLE canteiro.%I FORCE ROW LEVEL SECURITY',table_name);
        EXECUTE format('CREATE POLICY tenant_isolation ON canteiro.%I USING (tenant_id=NULLIF(current_setting(''canteiro.tenant_id'',true),'''')::UUID) WITH CHECK (tenant_id=NULLIF(current_setting(''canteiro.tenant_id'',true),'''')::UUID)',table_name);
        EXECUTE format('REVOKE ALL ON canteiro.%I FROM PUBLIC',table_name);
    END LOOP;
END $$;

-- Responsabilidades contínuas usam titulares diretos sem vencimento automático.
-- O bloqueio da organização serializa também encerramento de membro e revogação.
CREATE FUNCTION canteiro.protect_required_authority() RETURNS trigger LANGUAGE plpgsql SET search_path=pg_catalog AS $$
DECLARE tenant UUID; BEGIN
    tenant := OLD.tenant_id;
    UPDATE canteiro.organizations SET version=version WHERE tenant_id=tenant;
    IF EXISTS (
        SELECT 1 FROM canteiro.access_requirements r WHERE r.tenant_id=tenant
        AND NOT EXISTS (
            SELECT 1 FROM canteiro.access_grants g JOIN canteiro.organization_members m
            ON m.tenant_id=g.tenant_id AND m.user_id=g.user_id
            WHERE g.tenant_id=r.tenant_id AND g.scope_id=r.scope_id AND g.role=r.role
              AND r.action=ANY(g.actions) AND g.max_amount>=r.min_amount
              AND g.source_grant_id IS NULL AND g.revoked_at IS NULL AND g.valid_from<=clock_timestamp() AND g.valid_until IS NULL
              AND m.active AND m.valid_until IS NULL)) THEN
        RAISE EXCEPTION USING ERRCODE='23514', MESSAGE='Transfira a responsabilidade obrigatória antes de encerrar o vínculo.';
    END IF;
    RETURN NULL;
END $$;
REVOKE ALL ON FUNCTION canteiro.protect_required_authority() FROM PUBLIC;
CREATE TRIGGER protect_required_grant AFTER UPDATE OR DELETE ON canteiro.access_grants FOR EACH ROW EXECUTE FUNCTION canteiro.protect_required_authority();
CREATE TRIGGER protect_required_member AFTER UPDATE OR DELETE ON canteiro.organization_members FOR EACH ROW EXECUTE FUNCTION canteiro.protect_required_authority();
