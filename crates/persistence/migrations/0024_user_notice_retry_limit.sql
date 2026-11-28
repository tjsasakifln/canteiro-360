-- Issue #15: limite operacional verificável para falhas persistentes da caixa interna.
-- Após dez tentativas reais, não continuar tentando indefinidamente o mesmo
-- evento. O histórico de falhas da migração 0023 permanece imutável.
-- A nova função é SECURITY DEFINER porque a fila global não é acessível à API.
CREATE FUNCTION canteiro_meta.stop_exhausted_user_notice()
RETURNS TRIGGER LANGUAGE plpgsql SECURITY DEFINER
SET search_path = pg_catalog, pg_temp
SET row_security = on AS $$
DECLARE
    previous_tenant TEXT := COALESCE(current_setting('canteiro.tenant_id', true), '');
    previous_actor TEXT := COALESCE(current_setting('canteiro.actor_id', true), '');
BEGIN
    PERFORM set_config('canteiro.tenant_id', NEW.tenant_id::TEXT, true);
    PERFORM set_config('canteiro.actor_id', NEW.recipient_id::TEXT, true);

    UPDATE canteiro.user_notice_outbox
       SET state='failed', claimed_until=NULL
     WHERE tenant_id=NEW.tenant_id AND recipient_id=NEW.recipient_id
       AND event_id=NEW.event_id AND kind=NEW.kind AND channel=NEW.channel
       AND state <> 'delivered';

    UPDATE canteiro.user_notices
       SET state='failed'
     WHERE tenant_id=NEW.tenant_id AND recipient_id=NEW.recipient_id
       AND event_id=NEW.event_id AND kind=NEW.kind
       AND state <> 'resolved';

    UPDATE canteiro_meta.user_notice_queue
       SET processed_at=clock_timestamp()
     WHERE tenant_id=NEW.tenant_id AND recipient_id=NEW.recipient_id
       AND event_id=NEW.event_id AND kind=NEW.kind AND channel=NEW.channel
       AND processed_at IS NULL;

    PERFORM set_config('canteiro.tenant_id', previous_tenant, true);
    PERFORM set_config('canteiro.actor_id', previous_actor, true);
    RETURN NEW;
END;
$$;
REVOKE ALL ON FUNCTION canteiro_meta.stop_exhausted_user_notice() FROM PUBLIC;

-- Os gatilhos AFTER desta tabela rodam em ordem alfabética. Primeiro
-- audit_user_notice_retry registra a tentativa; depois este gatilho
-- fecha a fila. A atualização de processed_at não altera attempts
-- e, portanto, não dispara novamente os gatilhos de contagem.
CREATE TRIGGER stop_exhausted_user_notice
AFTER UPDATE OF attempts ON canteiro_meta.user_notice_queue
FOR EACH ROW
WHEN (NEW.attempts >= 10 AND NEW.attempts > OLD.attempts)
EXECUTE FUNCTION canteiro_meta.stop_exhausted_user_notice();
