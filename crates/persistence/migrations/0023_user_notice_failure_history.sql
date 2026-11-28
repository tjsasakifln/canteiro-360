-- Issue #15: an unsuccessful delivery attempt must be auditable without
-- confusing it with a provider acknowledgment. This is a new migration,
-- preserving the checksums and applied history of earlier migrations.
--
-- The consumer increments this global queue counter only after the delivery
-- subtransaction has rolled back. Recording the failure in an AFTER trigger
-- makes the retry counter and the immutable failure record one transaction.
CREATE FUNCTION canteiro_meta.record_user_notice_failure()
RETURNS TRIGGER LANGUAGE plpgsql SECURITY DEFINER
SET search_path = pg_catalog, pg_temp
SET row_security = on AS $$
DECLARE
    previous_tenant TEXT := COALESCE(current_setting('canteiro.tenant_id', true), '');
    previous_actor TEXT := COALESCE(current_setting('canteiro.actor_id', true), '');
BEGIN
    IF NEW.attempts <= OLD.attempts THEN
        RETURN NEW;
    END IF;

    PERFORM set_config('canteiro.tenant_id', NEW.tenant_id::TEXT, true);
    PERFORM set_config('canteiro.actor_id', NEW.recipient_id::TEXT, true);
    -- Failed attempts count for the recipient and for the outbox as well;
    -- otherwise a later success would reuse attempt 1 and conceal the retry.
    UPDATE canteiro.user_notice_outbox
       SET attempts = GREATEST(attempts + 1, NEW.attempts), state = 'queued', claimed_until = NULL
     WHERE tenant_id = NEW.tenant_id AND recipient_id = NEW.recipient_id
       AND event_id = NEW.event_id AND kind = NEW.kind AND channel = NEW.channel;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'Não foi possível contabilizar a tentativa de comunicação';
    END IF;
    UPDATE canteiro.user_notices
       SET attempts = GREATEST(attempts + 1, NEW.attempts), last_attempt = clock_timestamp()
     WHERE tenant_id = NEW.tenant_id AND recipient_id = NEW.recipient_id
       AND event_id = NEW.event_id AND kind = NEW.kind;

    INSERT INTO canteiro.user_notice_delivery_history
        (tenant_id, id, recipient_id, event_id, kind, channel, attempt, outcome)
    VALUES (NEW.tenant_id, gen_random_uuid(), NEW.recipient_id, NEW.event_id,
            NEW.kind, NEW.channel, NEW.attempts, 'failed');

    PERFORM set_config('canteiro.tenant_id', previous_tenant, true);
    PERFORM set_config('canteiro.actor_id', previous_actor, true);
    RETURN NEW;
END;
$$;
REVOKE ALL ON FUNCTION canteiro_meta.record_user_notice_failure() FROM PUBLIC;

-- An employee may inspect only their own attempt history, not the delivery
-- receipts of every other person in the same organization.
CREATE POLICY own_user_notice_delivery_history
ON canteiro.user_notice_delivery_history AS RESTRICTIVE FOR SELECT
USING (
    recipient_id = NULLIF(current_setting('canteiro.actor_id', true), '')::UUID
);

CREATE TRIGGER audit_user_notice_retry
AFTER UPDATE OF attempts ON canteiro_meta.user_notice_queue
FOR EACH ROW
WHEN (NEW.attempts > OLD.attempts)
EXECUTE FUNCTION canteiro_meta.record_user_notice_failure();
