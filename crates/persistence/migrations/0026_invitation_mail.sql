-- Issue #15: convites compartilham a outbox criptografada e a entrega idempotente
-- da autenticação, sem exigir um vínculo prévio com a organização.
ALTER TABLE canteiro_auth.mail_outbox
    ALTER COLUMN token_id DROP NOT NULL,
    ADD COLUMN invitation_tenant_id UUID,
    ADD COLUMN invitation_id UUID,
    ADD COLUMN invitation_expires_at TIMESTAMPTZ,
    ADD FOREIGN KEY (invitation_tenant_id, invitation_id)
        REFERENCES canteiro.organization_invites(tenant_id,id),
    ADD UNIQUE (invitation_tenant_id, invitation_id),
    ADD CHECK ((token_id IS NOT NULL AND invitation_tenant_id IS NULL
        AND invitation_id IS NULL AND invitation_expires_at IS NULL)
        OR (token_id IS NULL AND invitation_tenant_id IS NOT NULL
        AND invitation_id IS NOT NULL AND invitation_expires_at IS NOT NULL));

-- Revogar, substituir ou aceitar cancela uma entrega ainda não confirmada.
-- Um código eventualmente recebido após revogação continua inválido no servidor.
CREATE FUNCTION canteiro.cancel_pending_invitation_mail()
RETURNS TRIGGER LANGUAGE plpgsql
SET search_path = pg_catalog, pg_temp AS $$
BEGIN
    UPDATE canteiro_auth.mail_outbox
       SET discarded_at=clock_timestamp(), payload=pg_catalog.decode('', 'hex')
     WHERE invitation_tenant_id=NEW.tenant_id AND invitation_id=NEW.id
       AND delivered_at IS NULL AND discarded_at IS NULL;
    RETURN NEW;
END;
$$;
REVOKE ALL ON FUNCTION canteiro.cancel_pending_invitation_mail() FROM PUBLIC;
CREATE TRIGGER cancel_pending_invitation_mail
AFTER UPDATE OF revoked_at, accepted_at ON canteiro.organization_invites
FOR EACH ROW WHEN (NEW.revoked_at IS NOT NULL OR NEW.accepted_at IS NOT NULL)
EXECUTE FUNCTION canteiro.cancel_pending_invitation_mail();
