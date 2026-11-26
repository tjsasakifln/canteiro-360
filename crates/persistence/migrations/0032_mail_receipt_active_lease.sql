-- Issue #15: uma aceitação do provedor exige uma reserva válida e não encerrada.
-- Preserva as migrações anteriores e os recibos históricos.
CREATE OR REPLACE FUNCTION canteiro_auth.guard_mail_provider_receipt()
RETURNS TRIGGER LANGUAGE plpgsql
SET search_path = pg_catalog, pg_temp AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        IF NEW.delivered_at IS NOT NULL OR NEW.provider_receipt_id IS NOT NULL THEN
            RAISE EXCEPTION 'Entrega exige reserva prévia' USING ERRCODE = '23514';
        END IF;
        RETURN NEW;
    END IF;
    IF OLD.delivered_at IS NOT NULL THEN
        IF NEW.delivered_at IS DISTINCT FROM OLD.delivered_at OR
           NEW.provider_receipt_id IS DISTINCT FROM OLD.provider_receipt_id THEN
            RAISE EXCEPTION 'Recibo de provedor imutável' USING ERRCODE = '23514';
        END IF;
        RETURN NEW;
    END IF;
    IF NEW.delivered_at IS NOT NULL THEN
        IF NEW.provider_receipt_id IS NULL THEN
            RAISE EXCEPTION 'Recibo de provedor obrigatório' USING ERRCODE = '23514';
        END IF;
        IF OLD.attempts <= 0 OR OLD.lease_id IS NULL OR OLD.lease_until IS NULL
           OR OLD.lease_until <= clock_timestamp()
           OR OLD.discarded_at IS NOT NULL OR NEW.discarded_at IS NOT NULL
           OR OLD.unconfirmed_attempt >= OLD.attempts
           OR NEW.attempts IS DISTINCT FROM OLD.attempts
           OR NEW.unconfirmed_attempt IS DISTINCT FROM OLD.unconfirmed_attempt
           OR NEW.lease_id IS DISTINCT FROM OLD.lease_id
           OR NEW.lease_until IS DISTINCT FROM OLD.lease_until
           OR octet_length(NEW.payload) <> 0 THEN
            RAISE EXCEPTION 'Entrega exige reserva ativa e encerramento do payload'
                USING ERRCODE = '23514';
        END IF;
    END IF;
    RETURN NEW;
END;
$$;
