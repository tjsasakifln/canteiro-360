-- Issue #15: resultado inconclusivo por reserva, inclusive na última tentativa.
ALTER TABLE canteiro_auth.mail_outbox
    ADD COLUMN unconfirmed_attempt INTEGER NOT NULL DEFAULT 0
        CHECK (unconfirmed_attempt >= 0 AND unconfirmed_attempt <= attempts);

CREATE OR REPLACE FUNCTION canteiro_meta.record_invitation_mail_history()
RETURNS TRIGGER LANGUAGE plpgsql SECURITY DEFINER
SET search_path = pg_catalog, pg_temp AS $$
BEGIN
    IF NEW.attempts > OLD.attempts THEN
        IF OLD.attempts > 0 AND OLD.delivered_at IS NULL AND OLD.discarded_at IS NULL THEN
            INSERT INTO canteiro_meta.invitation_mail_history
                (mail_id, tenant_id, invitation_id, attempt, event)
            VALUES (OLD.id, OLD.invitation_tenant_id, OLD.invitation_id,
                    OLD.attempts, 'unconfirmed') ON CONFLICT DO NOTHING;
        END IF;
        INSERT INTO canteiro_meta.invitation_mail_history
            (mail_id, tenant_id, invitation_id, attempt, event)
        VALUES (NEW.id, NEW.invitation_tenant_id, NEW.invitation_id,
                NEW.attempts, 'claimed');
    END IF;

    -- O retorno negativo do transporte não prova rejeição definitiva pelo provedor.
    -- Registre a tentativa sem confirmação imediatamente, sem esperar outra reserva.
    IF NEW.unconfirmed_attempt > OLD.unconfirmed_attempt THEN
        INSERT INTO canteiro_meta.invitation_mail_history
            (mail_id, tenant_id, invitation_id, attempt, event)
        VALUES (NEW.id, NEW.invitation_tenant_id, NEW.invitation_id,
                NEW.unconfirmed_attempt, 'unconfirmed') ON CONFLICT DO NOTHING;
    END IF;

    IF OLD.delivered_at IS NULL AND NEW.delivered_at IS NOT NULL THEN
        INSERT INTO canteiro_meta.invitation_mail_history
            (mail_id, tenant_id, invitation_id, attempt, event)
        VALUES (NEW.id, NEW.invitation_tenant_id, NEW.invitation_id,
                NEW.attempts, 'accepted_by_provider');
    ELSIF OLD.discarded_at IS NULL AND NEW.discarded_at IS NOT NULL THEN
        INSERT INTO canteiro_meta.invitation_mail_history
            (mail_id, tenant_id, invitation_id, attempt, event)
        VALUES (NEW.id, NEW.invitation_tenant_id, NEW.invitation_id,
                NEW.attempts, 'discarded');
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER record_invitation_mail_history ON canteiro_auth.mail_outbox;
CREATE TRIGGER record_invitation_mail_history
AFTER UPDATE OF attempts, delivered_at, discarded_at, unconfirmed_attempt
ON canteiro_auth.mail_outbox
FOR EACH ROW WHEN (NEW.invitation_id IS NOT NULL)
EXECUTE FUNCTION canteiro_meta.record_invitation_mail_history();
