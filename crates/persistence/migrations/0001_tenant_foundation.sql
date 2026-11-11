-- Fundação técnica. Cadastro, membros e convites completos pertencem à issue 7.
CREATE SCHEMA canteiro;
REVOKE ALL ON SCHEMA canteiro FROM PUBLIC;
CREATE TABLE canteiro.organizations (
    tenant_id UUID PRIMARY KEY CHECK (tenant_id NOT IN ('00000000-0000-0000-0000-000000000000', 'ffffffff-ffff-ffff-ffff-ffffffffffff')),
    name TEXT NOT NULL CHECK (char_length(btrim(name)) >= 1 AND char_length(name) <= 160),
    created_by UUID NOT NULL CHECK (created_by NOT IN ('00000000-0000-0000-0000-000000000000', 'ffffffff-ffff-ffff-ffff-ffffffffffff')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT transaction_timestamp(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0)
);
CREATE TABLE canteiro.organization_units (
    tenant_id UUID NOT NULL REFERENCES canteiro.organizations(tenant_id),
    id UUID NOT NULL CHECK (id NOT IN ('00000000-0000-0000-0000-000000000000', 'ffffffff-ffff-ffff-ffff-ffffffffffff')),
    parent_id UUID,
    name TEXT NOT NULL CHECK (char_length(btrim(name)) >= 1 AND char_length(name) <= 160),
    created_by UUID NOT NULL CHECK (created_by NOT IN ('00000000-0000-0000-0000-000000000000', 'ffffffff-ffff-ffff-ffff-ffffffffffff')),
    updated_by UUID NOT NULL CHECK (updated_by NOT IN ('00000000-0000-0000-0000-000000000000', 'ffffffff-ffff-ffff-ffff-ffffffffffff')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT transaction_timestamp(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT transaction_timestamp(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    PRIMARY KEY (tenant_id, id),
    FOREIGN KEY (tenant_id, parent_id) REFERENCES canteiro.organization_units(tenant_id, id),
    CHECK (parent_id IS NULL OR parent_id <> id)
);
ALTER TABLE canteiro.organizations ENABLE ROW LEVEL SECURITY;
ALTER TABLE canteiro.organizations FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON canteiro.organizations
    USING (tenant_id = NULLIF(current_setting('canteiro.tenant_id', true), '')::UUID)
    WITH CHECK (tenant_id = NULLIF(current_setting('canteiro.tenant_id', true), '')::UUID);
ALTER TABLE canteiro.organization_units ENABLE ROW LEVEL SECURITY;
ALTER TABLE canteiro.organization_units FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON canteiro.organization_units
    USING (tenant_id = NULLIF(current_setting('canteiro.tenant_id', true), '')::UUID)
    WITH CHECK (tenant_id = NULLIF(current_setting('canteiro.tenant_id', true), '')::UUID);
