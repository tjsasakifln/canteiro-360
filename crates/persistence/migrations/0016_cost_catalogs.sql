-- Issue 16: preço histórico com fonte, competência, licença e revisão escolhida.
CREATE TABLE canteiro.cost_catalogs (
    tenant_id UUID NOT NULL,
    catalog_id TEXT NOT NULL CHECK (char_length(btrim(catalog_id)) BETWEEN 1 AND 128),
    scope_id UUID NOT NULL,
    title TEXT NOT NULL CHECK (char_length(btrim(title)) BETWEEN 2 AND 160),
    created_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY(tenant_id,catalog_id),
    FOREIGN KEY(tenant_id,scope_id) REFERENCES canteiro.access_scopes(tenant_id,id),
    FOREIGN KEY(tenant_id,created_by) REFERENCES canteiro.organization_members(tenant_id,user_id)
);
CREATE TABLE canteiro.cost_catalog_editions (
    tenant_id UUID NOT NULL,
    catalog_id TEXT NOT NULL,
    edition INTEGER NOT NULL CHECK(edition>0),
    source_title TEXT NOT NULL CHECK(char_length(btrim(source_title)) BETWEEN 1 AND 160),
    source_organization TEXT NOT NULL CHECK(char_length(btrim(source_organization)) BETWEEN 1 AND 160),
    license TEXT NOT NULL CHECK(char_length(btrim(license)) BETWEEN 1 AND 300),
    locality TEXT NOT NULL CHECK(char_length(btrim(locality)) BETWEEN 1 AND 120),
    tax_regime TEXT NOT NULL CHECK(char_length(btrim(tax_regime)) BETWEEN 1 AND 120),
    competence DATE NOT NULL CHECK (EXTRACT(DAY FROM competence)=1),
    published_on DATE NOT NULL,
    expected_items INTEGER NOT NULL CHECK(expected_items BETWEEN 1 AND 50000),
    state TEXT NOT NULL DEFAULT 'draft' CHECK(state IN ('draft','published')),
    published_at TIMESTAMPTZ,
    created_by UUID NOT NULL,
    PRIMARY KEY(tenant_id,catalog_id,edition),
    FOREIGN KEY(tenant_id,catalog_id) REFERENCES canteiro.cost_catalogs(tenant_id,catalog_id),
    FOREIGN KEY(tenant_id,created_by) REFERENCES canteiro.organization_members(tenant_id,user_id),
    CHECK((state='published')=(published_at IS NOT NULL))
);
CREATE TABLE canteiro.cost_catalog_items (
    tenant_id UUID NOT NULL,
    catalog_id TEXT NOT NULL,
    edition INTEGER NOT NULL,
    code TEXT NOT NULL CHECK(char_length(btrim(code)) BETWEEN 1 AND 80),
    description TEXT NOT NULL CHECK(char_length(btrim(description)) BETWEEN 1 AND 800),
    category TEXT NOT NULL CHECK(category IN ('material','labor','equipment','service','transport')),
    unit TEXT NOT NULL CHECK(unit IN ('un','m','m²','m³','kg','t','L','h')),
    price NUMERIC(26,6) NOT NULL CHECK(price>=0),
    PRIMARY KEY(tenant_id,catalog_id,edition,code),
    FOREIGN KEY(tenant_id,catalog_id,edition)
        REFERENCES canteiro.cost_catalog_editions(tenant_id,catalog_id,edition)
);
CREATE INDEX cost_catalog_history ON canteiro.cost_catalog_editions
    (tenant_id,catalog_id,competence DESC,edition DESC)
    WHERE state='published';
CREATE FUNCTION canteiro.cost_catalog_items_guard() RETURNS trigger
LANGUAGE plpgsql SET search_path=pg_catalog AS $$
DECLARE current_state TEXT;
BEGIN
    SELECT e.state INTO current_state FROM canteiro.cost_catalog_editions e
    WHERE e.tenant_id=COALESCE(NEW.tenant_id,OLD.tenant_id)
    AND e.catalog_id=COALESCE(NEW.catalog_id,OLD.catalog_id)
    AND e.edition=COALESCE(NEW.edition,OLD.edition) FOR UPDATE;
    IF current_state IS NULL OR current_state='published' THEN
        RAISE EXCEPTION USING ERRCODE='23514', MESSAGE='A edição publicada é imutável.';
    END IF;
    IF TG_OP='UPDATE' AND (
        NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
        OR NEW.catalog_id IS DISTINCT FROM OLD.catalog_id
        OR NEW.edition IS DISTINCT FROM OLD.edition
        OR NEW.code IS DISTINCT FROM OLD.code) THEN
        RAISE EXCEPTION USING ERRCODE='23514', MESSAGE='Não é possível trocar a identidade de um insumo.';
    END IF;
    RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END;
END $$;
CREATE TRIGGER cost_catalog_items_guard BEFORE INSERT OR UPDATE OR DELETE
    ON canteiro.cost_catalog_items FOR EACH ROW
    EXECUTE FUNCTION canteiro.cost_catalog_items_guard();
CREATE FUNCTION canteiro.cost_catalog_edition_guard() RETURNS trigger
LANGUAGE plpgsql SET search_path=pg_catalog AS $$
DECLARE count_items BIGINT;
BEGIN
    IF TG_OP='DELETE' THEN
        IF OLD.state='published' THEN
            RAISE EXCEPTION USING ERRCODE='23514',MESSAGE='A edição publicada é imutável.';
        END IF;
        RETURN OLD;
    END IF;
    IF OLD.state='published' OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
        OR NEW.catalog_id IS DISTINCT FROM OLD.catalog_id
        OR NEW.edition IS DISTINCT FROM OLD.edition THEN
        RAISE EXCEPTION USING ERRCODE='23514',MESSAGE='A identidade ou edição publicada é imutável.';
    END IF;
    IF NEW.state='published' THEN
        SELECT count(*) INTO count_items FROM canteiro.cost_catalog_items i
          WHERE i.tenant_id=NEW.tenant_id AND i.catalog_id=NEW.catalog_id
            AND i.edition=NEW.edition;
        IF count_items<>NEW.expected_items THEN
            RAISE EXCEPTION USING ERRCODE='23514',MESSAGE='O catálogo ainda não possui todas as linhas validadas.';
        END IF;
        NEW.published_at:=clock_timestamp();
    ELSIF NEW.published_at IS NOT NULL THEN
        RAISE EXCEPTION USING ERRCODE='23514',MESSAGE='Publicação incorreta.';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER cost_catalog_edition_guard BEFORE UPDATE OR DELETE
    ON canteiro.cost_catalog_editions FOR EACH ROW
    EXECUTE FUNCTION canteiro.cost_catalog_edition_guard();
DO $$ DECLARE item TEXT; BEGIN
    FOREACH item IN ARRAY ARRAY[
      'cost_catalogs','cost_catalog_editions','cost_catalog_items'
    ] LOOP
      EXECUTE format('ALTER TABLE canteiro.%I ENABLE ROW LEVEL SECURITY',item);
      EXECUTE format('ALTER TABLE canteiro.%I FORCE ROW LEVEL SECURITY',item);
      EXECUTE format('CREATE POLICY tenant_isolation ON canteiro.%I USING (tenant_id=NULLIF(current_setting(''canteiro.tenant_id'',true),'''')::UUID) WITH CHECK (tenant_id=NULLIF(current_setting(''canteiro.tenant_id'',true),'''')::UUID)',item);
      EXECUTE format('REVOKE ALL ON canteiro.%I FROM PUBLIC',item);
    END LOOP;
END $$;
