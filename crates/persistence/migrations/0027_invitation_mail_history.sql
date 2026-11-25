-- Issue #15: trilha administrativa por tentativa, sem destinatário, código ou payload.
-- Não se declara falha do provedor quando apenas não houve confirmação da reserva.
CREATE TABLE canteiro_meta.invitation_mail_history (
    mail_id UUID NOT NULL,
    tenant_id UUID NOT NULL,
    invitation_id UUID NOT NULL,
    attempt INTEGER NOT NULL CHECK (attempt >= 0),
    event TEXT NOT NULL CHECK (event IN
        ('claimed', 'unconfirmed', 'accepted_by_provider', 'discarded')),
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (mail_id, attempt, event),
    CHECK (attempt > 0 OR event = 'discarded')
);
REVOKE ALL ON canteiro_meta.invitation_mail_history FROM PUBLIC;

CREATE FUNCTION canteiro_meta.record_invitation_mail_history()
RETURNS TRIGGER LANGUAGE plpgsql SECURITY DEFINER
SET search_path = pg_catalog, pg_temp AS $$
BEGIN
    IF NEW.attempts > OLD.attempts THEN
        IF OLD.attempts > 0 AND OLD.delivered_at IS NULL AND OLD.discarded_at IS NULL THEN
            INSERT INTO canteiro_meta.invitation_mail_history
                (mail_id, tenant_id, invitation_id, attempt, event)
            VALUES (OLD.id, OLD.invitation_tenant_id, OLD.invitation_id,
                    OLD.attempts, 'unconfirmed');
        END IF;
        INSERT INTO canteiro_meta.invitation_mail_history
            (mail_id, tenant_id, invitation_id, attempt, event)
        VALUES (NEW.id, NEW.invitation_tenant_id, NEW.invitation_id,
                NEW.attempts, 'claimed');
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
REVOKE ALL ON FUNCTION canteiro_meta.record_invitation_mail_history() FROM PUBLIC;

CREATE TRIGGER record_invitation_mail_history
AFTER UPDATE OF attempts, delivered_at, discarded_at ON canteiro_auth.mail_outbox
FOR EACH ROW WHEN (NEW.invitation_id IS NOT NULL)
EXECUTE FUNCTION canteiro_meta.record_invitation_mail_history();
