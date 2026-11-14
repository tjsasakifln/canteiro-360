# Validação da issue #8

## Objeto

Papéis, vínculos temporais, limites por ato e valor, políticas, delegações, substituições e continuidade de responsabilidades. Escopos exatos de obra, contrato e orçamento individual, com orçamento ligado a contrato e contrato ligado a obra. Interface `/permissoes`, API autenticada e PostgreSQL.

Base: PR #79 integrada em `c623a8d1c58de52e89f19c39fcb4eba83a4b084d`. A revisão independente confirmou a [CI final 43429818330](https://github.com/tjsasakifln/canteiro-360/actions/runs/43429818330): 25 etapas e 188 testes aprovados na árvore exata do head `fc0912810a4a6f7c83f59e1de07345ede18bb098`.

## Critérios observáveis

| Critério | Verificação implementada |
|---|---|
| Leitor não altera diretamente; fiscal de A não opera B | Domínio, serviço transacional e API, incluindo empresa e escopo incorretos |
| Conferência de orçamento tem concessão própria | Vínculo com consulta e conferência permite conferir, mas recusa edição e aprovação; retirar o ato de conferência também impede conferir |
| Externo opera apenas o orçamento concedido | Dois orçamentos do mesmo contrato, vínculo exato em A, recusa de B e ausência de herança do contrato; vencimento ou revogação bloqueia a sessão existente |
| Proponente não se aprova quando segregado | Múltiplos papéis, conflito declarado, delegado do proponente e decisão independente |
| Delegado não amplia direitos nem permanece vencido | Ato, valor, escopo, datas, prazo da participação na empresa, revogação da fonte e saída do titular |
| Delegação e substituição preservam justificativa e autoria | Motivo obrigatório, conservação no vínculo e na auditoria, identidade própria do substituto e histórico preservado após saída do titular |
| Acréscimo +100 e redução -100 não viram redução pura | Valores separados, alçada 100, classificação com acréscimo e limiar inclusivo |

Também se verificam último responsável, sucessor suficiente, concorrência, versões antigas, política alterada, histórico imutável, reutilização de conexão e ausência de dados de outra área.

## Método

Domínio, testes de banco e interface foram preparados em frentes separadas, com revisão independente de segurança e integração. Testes utilizam exclusivamente dados fictícios. PostgreSQL local 17.11; a configuração de CI usa PostgreSQL 17.7 e as versões fixadas no repositório.

O Chrome completo não inicia neste executor por restrição de socket local. O percurso local foi executado no Chrome Headless Shell 156.0.8078.80. Essa limitação não dispensa o Chrome integral na CI nem autoriza fabricar eventos de ativação de abas. A revisão principal e a revisão independente abriram e conferiram as quatro capturas locais.

## Resultados locais comprovados

Os resultados abaixo identificam rodadas efetivamente executadas. A confirmação da árvore final depende do commit registrado na PR e de sua execução de CI.

| Verificação | Resultado observado | Cobertura ou limite da evidência |
|---|---|---|
| Domínio de acesso | 22 testes aprovados | Inclui escopo individual de orçamento, conferência separada, valores sem compensação líquida, delegação e independência das aprovações |
| Aplicação com PostgreSQL | 26 testes aprovados no conjunto completo, incluindo os 6 novos percursos | Permissões, empresa e escopo exatos, decisões concorrentes, revisões, motivo da delegação, continuidade de responsabilidade, orçamento externo com prazo e percursos anteriores de organizações |
| Persistência com PostgreSQL | 32 testes aprovados no conjunto completo, incluindo os 9 novos cenários | Isolamento, histórico imutável, motivo da delegação, concorrência em READ COMMITTED e REPEATABLE READ e recusa de histórico de migração futuro |
| API HTTP | 8 testes aprovados após a migração final, em 87,00 segundos, incluindo os 2 novos percursos | Sessão, CSRF, origem, identidade, empresa, versões e direitos explícitos |
| Limite de expiração | 1 teste aprovado | O vencimento da participação continua obrigatório entre a leitura do banco e a autorização, inclusive para a fonte delegante |
| Interface | 11 testes aprovados | Formatação, identidade, empresa, preservação compatível de campos, troca de formulário, desmontagem e retorno de rota, renovação de contexto e início sugerido dentro da validade da fonte |
| Clippy | Aprovado para SSR, WASM e e2e | Verificação estática; não substitui a execução dos percursos no navegador |
| Conjunto nativo do projeto | 163 testes aprovados, zero falhas | Os 77 cenários que exigem PostgreSQL ou navegador são executados separadamente pela CI |
| Percurso local de permissões no navegador | 1 cenário completo aprovado em 42,61 segundos | Concessões, decisão independente, substituição, teclado, falha de rede, perda de identidade, confirmações de descarte e navegação com rascunho protegido |
| Compilação otimizada e limite de arquivos | Aprovados sem alteração dos limites | [Compilação](issue-008/local/build.json) e [medidas de transferência](issue-008/local/budget.json) |

As fontes dos cenários estão nos testes de [domínio](../../crates/domain/tests/access.rs), [aplicação](../../crates/application/src/auth/organizations/access_tests.rs), [persistência](../../crates/persistence/src/access_database_tests.rs), [HTTP](../../crates/api/tests/auth/access.rs), [expiração](../../crates/application/src/auth/organizations/access.rs) e interface, nos módulos de [formatação](../../crates/web/src/access/format.rs) e [estado](../../crates/web/src/access/model.rs).

O [manifesto local](issue-008/local/manifest.json) identifica a fonte do produto, o blob do teste de navegador, o ambiente e os hashes dos arquivos. Os logs preservam os resultados de [testes nativos](issue-008/local/native-tests.log), [persistência](issue-008/local/persistence.log), [aplicação](issue-008/local/application.log), [HTTP](issue-008/local/http.log) e [navegador](issue-008/local/permissions-browser.log). O [relatório do navegador](issue-008/local/permissions-interface.json) registra os percursos, as larguras e os rótulos.

| Perfil | 360 px | 1280 px |
|---|---|---|
| Administrador | [Captura](issue-008/local/permissions-administrator-360.png) | [Captura](issue-008/local/permissions-administrator-1280.png) |
| Leitor | [Captura](issue-008/local/permissions-reader-360.png) | [Captura](issue-008/local/permissions-reader-1280.png) |

As capturas apresentam os painéis administrativos abertos. O posicionamento no topo usa entrada real de teclado antes da imagem longa, sem ocultar componentes. Foram conferidos 24 de 24 campos rotulados no administrador e 9 de 9 no leitor, sem rolagem horizontal ou colisão de componentes.

No ponto de controle desta documentação, a execução integral da CI da issue #8 ainda está pendente. O percurso local e a inspeção visual aprovados não antecipam esse resultado. As conclusões posteriores ficam registradas na PR e nas execuções descritas abaixo.

## Evidência da árvore final

A [execução 43554310796](https://github.com/tjsasakifln/canteiro-360/actions/runs/43554310796) aprovou os 159 testes nativos e 31 testes de persistência, mas encontrou uma fixture antiga que usava a versão fixa 5 como migração futura. A migração 0005 desta etapa tornou esse número existente. A fixture foi corrigida para usar a maior versão de `MIGRATOR` mais um, mantendo as verificações de recusa de conexão, recusa de migração e preservação dos dados. O reteste completo de persistência aprovou os 32 cenários em 6,63 segundos. As etapas posteriores não foram executadas naquela rodada de CI.

A revisão de experiência também identificou perdas de rascunho ao escolher outra regra, fonte de delegação ou pedido para revisão e ao navegar entre páginas. A correção aplica a proteção exigida no item 8 da seção 5 de [experiência](../experiencia.md), com restauração condicionada à identidade, à empresa e aos direitos atuais.

A PR desta implementação e as execuções associadas no GitHub Actions preservam o commit exato, a configuração e o resultado de cada rodada. O resultado final deve ser conferido pelo commit de origem da execução, sem atribuir a uma revisão posterior o sucesso de uma árvore anterior.

Os registros da PR e do GitHub Actions são a referência para a conferência visual e os resultados obtidos depois deste ponto de controle local. Este documento não antecipa aprovação dessas etapas. Mudanças de comportamento, cobertura ou limites identificadas posteriormente devem ser refletidas nesta documentação.

## Limites

Referências de obra, contrato e orçamento individual organizam direitos; não formalizam instrumentos nem executam medições ou pagamentos. Os módulos consumidores devem conferir seus objetos reais e autorizar a escrita na mesma transação. A proteção das futuras operações de orçamento e exportação depende dessa integração, inclusive em links antigos após vencimento ou revogação.

A autorização anterior para integração técnica foi preservada. O teste com três profissionais da issue #3 e a demonstração humana com gerenciador de senhas da issue #6 continuam abertos. Esta etapa não declara operação comercial nem publicação em produção.
