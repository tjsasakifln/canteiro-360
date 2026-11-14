-- Descoberta da fila entre empresas. Somente referências opacas e estado de entrega.
CREATE TABLE canteiro_meta.contract_notification_outbox (
    tenant_id UUID NOT NULL,
    event_id UUID NOT NULL,
    queued_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    attempts BIGINT NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    delivered_at TIMESTAMPTZ,
    PRIMARY KEY (tenant_id, event_id),
    FOREIGN KEY (tenant_id, event_id) REFERENCES canteiro.contract_history(tenant_id, event_id)
);
CREATE INDEX contract_notification_pending
    ON canteiro_meta.contract_notification_outbox(next_attempt_at, queued_at, tenant_id, event_id)
    WHERE delivered_at IS NULL;
REVOKE ALL ON canteiro_meta.contract_notification_outbox FROM PUBLIC;

-- A própria notificação é a prova durável de deduplicação, vinculada ao histórico.
CREATE TABLE canteiro.contract_notifications (
    tenant_id UUID NOT NULL,
    event_id UUID NOT NULL,
    delivered_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (tenant_id, event_id),
    FOREIGN KEY (tenant_id, event_id) REFERENCES canteiro.contract_history(tenant_id, event_id)
);
ALTER TABLE canteiro.contract_notifications ENABLE ROW LEVEL SECURITY;
ALTER TABLE canteiro.contract_notifications FORCE ROW LEVEL SECURITY;
CREATE POLICY contract_notification_recipient ON canteiro.contract_notifications
    USING (
        tenant_id = NULLIF(current_setting('canteiro.tenant_id', true), '')::UUID
        AND EXISTS (
            SELECT 1 FROM canteiro.contract_history h
            WHERE h.tenant_id = contract_notifications.tenant_id
              AND h.event_id = contract_notifications.event_id
              AND h.actor_id = NULLIF(current_setting('canteiro.actor_id', true), '')::UUID
        )
    );
REVOKE ALL ON canteiro.contract_notifications FROM PUBLIC;
CREATE TRIGGER contract_notification_delivery_immutable
    BEFORE UPDATE OR DELETE OR TRUNCATE ON canteiro.contract_notifications
    FOR EACH STATEMENT EXECUTE FUNCTION canteiro.reject_history_rewrite();

CREATE FUNCTION canteiro_meta.enqueue_contract_notification(p_tenant UUID, p_event UUID)
    RETURNS VOID LANGUAGE plpgsql SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp SET row_security = on AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM canteiro.contract_history h
        WHERE h.tenant_id = p_tenant AND h.event_id = p_event
          AND h.actor_id = NULLIF(current_setting('canteiro.actor_id', true), '')::UUID
          AND h.action = 'contract_reference_renamed' AND h.origin = 'web'
    ) THEN
        RAISE EXCEPTION USING ERRCODE = '42501', MESSAGE = 'Evento indisponível para notificação.';
    END IF;
    INSERT INTO canteiro_meta.contract_notification_outbox(tenant_id, event_id)
    VALUES (p_tenant, p_event) ON CONFLICT (tenant_id, event_id) DO NOTHING;
END;
$$;
REVOKE ALL ON FUNCTION canteiro_meta.enqueue_contract_notification(UUID, UUID) FROM PUBLIC;

-- Cada chamada confirma uma única entrega interna. O contexto do evento é
-- restaurado no sucesso e revertido pela subtransação se a entrega falhar.
CREATE FUNCTION canteiro_meta.consume_contract_notification()
    RETURNS TEXT LANGUAGE plpgsql SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp SET row_security = on AS $$
DECLARE
    pending RECORD;
    recipient UUID;
    delivery_time TIMESTAMPTZ;
    caller_tenant TEXT := COALESCE(current_setting('canteiro.tenant_id', true), '');
    caller_actor TEXT := COALESCE(current_setting('canteiro.actor_id', true), '');
BEGIN
    SELECT q.tenant_id, q.event_id INTO pending
    FROM canteiro_meta.contract_notification_outbox q
    WHERE q.delivered_at IS NULL AND q.next_attempt_at <= statement_timestamp()
    ORDER BY q.next_attempt_at, q.queued_at, q.tenant_id, q.event_id
    LIMIT 1 FOR UPDATE SKIP LOCKED;
    IF NOT FOUND THEN
        RETURN 'idle';
    END IF;

    BEGIN
        PERFORM set_config('canteiro.tenant_id', pending.tenant_id::TEXT, true);
        SELECT h.actor_id INTO STRICT recipient FROM canteiro.contract_history h
        WHERE h.tenant_id = pending.tenant_id AND h.event_id = pending.event_id;
        PERFORM set_config('canteiro.actor_id', recipient::TEXT, true);
        delivery_time := clock_timestamp();
        INSERT INTO canteiro.contract_notifications(tenant_id, event_id, delivered_at)
        VALUES (pending.tenant_id, pending.event_id, delivery_time)
        ON CONFLICT (tenant_id, event_id) DO NOTHING;
        UPDATE canteiro_meta.contract_notification_outbox q
        SET delivered_at = delivery_time, attempts = q.attempts + 1
        WHERE q.tenant_id = pending.tenant_id AND q.event_id = pending.event_id;
        PERFORM set_config('canteiro.tenant_id', caller_tenant, true);
        PERFORM set_config('canteiro.actor_id', caller_actor, true);
        RETURN 'delivered';
    EXCEPTION WHEN OTHERS THEN
        -- A subtransação desfaz a notificação e a confirmação antes de adiar o item.
        UPDATE canteiro_meta.contract_notification_outbox q
        SET next_attempt_at = clock_timestamp()
                + LEAST(60, 1 << LEAST(q.attempts, 6)::INTEGER) * INTERVAL '1 second',
            attempts = q.attempts + 1
        WHERE q.tenant_id = pending.tenant_id AND q.event_id = pending.event_id;
        RETURN 'deferred';
    END;
END;
$$;
REVOKE ALL ON FUNCTION canteiro_meta.consume_contract_notification() FROM PUBLIC;
