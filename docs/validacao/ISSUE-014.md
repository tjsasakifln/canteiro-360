# Issue 14: responsabilidades e obrigações técnicas

## Implementação

ART, RRT, designação, licença, seguro, garantia e referência ao CNO são registros distintos. A obrigação pode pertencer a obra, projeto, contrato ou orçamento, sem exigir um projeto fictício. Natureza, atividade, disciplina, escopo, profissional aplicável, vigência, data da atividade, número, emissor e versão do anexo ficam registrados.

As versões documentais são referenciadas por empresa, documento, versão e hash do conteúdo. A aplicação exige uma versão já aceita pelo fluxo de verificação de arquivos, pertencente ao mesmo escopo e à categoria técnica ou evidência. ART e RRT exigem via em PDF ou PNG. Um identificador inexistente ou de quarentena não é comprovante. Número provisório ou exemplo não confirma a obrigação.

Confirmar exige atuação profissional válida na data da atividade. Cadastro de pessoa e conta de usuário permanecem independentes. ART de orçamento não cobre automaticamente execução, outra disciplina ou uma data fora da vigência. A aplicação confere o conselho do profissional em relação à natureza ART/RRT.

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

Execução local: suíte de domínio `projects_obligations` e filtro PostgreSQL `project_records_tests`. As rotas nativas `/obrigacoes` e `/obrigacoes/acao` preservam os campos em erro e exigem proteção de origem, sessão e CSRF. O filtro PostgreSQL foi executado com sucesso em 19/11/2026 no horário de Brasília: 4 testes, 0 falhas, 6,69 s. O relatório integrado registra a revisão visual e a verificação final do conjunto.

## Revisão independente

O responsável pela issue 10 revisou a ligação de anexos em `obligations.rs` e na migração 0013. Conferiu existência da versão aceita, empresa, escopo, categoria, formato aplicável, hash, nova via na substituição e imutabilidade da anterior, sem encontrar falha impeditiva nessa ligação. A revisão não constitui autenticação do documento perante o conselho.
