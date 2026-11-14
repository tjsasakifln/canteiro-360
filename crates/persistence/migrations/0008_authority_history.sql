-- O fato econômico desta etapa é a decisão de alçada sobre uma versão.
-- Medição, faturamento e pagamento terão os respectivos fatos em seus módulos.
ALTER TABLE canteiro.access_requests ADD UNIQUE (tenant_id,id,scope_id);
ALTER TABLE canteiro.access_approvals ADD UNIQUE
    (tenant_id,request_id,request_version,actor_id,grant_id,approved);

CREATE TABLE canteiro.authority_history (
    tenant_id UUID NOT NULL,
    event_id UUID NOT NULL,
    scope_id UUID NOT NULL,
    request_id UUID NOT NULL,
    request_version BIGINT NOT NULL CHECK (request_version > 0),
    actor_id UUID NOT NULL REFERENCES canteiro_auth.users(id),
    event_kind TEXT NOT NULL CHECK (event_kind IN ('proposed','revised','decision')),
    before_version BIGINT,
    before_status TEXT,
    before_valid_approvals SMALLINT,
    after_status TEXT NOT NULL CHECK (after_status IN ('pending','approved','rejected')),
    after_valid_approvals SMALLINT NOT NULL CHECK (after_valid_approvals BETWEEN 0 AND 2),
    grant_id UUID,
    approval_value BOOLEAN GENERATED ALWAYS AS (
        CASE WHEN event_kind='decision' THEN after_status<>'rejected' END
    ) STORED,
    origin TEXT NOT NULL DEFAULT 'permissoes' CHECK (origin = 'permissoes'),
    reason TEXT NOT NULL CHECK (reason IN (
        'request_registered','request_revised','approval_recorded','authority_completed','request_rejected'
    )),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (tenant_id,event_id),
    FOREIGN KEY (tenant_id,request_id,scope_id) REFERENCES canteiro.access_requests(tenant_id,id,scope_id),
    FOREIGN KEY (tenant_id,request_id,request_version) REFERENCES canteiro.access_request_versions(tenant_id,request_id,version),
    FOREIGN KEY (tenant_id,request_id,before_version) REFERENCES canteiro.access_request_versions(tenant_id,request_id,version),
    FOREIGN KEY (tenant_id,request_id,request_version,actor_id,grant_id,approval_value)
        REFERENCES canteiro.access_approvals(tenant_id,request_id,request_version,actor_id,grant_id,approved),
    CHECK (
        (event_kind='proposed' AND request_version=1 AND before_version IS NULL
          AND before_status IS NULL AND before_valid_approvals IS NULL AND grant_id IS NULL
          AND after_status='pending' AND after_valid_approvals=0 AND reason='request_registered')
        OR
        (event_kind='revised' AND before_version IS NOT NULL AND before_version=request_version-1
          AND before_status IS NOT DISTINCT FROM 'pending' AND before_valid_approvals IS NOT NULL
          AND before_valid_approvals BETWEEN 0 AND 2 AND grant_id IS NULL
          AND after_status='pending' AND after_valid_approvals=0 AND reason='request_revised')
        OR
        (event_kind='decision' AND before_version IS NOT DISTINCT FROM request_version
          AND before_status IS NOT DISTINCT FROM 'pending' AND before_valid_approvals IS NOT NULL
          AND before_valid_approvals BETWEEN 0 AND 2 AND grant_id IS NOT NULL
          AND (after_status<>'approved' OR after_valid_approvals>=1)
          AND reason=CASE after_status WHEN 'pending' THEN 'approval_recorded'
            WHEN 'approved' THEN 'authority_completed' ELSE 'request_rejected' END)
    )
);

CREATE UNIQUE INDEX authority_history_revision_once ON canteiro.authority_history
    (tenant_id,request_id,request_version) WHERE event_kind IN ('proposed','revised');
CREATE UNIQUE INDEX authority_history_actor_decision_once ON canteiro.authority_history
    (tenant_id,request_id,request_version,actor_id) WHERE event_kind='decision';
CREATE UNIQUE INDEX authority_effect_once ON canteiro.authority_history
    (tenant_id,request_id,request_version) WHERE event_kind='decision' AND after_status='approved';
CREATE INDEX authority_history_request ON canteiro.authority_history
    (tenant_id,request_id,request_version,recorded_at,event_id);

ALTER TABLE canteiro.authority_history ENABLE ROW LEVEL SECURITY;
ALTER TABLE canteiro.authority_history FORCE ROW LEVEL SECURITY;
CREATE POLICY authority_history_tenant ON canteiro.authority_history
    USING (tenant_id=NULLIF(current_setting('canteiro.tenant_id',true),'')::UUID)
    WITH CHECK (tenant_id=NULLIF(current_setting('canteiro.tenant_id',true),'')::UUID
        AND actor_id=NULLIF(current_setting('canteiro.actor_id',true),'')::UUID);
REVOKE ALL ON canteiro.authority_history FROM PUBLIC;
CREATE TRIGGER authority_history_immutable
    BEFORE UPDATE OR DELETE OR TRUNCATE ON canteiro.authority_history
    FOR EACH STATEMENT EXECUTE FUNCTION canteiro.reject_history_rewrite();
