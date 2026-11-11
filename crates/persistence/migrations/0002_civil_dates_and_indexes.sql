-- Expansão aditiva compatível com a versão 1 e com registros existentes.
ALTER TABLE canteiro.organization_units ADD COLUMN effective_on DATE
    CHECK (effective_on BETWEEN DATE '0001-01-01' AND DATE '9999-12-31');
CREATE INDEX organization_units_parent_idx ON canteiro.organization_units(tenant_id, parent_id);
CREATE INDEX organization_units_created_idx ON canteiro.organization_units(tenant_id, created_at, id);
ALTER TABLE canteiro.organizations ADD CONSTRAINT organizations_created_range
    CHECK (created_at >= TIMESTAMPTZ '0001-01-01 00:00:00+00' AND created_at < TIMESTAMPTZ '10000-01-01 00:00:00+00');
ALTER TABLE canteiro.organization_units ADD CONSTRAINT units_instants_range
    CHECK (created_at >= TIMESTAMPTZ '0001-01-01 00:00:00+00' AND created_at < TIMESTAMPTZ '10000-01-01 00:00:00+00'
       AND updated_at >= created_at AND updated_at < TIMESTAMPTZ '10000-01-01 00:00:00+00');
