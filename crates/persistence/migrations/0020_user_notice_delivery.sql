-- Issue #15: registrar e entregar a comunicação INTERNA sem fingir envio de e-mail.
-- A fila global guarda apenas chaves opacas. O destinatário volta ao contexto RLS
-- antes de qualquer escrita na caixa. A entrega e seu recibo são atômicos.
CREATE TABLE canteiro_meta.user_notice_queue (
    tenant_id UUID NOT NULL,
    recipient_id UUID NOT NULL,
    event_id UUID NOT NULL,
    kind TEXT NOT NULL,
    channel TEXT NOT NULL CHECK (channel = 'internal'),
    ready_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    processed_at TIMESTAMPTZ,
    PRIMARY KEY (tenant_id, recipient_id, event_id, kind, channel),
    FOREIGN KEY (tenant_id, recipient_id, event_id, kind, channel)
        REFERENCES canteiro.user_notice_outbox(tenant_id, recipient_id, event_id, kind, channel)
);
CREATE INDEX user_notice_queue_ready
    ON canteiro_meta.user_notice_queue(ready_at, tenant_id, event_id)
    WHERE processed_at IS NULL;
REVOKE ALL ON canteiro_meta.user_notice_queue FROM PUBLIC;

CREATE FUNCTION canteiro_meta.enqueue_user_notice()
RETURNS TRIGGER LANGUAGE plpgsql SECURITY DEFINER
SET search_path = pg_catalog, pg_temp SET row_security = on AS $$
BEGIN
    IF NEW.channel = 'internal' THEN
        INSERT INTO canteiro_meta.user_notice_queue
            (tenant_id, recipient_id, event_id, kind, channel)
        VALUES (NEW.tenant_id, NEW.recipient_id, NEW.event_id, NEW.kind, NEW.channel)
        ON CONFLICT DO NOTHING;
    END IF;
    RETURN NEW;
END;
$$;
REVOKE ALL ON FUNCTION canteiro_meta.enqueue_user_notice() FROM PUBLIC;
CREATE TRIGGER queue_user_notice_after_insert
AFTER INSERT ON canteiro.user_notice_outbox
FOR EACH ROW EXECUTE FUNCTION canteiro_meta.enqueue_user_notice();

-- A implantação preserva avisos já criados, sem reeditar o evento de origem.
INSERT INTO canteiro_meta.user_notice_queue
    (tenant_id, recipient_id, event_id, kind, channel)
SELECT tenant_id, recipient_id, event_id, kind, channel
FROM canteiro.user_notice_outbox
WHERE channel = 'internal' AND state <> 'delivered'
ON CONFLICT DO NOTHING;

CREATE FUNCTION canteiro_meta.consume_user_notice()
RETURNS TEXT LANGUAGE plpgsql SECURITY DEFINER
SET search_path = pg_catalog, pg_temp SET row_security = on AS $$
DECLARE
    pending RECORD;
    previous_tenant TEXT := COALESCE(current_setting('canteiro.tenant_id', true), '');
    previous_actor TEXT := COALESCE(current_setting('canteiro.actor_id', true), '');
    current_state TEXT;
    attempt_number INTEGER;
    acknowledged_at TIMESTAMPTZ;
BEGIN
    SELECT q.tenant_id, q.recipient_id, q.event_id, q.kind, q.channel
      INTO pending
      FROM canteiro_meta.user_notice_queue q
     WHERE q.processed_at IS NULL AND q.ready_at <= statement_timestamp()
     ORDER BY q.ready_at, q.tenant_id, q.event_id
     LIMIT 1 FOR UPDATE SKIP LOCKED;
    IF NOT FOUND THEN
        RETURN 'idle';
    END IF;

    BEGIN
        PERFORM set_config('canteiro.tenant_id', pending.tenant_id::TEXT, true);
        PERFORM set_config('canteiro.actor_id', pending.recipient_id::TEXT, true);
        SELECT o.state INTO STRICT current_state
          FROM canteiro.user_notice_outbox o
         WHERE o.tenant_id=pending.tenant_id AND o.recipient_id=pending.recipient_id
           AND o.event_id=pending.event_id AND o.kind=pending.kind
           AND o.channel=pending.channel FOR UPDATE;

        IF current_state <> 'delivered' THEN
            acknowledged_at := clock_timestamp();
            UPDATE canteiro.user_notice_outbox o
               SET state='delivered', attempts=attempts+1,
                   delivered_at=acknowledged_at, claimed_until=NULL,
                   provider_receipt='internal'
             WHERE o.tenant_id=pending.tenant_id AND o.recipient_id=pending.recipient_id
               AND o.event_id=pending.event_id AND o.kind=pending.kind
               AND o.channel=pending.channel
             RETURNING o.attempts INTO STRICT attempt_number;

            UPDATE canteiro.user_notices n
               SET state=CASE WHEN n.state='resolved' THEN 'resolved' ELSE 'communicated' END,
                   attempts=attempts+1, last_attempt=acknowledged_at
             WHERE n.tenant_id=pending.tenant_id AND n.recipient_id=pending.recipient_id
               AND n.event_id=pending.event_id AND n.kind=pending.kind;

            INSERT INTO canteiro.user_notice_delivery_history
                (tenant_id,id,recipient_id,event_id,kind,channel,attempt,outcome,occurred_at)
            VALUES (pending.tenant_id,gen_random_uuid(),pending.recipient_id,pending.event_id,
                    pending.kind,pending.channel,attempt_number,'acknowledged',acknowledged_at);
        END IF;

        UPDATE canteiro_meta.user_notice_queue q
           SET processed_at=clock_timestamp()
         WHERE q.tenant_id=pending.tenant_id AND q.recipient_id=pending.recipient_id
           AND q.event_id=pending.event_id AND q.kind=pending.kind AND q.channel=pending.channel;
        PERFORM set_config('canteiro.tenant_id', previous_tenant, true);
        PERFORM set_config('canteiro.actor_id', previous_actor, true);
        RETURN 'delivered';
    EXCEPTION WHEN OTHERS THEN
        -- A subtransação desfez todas as alterações, inclusive o recibo.
        -- Apenas a fila global recebe adiamento, sem registrar entrega fictícia.
        UPDATE canteiro_meta.user_notice_queue q
           SET attempts=q.attempts+1,
               ready_at=clock_timestamp()
                   + LEAST(300, 1 << LEAST(q.attempts, 8)) * INTERVAL '1 second'
         WHERE q.tenant_id=pending.tenant_id AND q.recipient_id=pending.recipient_id
           AND q.event_id=pending.event_id AND q.kind=pending.kind AND q.channel=pending.channel;
        RETURN 'deferred';
    END;
END;
$$;
REVOKE ALL ON FUNCTION canteiro_meta.consume_user_notice() FROM PUBLIC;
