-- Arquivos completos entram em quarentena; somente versões verificadas são anexos.
CREATE TABLE canteiro.documents (
    tenant_id UUID NOT NULL,
    id UUID NOT NULL,
    scope_id UUID NOT NULL,
    active_version BIGINT NOT NULL CHECK (active_version > 0),
    created_by UUID NOT NULL REFERENCES canteiro_auth.users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (tenant_id,id),
    UNIQUE (tenant_id,id,scope_id),
    FOREIGN KEY (tenant_id,scope_id) REFERENCES canteiro.access_scopes(tenant_id,id)
);

CREATE TABLE canteiro.document_versions (
    tenant_id UUID NOT NULL,
    document_id UUID NOT NULL,
    version BIGINT NOT NULL CHECK (version > 0),
    sha256 TEXT NOT NULL CHECK (sha256 ~ '^[0-9a-f]{64}$'),
    content BYTEA NOT NULL CHECK (octet_length(content) BETWEEN 1 AND 5242880),
    content_type TEXT NOT NULL CHECK (content_type IN ('application/pdf','image/png','text/plain; charset=utf-8','text/csv; charset=utf-8')),
    file_name TEXT NOT NULL CHECK (char_length(file_name) BETWEEN 1 AND 180),
    description TEXT NOT NULL CHECK (char_length(btrim(description)) BETWEEN 2 AND 500),
    category TEXT NOT NULL CHECK (category IN ('technical','contractual','budget','evidence','other')),
    created_by UUID NOT NULL REFERENCES canteiro_auth.users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    scanner TEXT NOT NULL CHECK (scanner='clamav-instream'),
    PRIMARY KEY (tenant_id,document_id,version),
    UNIQUE (tenant_id,document_id,version,sha256),
    FOREIGN KEY (tenant_id,document_id) REFERENCES canteiro.documents(tenant_id,id)
);
ALTER TABLE canteiro.documents ADD CONSTRAINT document_active_version_exists
    FOREIGN KEY (tenant_id,id,active_version) REFERENCES canteiro.document_versions(tenant_id,document_id,version)
    DEFERRABLE INITIALLY DEFERRED;

CREATE TABLE canteiro.document_uploads (
    tenant_id UUID NOT NULL,
    id UUID NOT NULL,
    operation_key UUID NOT NULL,
    scope_id UUID NOT NULL,
    actor_id UUID NOT NULL REFERENCES canteiro_auth.users(id),
    target_document UUID,
    expected_version BIGINT NOT NULL CHECK(expected_version >= 0),
    fingerprint BYTEA NOT NULL CHECK(octet_length(fingerprint)=32),
    sha256 TEXT NOT NULL CHECK(sha256 ~ '^[0-9a-f]{64}$'),
    content BYTEA CHECK(octet_length(content) BETWEEN 1 AND 5242880),
    content_type TEXT NOT NULL,
    file_name TEXT NOT NULL,
    description TEXT NOT NULL,
    category TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'quarantine' CHECK(status IN ('quarantine','accepted','rejected')),
    reason TEXT NOT NULL DEFAULT 'awaiting_scan' CHECK(reason IN ('awaiting_scan','scanner_unavailable','accepted','prohibited','version_conflict')),
    result_document UUID,
    result_version BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY(tenant_id,id),
    UNIQUE(tenant_id,operation_key),
    FOREIGN KEY(tenant_id,scope_id) REFERENCES canteiro.access_scopes(tenant_id,id),
    FOREIGN KEY(tenant_id,target_document,scope_id) REFERENCES canteiro.documents(tenant_id,id,scope_id),
    FOREIGN KEY(tenant_id,result_document,result_version) REFERENCES canteiro.document_versions(tenant_id,document_id,version),
    CHECK((target_document IS NULL AND expected_version=0) OR (target_document IS NOT NULL AND expected_version>0)),
    CHECK((status='quarantine' AND content IS NOT NULL AND result_document IS NULL AND result_version IS NULL)
        OR (status='accepted' AND content IS NULL AND result_document IS NOT NULL AND result_version IS NOT NULL)
        OR (status='rejected' AND content IS NULL AND result_document IS NULL AND result_version IS NULL))
);
CREATE INDEX document_uploads_scope ON canteiro.document_uploads(tenant_id,scope_id,created_at DESC);

CREATE TABLE canteiro.document_signatures (
    tenant_id UUID NOT NULL,
    document_id UUID NOT NULL,
    version BIGINT NOT NULL,
    sha256 TEXT NOT NULL,
    evidence_key BYTEA NOT NULL CHECK(octet_length(evidence_key)=32),
    signer TEXT NOT NULL CHECK(char_length(btrim(signer)) BETWEEN 2 AND 160),
    signed_at TIMESTAMPTZ NOT NULL,
    evidence TEXT NOT NULL CHECK(char_length(btrim(evidence)) BETWEEN 10 AND 500),
    actor_id UUID NOT NULL REFERENCES canteiro_auth.users(id),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY(tenant_id,document_id,version,evidence_key),
    FOREIGN KEY(tenant_id,document_id,version,sha256) REFERENCES canteiro.document_versions(tenant_id,document_id,version,sha256)
);

CREATE TABLE canteiro.document_download_tickets (
    tenant_id UUID NOT NULL,
    token_hash BYTEA NOT NULL CHECK(octet_length(token_hash)=32),
    document_id UUID NOT NULL,
    version BIGINT NOT NULL,
    session_id UUID NOT NULL REFERENCES canteiro_auth.sessions(id),
    actor_id UUID NOT NULL REFERENCES canteiro_auth.users(id),
    expires_at TIMESTAMPTZ NOT NULL,
    preview BOOLEAN NOT NULL DEFAULT false,
    PRIMARY KEY(tenant_id,token_hash),
    FOREIGN KEY(tenant_id,document_id,version) REFERENCES canteiro.document_versions(tenant_id,document_id,version)
);

CREATE TABLE canteiro.document_history (
    tenant_id UUID NOT NULL,
    id UUID NOT NULL,
    document_id UUID NOT NULL,
    version BIGINT NOT NULL,
    previous_version BIGINT,
    sha256 TEXT NOT NULL,
    actor_id UUID NOT NULL REFERENCES canteiro_auth.users(id),
    event TEXT NOT NULL CHECK(event IN ('version_created','signature_recorded')),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY(tenant_id,id),
    FOREIGN KEY(tenant_id,document_id,version,sha256) REFERENCES canteiro.document_versions(tenant_id,document_id,version,sha256),
    FOREIGN KEY(tenant_id,document_id,previous_version) REFERENCES canteiro.document_versions(tenant_id,document_id,version)
);

DO $$ DECLARE table_name TEXT; BEGIN
    FOREACH table_name IN ARRAY ARRAY['documents','document_versions','document_uploads','document_signatures','document_download_tickets','document_history'] LOOP
        EXECUTE format('ALTER TABLE canteiro.%I ENABLE ROW LEVEL SECURITY',table_name);
        EXECUTE format('ALTER TABLE canteiro.%I FORCE ROW LEVEL SECURITY',table_name);
        EXECUTE format('CREATE POLICY tenant_isolation ON canteiro.%I USING (tenant_id=NULLIF(current_setting(''canteiro.tenant_id'',true),'''')::UUID) WITH CHECK (tenant_id=NULLIF(current_setting(''canteiro.tenant_id'',true),'''')::UUID)',table_name);
        EXECUTE format('REVOKE ALL ON canteiro.%I FROM PUBLIC',table_name);
    END LOOP;
    FOREACH table_name IN ARRAY ARRAY['document_versions','document_signatures','document_history'] LOOP
        EXECUTE format('CREATE TRIGGER document_immutable BEFORE UPDATE OR DELETE OR TRUNCATE ON canteiro.%I FOR EACH STATEMENT EXECUTE FUNCTION canteiro.reject_history_rewrite()',table_name);
    END LOOP;
END $$;
