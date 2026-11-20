-- Atos explícitos das etapas 10 a 14. Nenhuma concessão existente é ampliada.
ALTER TABLE canteiro.access_grants DROP CONSTRAINT access_grants_actions_check;
ALTER TABLE canteiro.access_grants ADD CONSTRAINT access_grants_actions_check CHECK (
    cardinality(actions) BETWEEN 1 AND 17 AND actions <@ ARRAY[
        'view','edit_budget','review_budget','record_execution','review_measurement','propose_change',
        'approve_budget','approve_change','authorize_payment','record_payment','technical_accept',
        'edit_contract_record','manage_portfolio','edit_project','approve_project_scope',
        'manage_technical_obligations','approve_technical_exception'
    ]::TEXT[]
    AND (NOT ('edit_contract_record'=ANY(actions)) OR role='manager')
    AND (NOT ('manage_portfolio'=ANY(actions)) OR role='manager')
    AND (NOT ('edit_project'=ANY(actions)) OR role IN ('manager','technical_responsible'))
    AND (NOT ('manage_technical_obligations'=ANY(actions)) OR role IN ('manager','technical_responsible'))
    AND (NOT ('approve_project_scope'=ANY(actions)) OR role='approver')
    AND (NOT ('approve_technical_exception'=ANY(actions)) OR role='approver')
);

ALTER TABLE canteiro.access_policies DROP CONSTRAINT access_policies_action_check;
ALTER TABLE canteiro.access_policies ADD CONSTRAINT access_policies_action_check CHECK (
    action IN ('approve_budget','approve_change','authorize_payment','technical_accept',
               'approve_project_scope','approve_technical_exception')
);
ALTER TABLE canteiro.access_request_versions DROP CONSTRAINT access_request_versions_action_check;
ALTER TABLE canteiro.access_request_versions ADD CONSTRAINT access_request_versions_action_check CHECK (
    action IN ('approve_budget','approve_change','authorize_payment','technical_accept',
               'approve_project_scope','approve_technical_exception')
);
