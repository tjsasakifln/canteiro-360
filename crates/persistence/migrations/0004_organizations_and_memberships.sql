-- Organizações e direitos de participação. A seleção de empresa não concede acesso a obras.
ALTER TABLE canteiro.organizations ADD COLUMN access_code_hash BYTEA
    CHECK (access_code_hash IS NULL OR octet_length(access_code_hash) = 32);

CREATE TABLE canteiro.organization_members (
    tenant_id UUID NOT NULL REFERENCES canteiro.organizations(tenant_id),
    user_id UUID NOT NULL REFERENCES canteiro_auth.users(id),
    role TEXT NOT NULL CHECK (role IN ('administrator', 'member')),
    active BOOLEAN NOT NULL DEFAULT true,
    valid_until TIMESTAMPTZ,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (tenant_id, user_id),
    CHECK (role <> 'administrator' OR valid_until IS NULL)
);
CREATE INDEX organization_members_person ON canteiro.organization_members(user_id, tenant_id)
    WHERE active;

CREATE TABLE canteiro.organization_invites (
    tenant_id UUID NOT NULL REFERENCES canteiro.organizations(tenant_id),
    id UUID NOT NULL,
    email TEXT NOT NULL CHECK (email = lower(btrim(email)) AND char_length(email) BETWEEN 3 AND 254),
    role TEXT NOT NULL CHECK (role IN ('administrator', 'member')),
    token_hash BYTEA NOT NULL CHECK (octet_length(token_hash) = 32),
    expires_at TIMESTAMPTZ NOT NULL,
    membership_valid_until TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    accepted_at TIMESTAMPTZ,
    created_by UUID NOT NULL REFERENCES canteiro_auth.users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    PRIMARY KEY (tenant_id, id),
    UNIQUE (tenant_id, token_hash),
    CHECK (expires_at > created_at),
    CHECK (role <> 'administrator' OR membership_valid_until IS NULL),
    CHECK (revoked_at IS NULL OR accepted_at IS NULL)
);
CREATE INDEX organization_invites_history ON canteiro.organization_invites(tenant_id, created_at DESC, id);

CREATE TABLE canteiro.organization_access_requests (
    tenant_id UUID NOT NULL REFERENCES canteiro.organizations(tenant_id),
    id UUID NOT NULL,
    user_id UUID NOT NULL REFERENCES canteiro_auth.users(id),
    requested_role TEXT NOT NULL CHECK (requested_role IN ('administrator', 'member')),
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'approved', 'denied')),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    decided_by UUID REFERENCES canteiro_auth.users(id),
    decided_at TIMESTAMPTZ,
    PRIMARY KEY (tenant_id, id),
    CHECK ((status = 'pending' AND decided_by IS NULL AND decided_at IS NULL)
        OR (status <> 'pending' AND decided_by IS NOT NULL AND decided_at IS NOT NULL))
);
CREATE UNIQUE INDEX organization_access_requests_pending
    ON canteiro.organization_access_requests(tenant_id, user_id) WHERE status = 'pending';
CREATE INDEX organization_access_requests_history
    ON canteiro.organization_access_requests(tenant_id, created_at DESC, id);

CREATE TABLE canteiro.organization_audit (
    tenant_id UUID NOT NULL REFERENCES canteiro.organizations(tenant_id),
    id UUID NOT NULL,
    actor_id UUID NOT NULL REFERENCES canteiro_auth.users(id),
    event TEXT NOT NULL CHECK (char_length(event) BETWEEN 1 AND 80),
    subject_id UUID,
    detail TEXT NOT NULL DEFAULT '' CHECK (char_length(detail) <= 500),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (tenant_id, id)
);
CREATE INDEX organization_audit_history ON canteiro.organization_audit(tenant_id, created_at, id);

ALTER TABLE canteiro.organization_members ENABLE ROW LEVEL SECURITY;
ALTER TABLE canteiro.organization_members FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON canteiro.organization_members
    USING (tenant_id = NULLIF(current_setting('canteiro.tenant_id', true), '')::UUID)
    WITH CHECK (tenant_id = NULLIF(current_setting('canteiro.tenant_id', true), '')::UUID);
-- Sem empresa selecionada, a pessoa consulta somente seus próprios vínculos.
-- Esta política é exclusivamente de leitura e não permite conceder direitos.
CREATE POLICY own_memberships ON canteiro.organization_members FOR SELECT
    USING (NULLIF(current_setting('canteiro.tenant_id', true), '') IS NULL
        AND user_id = NULLIF(current_setting('canteiro.actor_id', true), '')::UUID);

ALTER TABLE canteiro.organization_invites ENABLE ROW LEVEL SECURITY;
ALTER TABLE canteiro.organization_invites FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON canteiro.organization_invites
    USING (tenant_id = NULLIF(current_setting('canteiro.tenant_id', true), '')::UUID)
    WITH CHECK (tenant_id = NULLIF(current_setting('canteiro.tenant_id', true), '')::UUID);
ALTER TABLE canteiro.organization_access_requests ENABLE ROW LEVEL SECURITY;
ALTER TABLE canteiro.organization_access_requests FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON canteiro.organization_access_requests
    USING (tenant_id = NULLIF(current_setting('canteiro.tenant_id', true), '')::UUID)
    WITH CHECK (tenant_id = NULLIF(current_setting('canteiro.tenant_id', true), '')::UUID);
ALTER TABLE canteiro.organization_audit ENABLE ROW LEVEL SECURITY;
ALTER TABLE canteiro.organization_audit FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON canteiro.organization_audit
    USING (tenant_id = NULLIF(current_setting('canteiro.tenant_id', true), '')::UUID)
    WITH CHECK (tenant_id = NULLIF(current_setting('canteiro.tenant_id', true), '')::UUID
        AND actor_id = NULLIF(current_setting('canteiro.actor_id', true), '')::UUID);

CREATE TABLE canteiro_auth.organization_selections (
    session_id UUID PRIMARY KEY REFERENCES canteiro_auth.sessions(id),
    tenant_id UUID REFERENCES canteiro.organizations(tenant_id),
    generation BIGINT NOT NULL DEFAULT 1 CHECK (generation > 0)
);
ALTER TABLE canteiro_auth.organization_selections ENABLE ROW LEVEL SECURITY;
ALTER TABLE canteiro_auth.organization_selections FORCE ROW LEVEL SECURITY;
CREATE POLICY own_session_selection ON canteiro_auth.organization_selections
    USING (EXISTS (SELECT 1 FROM canteiro_auth.sessions s
        WHERE s.id = session_id AND s.user_id = NULLIF(current_setting('canteiro.actor_id', true), '')::UUID))
    WITH CHECK (EXISTS (SELECT 1 FROM canteiro_auth.sessions s
        WHERE s.id = session_id AND s.user_id = NULLIF(current_setting('canteiro.actor_id', true), '')::UUID));

-- Defesa no próprio banco. A aplicação adquire primeiro o mesmo bloqueio da
-- organização e confere novamente o ator, a versão e o número de administradores.
-- Não muda organizações legadas da fundação nem cria um administrador fictício.
CREATE FUNCTION canteiro.protect_last_administrator() RETURNS trigger
    LANGUAGE plpgsql SET search_path = pg_catalog AS $$
BEGIN
    IF TG_OP = 'UPDATE' AND (NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
        OR NEW.user_id IS DISTINCT FROM OLD.user_id) THEN
        RAISE EXCEPTION USING ERRCODE = '23514', MESSAGE = 'A identidade do vínculo não pode ser alterada.';
    END IF;
    IF OLD.active AND OLD.role = 'administrator'
       AND (TG_OP = 'DELETE' OR NOT NEW.active OR NEW.role <> 'administrator') THEN
        -- Uma escrita, ainda que sem mudar a versão lógica, também impede write
        -- skew em REPEATABLE READ: o concorrente recebe falha de serialização.
        UPDATE canteiro.organizations SET version = version WHERE tenant_id = OLD.tenant_id;
        IF NOT EXISTS (SELECT 1 FROM canteiro.organization_members
            WHERE tenant_id = OLD.tenant_id AND user_id <> OLD.user_id
              AND active AND role = 'administrator') THEN
            RAISE EXCEPTION USING ERRCODE = '23514', MESSAGE = 'A organização precisa de um administrador ativo.';
        END IF;
    END IF;
    IF TG_OP = 'DELETE' THEN RETURN OLD; END IF;
    RETURN NEW;
END;
$$;
REVOKE ALL ON FUNCTION canteiro.protect_last_administrator() FROM PUBLIC;
CREATE TRIGGER protect_last_administrator
    BEFORE UPDATE OR DELETE ON canteiro.organization_members
    FOR EACH ROW EXECUTE FUNCTION canteiro.protect_last_administrator();

REVOKE ALL ON canteiro.organization_members, canteiro.organization_invites,
    canteiro.organization_access_requests, canteiro.organization_audit,
    canteiro_auth.organization_selections FROM PUBLIC;
