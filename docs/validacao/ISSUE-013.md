# Issue 13: projetos, entregas e responsáveis

## Implementação

O projeto possui um escopo de acesso próprio e pode existir sem obra, contrato ou documento fictício. Seu cadastro mantém disciplina, objetivo, escopo, referência externa, partes interessadas, entregas e marcos. Cada entrega identifica responsável profissional, prazo e critério de aceite. Um profissional não precisa de login para responder por uma entrega; o cadastro não cria uma conta de acesso.

O criador recebe uma concessão explícita para editar seu projeto. Aprovar o escopo exige outro usuário com a permissão específica. O aceite de uma entrega exige a permissão de aceite técnico. A aplicação revalida sessão, empresa ativa, participação e escopo na mesma transação de escrita. Consultas e históricos usam a mesma fronteira.

Cada mudança produz uma versão preservada. Revisar um escopo aprovado abre uma nova versão em elaboração, sem carregar os aceites da versão anterior. O banco recusa atualização, remoção ou truncamento das versões e dos vínculos históricos. O encerramento exige as entregas aceitas. Aceite técnico não gera medição, fatura, pagamento, recebimento ou transferência de direitos.

As páginas `/projetos` usam componentes Leptos renderizados pelo servidor e formulários nativos. O erro mantém os campos, a identificação da tentativa e a versão de partida. Reaplicar após conflito exige um botão expresso. Se a empresa ou a geração do contexto mudou em outra aba, o preenchimento é preservado para consulta, sem formulário de envio que possa transferi-lo silenciosamente para a nova empresa. As respostas são privadas e não armazenáveis em cache. Datas civis são apresentadas em português brasileiro e registros de auditoria no horário de Brasília.

## Critérios e evidências reproduzíveis

| Critério | Cenário |
|---|---|
| Projeto anterior a obra ou contrato | `project_lifecycle_independent_professional_approval_revision_and_acceptance` cria escopo `project` com `work_id` ausente. |
| Aprovação exige responsável e critério | Testes de domínio `independent_project_and_commitment_requirements` e fluxo PostgreSQL recusam o compromisso incompleto. |
| Revisão preserva escopo inicial | O fluxo consulta cinco versões e compara a revisão inicial e o escopo aprovado com seus valores originais. |
| Aceite sem efeito financeiro | O teste confere que o aceite modifica a lista de entregas aceitas, preserva o escopo e não cria pedidos de alçada. Módulos financeiros posteriores permanecem sujeitos à homologação integrada. |
| Concorrência e repetição | `project_concurrency_repetition_cross_scope_and_revocation` verifica duas criações simultâneas com o mesmo recibo, uma única alteração entre versões concorrentes, conteúdo diferente na mesma chave, consulta por membro sem concessão e repetição depois da revogação. |
| Formulário nativo e preservação em erro | `native_project_form_creates_independently_and_preserves_invalid_revision` cria por POST URL-encoded com sessão e CSRF reais; uma revisão inválida preserva os campos e a versão sem gravar a alteração. |
| Revisão independente | A validação integrada do conjunto 10 a 14 registra a revisão e os resultados finais. |

Execução local: `cargo test -p canteiro-domain --test projects_obligations` e `cargo test -p canteiro-application project_records_tests -- --ignored --test-threads=1`, com PostgreSQL isolado e `CANTEIRO_TEST_DATABASE_URL`. A rodada central anterior ao ajuste HTTP, já com as alterações de enquadramento do RRT e revalidação temporal, passou os 47 testes de aplicação com PostgreSQL, incluindo os quatro cenários de `project_records_tests`. A suíte do workspace passou 208 testes, incluindo os dez testes de domínio de `projects_obligations`. Os registros históricos estão em `target/evidence/registry-application.log` e `target/evidence/registry-workspace.log`.

A [execução 44648996986 do GitHub Actions](https://github.com/tjsasakifln/canteiro-360/actions/runs/44648996986), sobre o commit `262acef5409983af4a157ae93a6fc48b77d594b9`, também passou 208 testes gerais, 37 de persistência, 47 de aplicação e 9 de HTTP. No navegador, passaram 11 dos 13 testes. O novo percurso chegou à página de projetos, mas não concluiu a criação pelo formulário. A investigação identificou `Referrer-Policy: no-referrer`, que faz o POST nativo enviar origem nula segundo o Fetch Standard. O HTML agora usa `same-origin`, mantendo a verificação de origem, sessão e CSRF. A outra falha levou ao ajuste do limite de tabulações do teste: a busca parava em 12 passos antes do 14º controle. Os detalhes estão no [relatório integrado](ISSUES-010-014.md).

Após a correção, o novo teste HTTP nativo passou localmente: **1 aprovado, 0 falhas, 6,95 s**. Ele verifica criação, recusa de origem ou CSRF inválidos e preservação da revisão recusada. Os dez testes HTTP previstos para a próxima CI ainda não foram executados como uma rodada completa.

A cota de artefatos do GitHub Actions impediu obter as capturas para revisão. O transporte alternativo de evidências sintéticas está em preparação. A próxima CI e a revisão visual e por teclado após as correções permanecem pendentes; os avisos de manutenção de `chrono` e `chrono-tz` também permanecem como bloqueio de integração.

## Limites de integração

Estas implementações partem da PR 82 e não alteram os requisitos de aceite humano das etapas anteriores. O uso por profissionais reais e a reconciliação com medições e financeiro pertencem às etapas expressamente previstas no plano. A implantação de projeto não exige a criação de contrato fictício nem transfere direitos intelectuais.
