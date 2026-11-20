# Issue 14: responsabilidades e obrigações técnicas

## Implementação

ART, RRT, designação, licença, seguro, garantia e referência ao CNO são registros distintos. A obrigação pode pertencer a obra, projeto, contrato ou orçamento, sem exigir um projeto fictício. Natureza, atividade, disciplina, escopo, profissional aplicável, vigência, data da atividade, número, emissor e versão do anexo ficam registrados.

As versões documentais são referenciadas por empresa, documento, versão e hash do conteúdo. A aplicação exige uma versão já aceita pelo fluxo de verificação de arquivos, pertencente ao mesmo escopo e à categoria técnica ou evidência. ART e RRT exigem via em PDF ou PNG. Um identificador inexistente ou de quarentena não é comprovante. Número provisório ou exemplo não confirma a obrigação.

Para ART, RRT e designação, confirmar exige atuação profissional válida na data da atividade. Cadastro de pessoa e conta de usuário permanecem independentes. ART de orçamento não cobre automaticamente execução, outra disciplina ou uma data fora da vigência. A aplicação confere o conselho do profissional em relação à natureza ART/RRT.

Documento conferido não é reescrito. A substituição cria outro registro, exige nova versão documental, preserva o conteúdo, a vigência e o responsável anteriores, e mantém a relação entre as vias. O banco protege versões e relações de substituição contra alteração e exclusão.

A exceção exige fonte identificável, endereço HTTPS, fundamento específico e anexo. É salva em elaboração e depois aprovada por outra pessoa com a permissão de aprovar exceções. A aprovação não pode alterar a redação apresentada. O produto informa que se trata de decisão interna, sem autenticar documento do conselho nem afirmar dispensa legal.

## Fontes e interpretação

- [Resolução CONFEA 1.137/2023, texto consolidado](https://normativos.confea.org.br/Ementas/Visualizar?id=76099), arts. 2, 4, 10, 12 e 27 a 30, consultada em 19/11/2026. A aplicação diferencia responsabilidade e atividade e informa a regra temporal de registro. O cadastro interno não efetiva ART.
- [Resolução CAU 184/2019, art. 1, nova redação do art. 2 da Resolução 91/2014](https://transparencia.caubr.gov.br/resolucao184/), consultado em 19/11/2026. Execução e projeto têm condições diferentes; não se aplica prazo genérico único aos grupos.
- [Cadastro Nacional de Obras, Receita Federal](https://www.gov.br/receitafederal/pt-br/assuntos/construcao-civil/cno/cno). O cadastro interno é referência documental, sem substituir a inscrição oficial.
- Regras RT-04, ORC24 e ORC25 permanecem rastreadas na matriz do repositório. A dispensa de um campo na interface histórica do SICOP não é tratada como dispensa legal.

A confirmação do RRT fora da execução exige enquadramento normativo e evento de prazo. As alternativas do inciso II estão identificadas por grupo 1, grupo 4 ou atividade 3.1, 7.8.12 e 7.8.13. O orçamento pertence ao grupo 1, itens 1.7.3 e 1.10.3; a fiscalização 3.6 entra nas demais atividades do inciso III, conforme a [Resolução CAU 21/2012](https://transparencia.caubr.gov.br/resolucao21/), consultada em 19/11/2026. Projeto/orçamento não aceitam o evento das demais atividades e fiscalização não aceita enquadramento de projeto para escapar do prazo.

No inciso II, informa-se o primeiro evento aplicável entre término, entrega, protocolo ou divulgação; o registro não pode ser posterior a essa data. No inciso III, informa-se o término e verifica-se também o limite de 30 dias contados do início. A interface solicita a classificação conforme o documento, inclusive quando a atividade de negócio é “Outra”. Múltiplo Mensal, emergência decretada e demais hipóteses especiais seguem a aprovação de exceção com fundamento e anexo próprios, sem presunção de dispensa.

As datas civis armazenadas não comprovam a ordem de eventos ocorridos no mesmo dia. A página explica essa limitação e a necessidade de conferir entrega, protocolo e divulgação no caso do RRT de projeto. Não há conexão de autenticação com CREA ou CAU, e nenhum estado recebe o nome de documento autenticado.

## Critérios e evidências reproduzíveis

| Critério | Cenário |
|---|---|
| ART de orçamento não cobre execução | `budget_art_does_not_cover_execution_or_another_discipline`. |
| Prazo do RRT por enquadramento e evento | Testes `council_and_registration_timing_depend_on_the_document_nature`, `other_rrt_activities_need_both_thirty_day_limit_and_completion_event` e `rrt_framework_cannot_be_switched_to_escape_the_applicable_deadline`. |
| Profissional expirado não assume evento futuro | `obligation_requires_real_same_scope_evidence_and_effective_professional`, que recusa vínculo encerrado em 2026 para atividade em 2027. |
| Documento inexistente ou de outro escopo | O mesmo cenário recusa ID inexistente e arquivo aceito de outro projeto. |
| Versão anterior e responsável preservados | O cenário confirma uma substituição e compara o registro e hash originais com o histórico preservado. |
| Exceção fundamentada e segregada | `technical_exception_requires_separate_approver_and_unchanged_proposal` exige aprovador distinto e recusa conteúdo alterado durante a aprovação. |
| Link e dados de entrada seguros | Domínio recusa fonte `javascript:` até no rascunho; Leptos escapa campos ao gerar HTML. |
| Pendência e recusa no navegador | O percurso integrado da CI 44657829805 salva ART pendente, recusa a conferência sem profissional e mantém o número provisório e os demais campos enviados. |

### Histórico local e CI 44648996986

Execução local: suíte de domínio `projects_obligations` e filtro PostgreSQL `project_records_tests`. As rotas nativas `/obrigacoes` e `/obrigacoes/acao` preservam os campos em erro e exigem proteção de origem, sessão e CSRF. A rodada central anterior ao ajuste HTTP, já com as alterações de `RrtFramework` e revalidação temporal, passou os 47 testes de aplicação com PostgreSQL, incluindo os quatro cenários de `project_records_tests`. A suíte do workspace passou 208 testes, incluindo os dez testes de domínio de `projects_obligations`. Os registros históricos estão em `target/evidence/registry-application.log` e `target/evidence/registry-workspace.log`.

A [execução 44648996986 do GitHub Actions](https://github.com/tjsasakifln/canteiro-360/actions/runs/44648996986), sobre o commit `262acef5409983af4a157ae93a6fc48b77d594b9`, passou 208 testes gerais, 37 de persistência, 47 de aplicação e 9 de HTTP. No navegador, passaram 11 dos 13 testes. O novo percurso foi interrompido no envio do projeto, antes de alcançar a etapa de obrigações. Foram corrigidos o limite de tabulações do teste, que parava em 12 passos antes do 14º controle, e a política de referência das páginas nativas. O HTML de projetos e obrigações agora usa `Referrer-Policy: same-origin` para preservar a origem nos POST internos; a proteção de origem, sessão e CSRF continua exigida. Os detalhes estão no [relatório integrado](ISSUES-010-014.md).

O novo teste HTTP `native_project_form_creates_independently_and_preserves_invalid_revision` passou localmente após a correção: **1 aprovado, 0 falhas, 6,95 s**. Ele verifica a criação e o erro de revisão em projetos e também o cabeçalho da página `/obrigacoes`; não cobre o POST de ART. Esse ensaio focal antecedeu a rodada completa de dez testes HTTP e o percurso de obrigações no navegador registrados abaixo.

A cota de artefatos do GitHub Actions impediu obter as capturas daquela primeira execução. Essa limitação motivou o transporte restrito de evidências sintéticas pelos registros da CI, usado na execução seguinte, sem encobrir a falha do upload convencional.

### CI 44657829805 e revisão das capturas anteriores ao ajuste de CSS

A [execução 44657829805](https://github.com/tjsasakifln/canteiro-360/actions/runs/44657829805) testou a head `f1c297d029fdc31742bbc0d933c5e8ed7acb86b5` pelo checkout de integração `2de5b86fa1d6ebf3ccad1f064aac59dbaf1b0464`. Passaram 213 testes gerais, 37 de persistência, 47 de aplicação, dez de HTTP e 13 de navegador, totalizando **320 testes executados**, sem contar os testes reservados como aprovações adicionais. A compilação de produção e o orçamento de arquivos também passaram. Os resultados completos estão no [relatório integrado](ISSUES-010-014.md).

O percurso de navegador salvou a ART pendente da obra, tentou conferi-la sem indicar profissional e recebeu o erro esperado. As capturas `registry-obrigacoes-360.png` e `registry-obrigacoes-1280.png`, recuperadas com conferência de origem, tamanho e SHA-256, foram abertas e revisadas. Mostram a versão salva 1, o estado “Pendente de conferência”, a mensagem “Indique o profissional responsável pela atividade” e o número `PENDENTE` mantido no formulário. Obra, execução, disciplina e descrição permanecem preenchidas. O motivo aparece integralmente em desktop; o campo de uma linha mostra apenas parte do texto em 360 px, sem transbordar o controle. O teste de navegador confere também que a obrigação continua sem confirmação.

Não foram observados sobreposição ou corte de cartões e botões nas duas larguras. A revisão encontrou apresentação nativa e fonte menor nos botões secundários, além de links do cabeçalho sem alvo mínimo de 44 px. A correção ficou restrita a `style/projects.css`, compartilhado pelas páginas nativas: fonte herdada e estados dos botões, espaçamento, foco visível e alvos de navegação de pelo menos 44 × 44 px. Outro agente revisou o arquivo salvo, sem achado impeditivo. As capturas desta seção antecedem esse ajuste e não constituem aprovação visual do CSS corrigido.

### CI 44663208420: ajuste de CSS verificado

A [execução 44663208420](https://github.com/tjsasakifln/canteiro-360/actions/runs/44663208420) testou a head `7166902c789aac63092a470b13175eefbd18f067` pelo checkout de integração `36edb2b1a98ebfac9385b73a180b49f71f0bb61a`. Passaram **320 testes**: 213 gerais, 37 de persistência, 47 de aplicação, dez de HTTP e 13 de navegador, sem falha funcional. O percurso integrado das cinco telas terminou em 30,49 segundos. A compilação de produção e o orçamento também passaram. A auditoria de dependências falhou pelos avisos de manutenção registrados abaixo, e o upload convencional foi recusado pela cota de armazenamento. Esses resultados permanecem separados da aprovação funcional. As novas evidências foram recuperadas pelo transporte restrito e tiveram sua integridade conferida.

Foram abertas e revisadas as capturas de [obrigações em 360 px](issue-010-014/registry-obrigacoes-360.png) e [obrigações em 1280 px](issue-010-014/registry-obrigacoes-1280.png). Os botões “Salvar pendência” e “Conferi a versão atual: reaplicar pendência” agora têm fonte e apresentação coerentes com os demais controles. O texto dos botões quebra em linhas dentro da largura disponível, sem sobreposição ou corte. A navegação tem espaçamento suficiente. A ART continua na versão salva 1 e no estado pendente, com erro explícito de profissional ausente e os campos preenchidos preservados. A revisão visual deste estado, nas duas larguras, foi aprovada sem novo achado material.

O [registro das medidas do navegador](issue-010-014/registry-interface.json) confirma os controles visíveis, incluindo campos, botões, resumos expansíveis e links de navegação.

| Largura do viewport | Controles medidos | Menor altura | Menor largura | Largura do conteúdo |
|---|---:|---:|---:|---:|
| 360 px | 32 | 44 px | 58 px | 360 px |
| 1280 px | 32 | 44 px | 63 px | 1265 px |

Os 1265 px correspondem à área útil do viewport de 1280 px com a barra de rolagem. Não foi registrado transbordamento horizontal. Os campos nativos de data mostram o formato do Chrome da CI, como `11/20/2026`, enquanto os resumos usam `20/11/2026`. Em 360 px, o motivo longo fica parcialmente visível no campo de uma linha; a captura de desktop permite ler o texto completo.

O cenário visual é uma pendência com recusa de conferência, não uma ART confirmada, RRT aprovado ou substituição concluída. Imagens estáticas não substituem avaliação com leitor de tela ou medição de contraste. A conferência de anexo nos ensaios usa uma fixture explícita de scanner; não comprova definições de antivírus real nem autenticidade perante conselho. Os avisos de manutenção de `chrono` e `chrono-tz` continuam registrados como bloqueio de integração. A aprovação visual não constitui aceite humano do produto ou confirmação perante conselho.

## Revisão independente

O responsável pela issue 10 revisou a ligação de anexos em `obligations.rs` e na migração 0013. Conferiu existência da versão aceita, empresa, escopo, categoria, formato aplicável, hash, nova via na substituição e imutabilidade da anterior, sem encontrar falha impeditiva nessa ligação. A revisão não constitui autenticação do documento perante o conselho.
