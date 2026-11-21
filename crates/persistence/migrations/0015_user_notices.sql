-- Issue 15: entrega interna transacional, tentativa do provedor e preferências por pessoa.
-- O estado da comunicação não altera o estado da pendência nem concede acesso.
CREATE TABLE canteiro.user_notice_preferences (
    tenant_id UUID NOT NULL,
    recipient_id UUID NOT NULL,
    mute_optional_email BOOLEAN NOT NULL DEFAULT false,
    group_by_day BOOLEAN NOT NULL DEFAULT true,
    reminder_days SMALLINT[] NOT NULL DEFAULT ARRAY[1,7]::SMALLINT[],
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    PRIMARY KEY (tenant_id, recipient_id),
    FOREIGN KEY (tenant_id, recipient_id) REFERENCES canteiro.organization_members(tenant_id, user_id),
    CHECK (cardinality(reminder_days) <= 10),
    CHECK (reminder_days <@ ARRAY[0,1,2,3,5,7,10,14,21,30,45,60,90,180,365]::SMALLINT[])
);
CREATE TABLE canteiro.user_notices (
    tenant_id UUID NOT NULL,
    recipient_id UUID NOT NULL,
    event_id UUID NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('technical_obligation','invitation','approval','deadline','assigned_task')),
    scope_id UUID NOT NULL,
    target_id UUID NOT NULL,
    due_on DATE,
    essential BOOLEAN NOT NULL DEFAULT true,
    state TEXT NOT NULL DEFAULT 'pending'
        CHECK (state IN ('pending','communicated','failed','resolved')),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    last_attempt TIMESTAMPTZ,
    resolved_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (tenant_id, recipient_id, event_id, kind),
    FOREIGN KEY (tenant_id, recipient_id) REFERENCES canteiro.organization_members(tenant_id, user_id),
    FOREIGN KEY (tenant_id, scope_id) REFERENCES canteiro.access_scopes(tenant_id, id),
    CHECK ((state = 'resolved') = (resolved_at IS NOT NULL)),
    CHECK (attempts > 0 OR last_attempt IS NULL)
);
CREATE INDEX user_notices_inbox
    ON canteiro.user_notices (tenant_id,recipient_id,state,due_on,created_at DESC)
    WHERE state <> 'resolved';
CREATE TABLE canteiro.user_notice_outbox (
    tenant_id UUID NOT NULL,
    recipient_id UUID NOT NULL,
    event_id UUID NOT NULL,
    kind TEXT NOT NULL,
    channel TEXT NOT NULL CHECK (channel IN ('internal','email')),
    state TEXT NOT NULL DEFAULT 'queued'
        CHECK (state IN ('queued','processing','delivered','failed')),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    ready_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    claimed_until TIMESTAMPTZ,
    delivered_at TIMESTAMPTZ,
    provider_receipt TEXT CHECK (char_length(provider_receipt) <= 200),
    PRIMARY KEY (tenant_id,recipient_id,event_id,kind,channel),
    FOREIGN KEY (tenant_id,recipient_id,event_id,kind)
        REFERENCES canteiro.user_notices(tenant_id,recipient_id,event_id,kind),
    CHECK (state <> 'delivered' OR delivered_at IS NOT NULL)
);
CREATE INDEX user_notice_ready
    ON canteiro.user_notice_outbox (ready_at,tenant_id,event_id)
    WHERE state IN ('queued','failed','processing');
CREATE TABLE canteiro.user_notice_delivery_history (
    tenant_id UUID NOT NULL,
    id UUID NOT NULL,
    recipient_id UUID NOT NULL,
    event_id UUID NOT NULL,
    kind TEXT NOT NULL,
    channel TEXT NOT NULL,
    attempt INTEGER NOT NULL CHECK (attempt > 0),
    outcome TEXT NOT NULL CHECK (outcome IN ('acknowledged','failed')),
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY(tenant_id,id),
    FOREIGN KEY (tenant_id,recipient_id,event_id,kind,channel)
        REFERENCES canteiro.user_notice_outbox(tenant_id,recipient_id,event_id,kind,channel)
);
DO $$ DECLARE item TEXT; BEGIN
    FOREACH item IN ARRAY ARRAY[
        'user_notice_preferences','user_notices','user_notice_outbox','user_notice_delivery_history'
    ] LOOP
        EXECUTE format('ALTER TABLE canteiro.%I ENABLE ROW LEVEL SECURITY',item);
        EXECUTE format('ALTER TABLE canteiro.%I FORCE ROW LEVEL SECURITY',item);
        EXECUTE format('CREATE POLICY tenant_isolation ON canteiro.%I USING (tenant_id=NULLIF(current_setting(''canteiro.tenant_id'',true),'''')::UUID) WITH CHECK (tenant_id=NULLIF(current_setting(''canteiro.tenant_id'',true),'''')::UUID)',item);
        EXECUTE format('REVOKE ALL ON canteiro.%I FROM PUBLIC',item);
    END LOOP;
END $$;
-- O ator vê e altera somente suas preferências. O serviço aplica ainda os direitos de escopo atuais.
CREATE POLICY own_notice_prefs ON canteiro.user_notice_preferences AS RESTRICTIVE
    USING(recipient_id=NULLIF(current_setting('canteiro.actor_id',true),'')::UUID)
    WITH CHECK(recipient_id=NULLIF(current_setting('canteiro.actor_id',true),'')::UUID);
CREATE POLICY own_notice_records ON canteiro.user_notices AS RESTRICTIVE FOR SELECT
    USING(recipient_id=NULLIF(current_setting('canteiro.actor_id',true),'')::UUID);
-- Histórico de tentativa de comunicação imutável, com evolução por novos fatos.
CREATE TRIGGER immutable_notice_delivery_history BEFORE UPDATE OR DELETE OR TRUNCATE
    ON canteiro.user_notice_delivery_history
    FOR EACH STATEMENT EXECUTE FUNCTION canteiro.reject_history_rewrite();
