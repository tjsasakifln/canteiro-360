# Issue 13: projetos, entregas e responsáveis

## Implementação

O projeto possui um escopo de acesso próprio e pode existir sem obra, contrato ou documento fictício. Seu cadastro mantém disciplina, objetivo, escopo, referência externa, partes interessadas, entregas e marcos. A aprovação exige responsável profissional, prazo e critério de aceite em cada entrega. Um profissional não precisa de login para responder por uma entrega; o cadastro não cria uma conta de acesso.

O criador recebe uma concessão explícita para editar seu projeto. Aprovar o escopo exige outro usuário com a permissão específica. O aceite de uma entrega exige a permissão de aceite técnico. A aplicação revalida sessão, empresa ativa, participação e escopo na mesma transação de escrita. Consultas e históricos usam a mesma fronteira.

Cada mudança produz uma versão preservada. Revisar um escopo aprovado abre uma nova versão em elaboração, sem carregar os aceites da versão anterior. O banco recusa atualização, remoção ou truncamento das versões e dos vínculos históricos. O encerramento exige as entregas aceitas. Aceite técnico não gera medição, fatura, pagamento, recebimento ou transferência de direitos.

As páginas `/projetos` usam componentes Leptos renderizados pelo servidor e formulários nativos. O erro mantém os campos, a identificação da tentativa e a versão de partida. Reaplicar após conflito exige um botão expresso. Se a empresa ou a geração do contexto mudou em outra aba, o preenchimento é preservado para consulta, sem formulário de envio que possa transferi-lo silenciosamente para a nova empresa. As respostas são privadas e não armazenáveis em cache. Datas civis nos resumos são apresentadas em português brasileiro e registros de auditoria no horário de Brasília. Os campos nativos de data seguem a configuração de idioma do navegador.

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

### Histórico local e CI 44648996986

Execução local: `cargo test -p canteiro-domain --test projects_obligations` e `cargo test -p canteiro-application project_records_tests -- --ignored --test-threads=1`, com PostgreSQL isolado e `CANTEIRO_TEST_DATABASE_URL`. A rodada central anterior ao ajuste HTTP, já com as alterações de enquadramento do RRT e revalidação temporal, passou os 47 testes de aplicação com PostgreSQL, incluindo os quatro cenários de `project_records_tests`. A suíte do workspace passou 208 testes, incluindo os dez testes de domínio de `projects_obligations`. Os registros históricos estão em `target/evidence/registry-application.log` e `target/evidence/registry-workspace.log`.

A [execução 44648996986 do GitHub Actions](https://github.com/tjsasakifln/canteiro-360/actions/runs/44648996986), sobre o commit `262acef5409983af4a157ae93a6fc48b77d594b9`, também passou 208 testes gerais, 37 de persistência, 47 de aplicação e 9 de HTTP. No navegador, passaram 11 dos 13 testes. O novo percurso chegou à página de projetos, mas não concluiu a criação pelo formulário. A investigação identificou `Referrer-Policy: no-referrer`, que faz o POST nativo enviar origem nula segundo o Fetch Standard. O HTML agora usa `same-origin`, mantendo a verificação de origem, sessão e CSRF. A outra falha levou ao ajuste do limite de tabulações do teste: a busca parava em 12 passos antes do 14º controle. Os detalhes estão no [relatório integrado](ISSUES-010-014.md).

Após a correção, o novo teste HTTP nativo passou localmente: **1 aprovado, 0 falhas, 6,95 s**. Ele verifica criação, recusa de origem ou CSRF inválidos e preservação da revisão recusada. Esse ensaio focal antecedeu a rodada completa de dez testes HTTP registrada abaixo.

A cota de artefatos do GitHub Actions impediu obter as capturas daquela primeira execução. Essa limitação motivou o transporte restrito de evidências sintéticas pelos registros da CI, usado na execução seguinte, sem encobrir a falha do upload convencional.

### CI 44657829805 e revisão das capturas anteriores ao ajuste de CSS

A [execução 44657829805](https://github.com/tjsasakifln/canteiro-360/actions/runs/44657829805) testou a head `f1c297d029fdc31742bbc0d933c5e8ed7acb86b5` pelo checkout de integração `2de5b86fa1d6ebf3ccad1f064aac59dbaf1b0464`. Passaram 213 testes gerais, 37 de persistência, 47 de aplicação, dez de HTTP e 13 de navegador, totalizando **320 testes executados**, sem contar os testes reservados como aprovações adicionais. A compilação de produção e o orçamento de arquivos também passaram. Os resultados completos estão no [relatório integrado](ISSUES-010-014.md).

O percurso de navegador conseguiu criar o projeto independente e prosseguiu pelas demais telas. As capturas `registry-projetos-360.png` e `registry-projetos-1280.png`, recuperadas com conferência de origem, tamanho e SHA-256, foram abertas e revisadas. Mostram “Projeto antes da obra” na versão 1, em elaboração, com zero entregas, e os campos de título, disciplina, objetivo e escopo repostos no formulário. A disposição muda de duas colunas para uma coluna em 360 px, sem sobreposição ou corte dos cartões e botões observados.

A revisão encontrou links do cabeçalho sem alvo mínimo de 44 px e apresentação nativa dos botões secundários no CSS compartilhado com obrigações. A correção ficou restrita a `style/projects.css`: fonte herdada, espaçamento e estados dos botões secundários, alvos de navegação de pelo menos 44 × 44 px e foco visível. Outro agente revisou o arquivo salvo, sem achado impeditivo na cascata ou na quebra de texto. As capturas desta seção antecedem esse ajuste e não constituem aprovação visual do CSS corrigido.

### CI 44663208420: ajuste de CSS verificado

A [execução 44663208420](https://github.com/tjsasakifln/canteiro-360/actions/runs/44663208420) testou a head `7166902c789aac63092a470b13175eefbd18f067` pelo checkout de integração `36edb2b1a98ebfac9385b73a180b49f71f0bb61a`. Passaram **320 testes**: 213 gerais, 37 de persistência, 47 de aplicação, dez de HTTP e 13 de navegador, sem falha funcional. O percurso integrado das cinco telas terminou em 30,49 segundos. A compilação de produção e o orçamento também passaram. A auditoria de dependências falhou pelos avisos de manutenção registrados abaixo, e o upload convencional foi recusado pela cota de armazenamento. Esses resultados permanecem separados da aprovação funcional. As novas evidências foram recuperadas pelo transporte restrito e tiveram sua integridade conferida.

Foram abertas e revisadas as capturas de [projetos em 360 px](issue-010-014/registry-projetos-360.png) e [projetos em 1280 px](issue-010-014/registry-projetos-1280.png). A fonte e a apresentação do botão secundário “Novo projeto” estão coerentes com os demais controles. A navegação tem espaçamento suficiente, os formulários continuam legíveis e não há sobreposição ou corte dos cartões e botões observados. O projeto continua em elaboração, na versão 1, com os mesmos campos preenchidos. A revisão visual deste estado, nas duas larguras, foi aprovada sem novo achado material.

O [registro das medidas do navegador](issue-010-014/registry-interface.json) confirma os controles visíveis, incluindo campos, botões, resumos expansíveis e links de navegação. Para checkboxes, considera o rótulo clicável associado.

| Largura do viewport | Controles medidos | Menor altura | Menor largura | Largura do conteúdo |
|---|---:|---:|---:|---:|
| 360 px | 25 | 44 px | 58 px | 360 px |
| 1280 px | 25 | 44 px | 63 px | 1265 px |

Os 1265 px correspondem à área útil do viewport de 1280 px com a barra de rolagem. Não foi registrado transbordamento horizontal. As capturas usam a configuração de idioma do Chrome da CI: o campo nativo de data mostra `mm/dd/yyyy`, enquanto os resumos da aplicação usam português brasileiro. A opção longa do seletor de responsável fica parcialmente visível em 360 px; o rótulo e a instrução sobre a aprovação continuam disponíveis.

Imagens estáticas não comprovam todos os estados de permissão, aprovação, revisão ou aceite, nem substituem avaliação com leitor de tela ou medição de contraste. Esses cenários permanecem associados aos testes específicos e à homologação prevista. Os avisos de manutenção de `chrono` e `chrono-tz` continuam registrados como bloqueio de integração. A aprovação visual não equivale ao aceite humano do produto ou à reconciliação com módulos financeiros.

## Limites de integração

Estas implementações partem da PR 82 e não alteram os requisitos de aceite humano das etapas anteriores. O uso por profissionais reais e a reconciliação com medições e financeiro pertencem às etapas expressamente previstas no plano. A implantação de projeto não exige a criação de contrato fictício nem transfere direitos intelectuais.
