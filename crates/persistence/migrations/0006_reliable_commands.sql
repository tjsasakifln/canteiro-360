-- Contexto de execução não faz parte da chave de negócio. A identidade do
-- efeito permanece disponível mesmo que nenhuma resposta esteja em cache.
CREATE TABLE canteiro.operation_receipts (
    tenant_id UUID NOT NULL, act TEXT NOT NULL, operation_key UUID NOT NULL,
    actor_id UUID NOT NULL, scope_id UUID NOT NULL,
    fingerprint BYTEA NOT NULL CHECK (octet_length(fingerprint)=32),
    result_id UUID NOT NULL, result_version BIGINT NOT NULL CHECK (result_version>0),
    PRIMARY KEY (tenant_id,act,operation_key),
    FOREIGN KEY (tenant_id,scope_id) REFERENCES canteiro.access_scopes(tenant_id,id),
    FOREIGN KEY (tenant_id,actor_id) REFERENCES canteiro.organization_members(tenant_id,user_id),
    CHECK (act IN ('contract_reference_rename','authority_propose','authority_revise','authority_decide'))
);

CREATE TABLE canteiro.contract_history (
    tenant_id UUID NOT NULL, contract_id UUID NOT NULL, version BIGINT NOT NULL CHECK (version>0),
    event_id UUID NOT NULL DEFAULT gen_random_uuid(),
    actor_id UUID NOT NULL REFERENCES canteiro_auth.users(id),
    action TEXT NOT NULL, origin TEXT NOT NULL,
    before_title TEXT, after_title TEXT NOT NULL, reason TEXT NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL, recorded_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (tenant_id,contract_id,version), UNIQUE (tenant_id,event_id),
    FOREIGN KEY (tenant_id,contract_id) REFERENCES canteiro.access_scopes(tenant_id,id),
    CHECK (char_length(btrim(after_title)) BETWEEN 2 AND 160),
    CHECK (char_length(btrim(reason)) BETWEEN 2 AND 200),
    CHECK (isfinite(occurred_at) AND isfinite(recorded_at) AND occurred_at<=recorded_at),
    CHECK (
        (action='contract_reference_created' AND origin IN ('cadastro','migração') AND before_title IS NULL)
        OR (action='contract_reference_renamed' AND origin='web' AND before_title IS NOT NULL)
    )
);

CREATE TABLE canteiro.contract_edit_leases (
    tenant_id UUID NOT NULL, contract_id UUID NOT NULL,
    actor_id UUID NOT NULL, session_id UUID NOT NULL REFERENCES canteiro_auth.sessions(id) ON DELETE CASCADE,
    lease_id UUID NOT NULL, expires_at TIMESTAMPTZ NOT NULL CHECK (isfinite(expires_at)),
    PRIMARY KEY (tenant_id,contract_id),
    FOREIGN KEY (tenant_id,contract_id) REFERENCES canteiro.access_scopes(tenant_id,id),
    FOREIGN KEY (tenant_id,actor_id) REFERENCES canteiro.organization_members(tenant_id,user_id)
);
CREATE INDEX contract_edit_leases_session ON canteiro.contract_edit_leases(session_id);

-- Na atualização de uma instalação, a primeira linha é o estado cuja autoria
-- original já era conhecida. As mudanças futuras entram como novas versões.
-- O lock de DDL protege a janela em que o dono precisa ler o legado sob RLS.
ALTER TABLE canteiro.access_scopes NO FORCE ROW LEVEL SECURITY;
WITH migration_clock AS MATERIALIZED (SELECT clock_timestamp() AS recorded_at)
INSERT INTO canteiro.contract_history
    (tenant_id,contract_id,version,actor_id,action,origin,after_title,reason,occurred_at,recorded_at)
SELECT tenant_id,id,version,created_by,'contract_reference_created','migração',name,
       'Referência existente na implantação do histórico.',
       LEAST(created_at,migration_clock.recorded_at),migration_clock.recorded_at
FROM canteiro.access_scopes CROSS JOIN migration_clock
WHERE kind='contract';
ALTER TABLE canteiro.access_scopes FORCE ROW LEVEL SECURITY;

DO $$ DECLARE table_name TEXT; BEGIN
    FOREACH table_name IN ARRAY ARRAY['operation_receipts','contract_history','contract_edit_leases'] LOOP
        EXECUTE format('ALTER TABLE canteiro.%I ENABLE ROW LEVEL SECURITY',table_name);
        EXECUTE format('ALTER TABLE canteiro.%I FORCE ROW LEVEL SECURITY',table_name);
        EXECUTE format('CREATE POLICY tenant_read ON canteiro.%I FOR SELECT USING
            (tenant_id=NULLIF(current_setting(''canteiro.tenant_id'',true),'''')::UUID)',table_name);
        EXECUTE format('REVOKE ALL ON canteiro.%I FROM PUBLIC',table_name);
    END LOOP;
END $$;

ALTER POLICY tenant_read ON canteiro.operation_receipts USING (
    tenant_id=NULLIF(current_setting('canteiro.tenant_id',true),'')::UUID
    AND actor_id=NULLIF(current_setting('canteiro.actor_id',true),'')::UUID
);

CREATE POLICY receipt_author ON canteiro.operation_receipts FOR INSERT WITH CHECK (
    tenant_id=NULLIF(current_setting('canteiro.tenant_id',true),'')::UUID
    AND actor_id=NULLIF(current_setting('canteiro.actor_id',true),'')::UUID
);
CREATE POLICY history_insert ON canteiro.contract_history FOR INSERT WITH CHECK (
    tenant_id=NULLIF(current_setting('canteiro.tenant_id',true),'')::UUID
);
CREATE POLICY lease_write ON canteiro.contract_edit_leases FOR ALL
    USING (tenant_id=NULLIF(current_setting('canteiro.tenant_id',true),'')::UUID)
    WITH CHECK (
        tenant_id=NULLIF(current_setting('canteiro.tenant_id',true),'')::UUID
        AND actor_id=NULLIF(current_setting('canteiro.actor_id',true),'')::UUID
    );

CREATE FUNCTION canteiro.reject_history_rewrite() RETURNS trigger
LANGUAGE plpgsql SET search_path=pg_catalog AS $$
BEGIN
    RAISE EXCEPTION USING ERRCODE='42501', MESSAGE='O histórico confirmado permite somente inclusão.';
END $$;
REVOKE ALL ON FUNCTION canteiro.reject_history_rewrite() FROM PUBLIC;
CREATE TRIGGER contract_history_immutable BEFORE UPDATE OR DELETE OR TRUNCATE ON canteiro.contract_history
    FOR EACH STATEMENT EXECUTE FUNCTION canteiro.reject_history_rewrite();
CREATE TRIGGER operation_receipts_immutable BEFORE UPDATE OR DELETE OR TRUNCATE ON canteiro.operation_receipts
    FOR EACH STATEMENT EXECUTE FUNCTION canteiro.reject_history_rewrite();

-- O aplicativo não recebe INSERT no histórico. A mesma transação que cria a
-- referência grava a primeira revisão, inclusive na rota de permissões da #8.
CREATE FUNCTION canteiro.record_contract_reference() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp SET row_security=on AS $$
DECLARE active_tenant TEXT := current_setting('canteiro.tenant_id',true);
    active_actor UUID := NULLIF(current_setting('canteiro.actor_id',true),'')::UUID;
BEGIN
    IF active_actor IS NOT NULL AND NEW.created_by<>active_actor THEN
        RAISE EXCEPTION USING ERRCODE='42501', MESSAGE='A autoria da referência não corresponde à sessão.';
    END IF;
    PERFORM set_config('canteiro.tenant_id',NEW.tenant_id::TEXT,true);
    INSERT INTO canteiro.contract_history
        (tenant_id,contract_id,version,actor_id,action,origin,after_title,reason,occurred_at,recorded_at)
    VALUES (NEW.tenant_id,NEW.id,NEW.version,NEW.created_by,'contract_reference_created','cadastro',NEW.name,
            'Cadastro da referência contratual.',NEW.created_at,NEW.created_at);
    PERFORM set_config('canteiro.tenant_id',COALESCE(active_tenant,''),true);
    RETURN NEW;
END $$;
REVOKE ALL ON FUNCTION canteiro.record_contract_reference() FROM PUBLIC;
CREATE TRIGGER contract_reference_created AFTER INSERT ON canteiro.access_scopes
    FOR EACH ROW WHEN (NEW.kind='contract') EXECUTE FUNCTION canteiro.record_contract_reference();

-- Verificação adicional para a função com privilégios elevados. O mesmo lock
-- mantém a concessão atual durante a escrita; o servidor também aplica o domínio Rust.
CREATE FUNCTION canteiro.contract_editor_available(tenant UUID, contract UUID, actor UUID, at_time TIMESTAMPTZ)
RETURNS BOOLEAN LANGUAGE sql SET search_path=pg_catalog,pg_temp AS $$
    WITH available AS (
        SELECT g.*,
            LEAST(g.valid_until,m.valid_until) AS valid_end
        FROM canteiro.access_grants g JOIN canteiro.organization_members m USING (tenant_id,user_id)
        WHERE g.tenant_id=tenant AND g.scope_id=contract AND m.active
          AND g.revoked_at IS NULL AND g.valid_from<=at_time
          AND (LEAST(g.valid_until,m.valid_until) IS NULL OR LEAST(g.valid_until,m.valid_until)>at_time)
    ), permitted AS (
        SELECT g.* FROM available g
        LEFT JOIN available src ON src.id=g.source_grant_id
        WHERE g.user_id=actor AND (
            g.source_grant_id IS NULL OR (
                src.source_grant_id IS NULL AND src.user_id<>actor AND src.actions<>ARRAY['view']::TEXT[]
                AND g.actions<@src.actions AND g.max_amount<=src.max_amount
                AND g.valid_from>=src.valid_from AND g.valid_end IS NOT NULL
            )
        )
    )
    SELECT COALESCE(bool_or('view'=ANY(actions)), false)
       AND COALESCE(bool_or(role='manager' AND 'edit_contract_record'=ANY(actions)), false)
    FROM permitted
$$;
REVOKE ALL ON FUNCTION canteiro.contract_editor_available(UUID,UUID,UUID,TIMESTAMPTZ) FROM PUBLIC;

-- Somente a revisão do título contratual está disponível por este caminho.
-- O serviço autoriza o ato sob o lock da organização; o banco também exige o
-- lease da mesma sessão e a versão correta, e fecha o efeito com sua auditoria.
CREATE FUNCTION canteiro.rename_contract_reference(
    tenant UUID, contract UUID, actor UUID, editing_session UUID, lease UUID,
    expected BIGINT, title TEXT, reason TEXT, occurred TIMESTAMPTZ
) RETURNS UUID LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp SET row_security=on AS $$
DECLARE previous_title TEXT; lease_end TIMESTAMPTZ; saved_at TIMESTAMPTZ; saved_event UUID;
BEGIN
    IF tenant IS DISTINCT FROM NULLIF(current_setting('canteiro.tenant_id',true),'')::UUID
       OR actor IS DISTINCT FROM NULLIF(current_setting('canteiro.actor_id',true),'')::UUID THEN
        RAISE EXCEPTION USING ERRCODE='42501', MESSAGE='O contexto da gravação não corresponde à sessão.';
    END IF;
    PERFORM 1 FROM canteiro.organizations o WHERE o.tenant_id=tenant FOR NO KEY UPDATE;
    IF NOT FOUND THEN
        RETURN NULL;
    END IF;
    SELECT s.name INTO previous_title FROM canteiro.access_scopes s
        WHERE s.tenant_id=tenant AND s.id=contract AND s.kind='contract' AND s.version=expected
        FOR NO KEY UPDATE;
    IF NOT FOUND THEN
        RETURN NULL;
    END IF;
    -- Confere e protege a reserva mesmo diante de outra conexão.
    SELECT e.expires_at INTO lease_end FROM canteiro.contract_edit_leases e
        WHERE e.tenant_id=tenant AND e.contract_id=contract AND e.actor_id=actor
          AND e.session_id=editing_session AND e.lease_id=lease
        FOR UPDATE;
    IF NOT FOUND THEN
        RETURN NULL;
    END IF;
    saved_at := clock_timestamp();
    IF lease_end<=saved_at OR NOT EXISTS (
        SELECT 1 FROM canteiro_auth.sessions s JOIN canteiro_auth.users u ON u.id=s.user_id
        JOIN canteiro_auth.organization_selections selection ON selection.session_id=s.id AND selection.tenant_id=tenant
        WHERE s.id=editing_session AND s.user_id=actor AND s.stage='authenticated'
          AND s.revoked_at IS NULL AND s.expires_at>saved_at
          AND s.last_seen_at>saved_at-interval '30 minutes'
          AND s.password_version=u.password_version AND u.verified
          AND (NOT u.privileged AND u.mfa_secret IS NULL OR s.mfa_verified)
    ) OR NOT canteiro.contract_editor_available(tenant,contract,actor,saved_at) THEN
        RETURN NULL;
    END IF;
    UPDATE canteiro.access_scopes s SET name=title,version=s.version+1
        WHERE s.tenant_id=tenant AND s.id=contract AND s.version=expected;
    INSERT INTO canteiro.contract_history
        (tenant_id,contract_id,version,actor_id,action,origin,before_title,after_title,reason,occurred_at,recorded_at)
    VALUES (tenant,contract,expected+1,actor,'contract_reference_renamed','web',previous_title,title,reason,
            COALESCE(occurred,saved_at),saved_at)
    RETURNING event_id INTO saved_event;
    PERFORM canteiro_meta.enqueue_contract_notification(tenant,saved_event);
    RETURN saved_event;
END $$;
REVOKE ALL ON FUNCTION canteiro.rename_contract_reference(UUID,UUID,UUID,UUID,UUID,BIGINT,TEXT,TEXT,TIMESTAMPTZ) FROM PUBLIC;

-- Amplia a lista da #8 com um direito específico para a referência de
-- contrato, sem mudar o significado de propor ou aprovar alteração de valor.
ALTER TABLE canteiro.access_grants DROP CONSTRAINT access_grants_actions_check;
ALTER TABLE canteiro.access_grants ADD CONSTRAINT access_grants_actions_check CHECK (
    cardinality(actions) BETWEEN 1 AND 12 AND actions <@ ARRAY[
        'view','edit_budget','review_budget','record_execution','review_measurement','propose_change',
        'approve_budget','approve_change','authorize_payment','record_payment','technical_accept','edit_contract_record'
    ]::TEXT[]
    AND (NOT ('edit_contract_record'=ANY(actions)) OR role='manager')
);
