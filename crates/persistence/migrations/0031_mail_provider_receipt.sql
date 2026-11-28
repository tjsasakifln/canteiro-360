-- Issue #15: preservar o identificador opaco de aceitação do provedor.
-- Registros históricos sem recibo continuam históricos; não inventar identificadores.
ALTER TABLE canteiro_auth.mail_outbox
    ADD COLUMN provider_receipt_id TEXT,
    ADD CHECK (provider_receipt_id IS NULL OR
        (delivered_at IS NOT NULL AND octet_length(provider_receipt_id) BETWEEN 1 AND 256
         AND provider_receipt_id COLLATE "C" !~ '[^!-~]'));

CREATE FUNCTION canteiro_auth.guard_mail_provider_receipt()
RETURNS TRIGGER LANGUAGE plpgsql
SET search_path = pg_catalog, pg_temp AS $$
BEGIN
    IF TG_OP = 'UPDATE' THEN
        IF OLD.delivered_at IS NOT NULL THEN
            IF NEW.delivered_at IS DISTINCT FROM OLD.delivered_at OR
               NEW.provider_receipt_id IS DISTINCT FROM OLD.provider_receipt_id THEN
                RAISE EXCEPTION 'Recibo de provedor imutável' USING ERRCODE = '23514';
            END IF;
            RETURN NEW;
        END IF;
    END IF;
    IF NEW.delivered_at IS NOT NULL AND NEW.provider_receipt_id IS NULL THEN
        RAISE EXCEPTION 'Recibo de provedor obrigatório' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;
REVOKE ALL ON FUNCTION canteiro_auth.guard_mail_provider_receipt() FROM PUBLIC;
CREATE TRIGGER guard_mail_provider_receipt
BEFORE INSERT OR UPDATE OF delivered_at, provider_receipt_id ON canteiro_auth.mail_outbox
FOR EACH ROW EXECUTE FUNCTION canteiro_auth.guard_mail_provider_receipt();
