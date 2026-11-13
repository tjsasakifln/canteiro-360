# Integração técnica das issues 3 a 6

Registro de decisão de 13/11/2026, horário de Brasília.

## Decisão e alcance

Após receber a informação de que a PR #78 aguardava a integração das etapas anteriores e o teste da issue #3 com três profissionais reais, o responsável pelo produto autorizou: "rebase se necessario, valide e merge".

A decisão autoriza a integração técnica das PRs #75, #76, #77 e #78 em `main`, nessa ordem de dependência. Este registro documenta a exceção de sequência de aceite prevista em [AGENTS.md](../../AGENTS.md). O teste com pelo menos três profissionais reais continua pendente na [issue #3](https://github.com/tjsasakifln/canteiro-360/issues/3). A autorização não equivale à realização desse teste, não altera seus critérios e não constitui aceite comercial.

Os relatórios, manifestos e artefatos anteriores permanecem preservados nos commits que identificam. Este documento acrescenta o contexto da integração e não reatribui resultados antigos a uma revisão diferente.

## Ancestralidade e método

A conferência anterior à integração encontrou a seguinte cadeia linear, sem commits divergentes de `main`:

| Referência | Commit conferido | Distância desde o predecessor |
|---|---|---:|
| `main` inicial, issue 2 | `83c94e0938c09d7d30d9f97aad6de2840cfb50e6` | Base |
| [PR #75, issue 3](https://github.com/tjsasakifln/canteiro-360/pull/75) | `2013cef3c5097c087c9e4a116c125d2dfb248596` | 8 commits |
| [PR #76, issue 4](https://github.com/tjsasakifln/canteiro-360/pull/76) | `a7f3b1ef1ab4bd654d2ce047b7bc4eb991488670` | 8 commits |
| [PR #77, issue 5](https://github.com/tjsasakifln/canteiro-360/pull/77) | `90034d15372fc209583a90fabb567942935869bd` | 4 commits |
| [PR #78, issue 6](https://github.com/tjsasakifln/canteiro-360/pull/78) | `5b711d9c11c3c4f99c53f5ef05fa12e9c4a52a05` | 4 commits |

Assim, não é necessário rebase no estado examinado. A integração utiliza commits de merge para preservar as revisões e suas evidências. Cada sucessora passa a ter `main` como base depois da integração de sua predecessora. A operação deve conferir o head esperado e a árvore resultante, respeitando eventuais requisitos impostos pelo GitHub.

A inclusão deste registro na PR #78 acrescenta somente este arquivo. A equivalência dos demais arquivos com a revisão aprovada deve ser conferida antes do merge.

## Validação que fundamenta a integração

| Objeto | Evidência | Resultado observado |
|---|---|---|
| PR #75, head `2013cef3` | [CI 43145276613](https://github.com/tjsasakifln/canteiro-360/actions/runs/43145276613) | Sucesso |
| PR #76, fonte `5588b0c6` | [CI 43227797776](https://github.com/tjsasakifln/canteiro-360/actions/runs/43227797776) | Sucesso |
| PR #76, head documental `a7f3b1ef` | [CI 43234344325](https://github.com/tjsasakifln/canteiro-360/actions/runs/43234344325) | Cancelada, sem aprovação atribuída a esta execução |
| PR #77, head `90034d15` | [CI 43322656917](https://github.com/tjsasakifln/canteiro-360/actions/runs/43322656917) | Sucesso |
| PR #78, head `5b711d9c` | [CI 43383855303](https://github.com/tjsasakifln/canteiro-360/actions/runs/43383855303) | Sucesso em todas as 25 etapas |

O comparativo entre `5588b0c695c13259e6e19b0a5a1256e39112a6c8` e `a7f3b1ef1ab4bd654d2ce047b7bc4eb991488670` contém somente alterações em `docs/validacao/`. O código da aplicação, a configuração de execução e os testes da aplicação não foram alterados nessa consolidação. A aprovação do código da PR #76 se fundamenta na execução da fonte e na validação agregada posterior, sem tratar a execução cancelada como aprovada.

A validação agregada da PR #78 aprovou 150 funções de teste: 111 nativas, 27 com PostgreSQL para autenticação e persistência, três de HTTP com PostgreSQL e nove de navegador, incluindo regressões das etapas anteriores. Os testes externos ignorados no comando nativo geral foram executados explicitamente nas etapas próprias. Formatação, regras do repositório, Clippy nativo e WASM, auditoria de dependências, migrações, compilação otimizada e limites dos arquivos também passaram.

A árvore do checkout de CI `2e2d7c07fcfdd0c24e79ae4ef599c3df7db5155e` coincide com a árvore `10782a0176e3cb643cfd4e652cbac633bbd638ab` do head `5b711d9c11c3c4f99c53f5ef05fa12e9c4a52a05`: 517 arquivos, sem diferenças de caminho ou SHA. Essa correspondência foi conferida pela revisão principal e por revisão independente.

## Verificação da integração e pendências

Depois dos merges, devem ser conferidos o commit final de `main`, sua correspondência com a árvore aprovada acrescida deste registro e o resultado da CI disparada pela integração. Os resultados e os commits efetivos ficam registrados nas descrições das PRs e nas execuções vinculadas do GitHub. Este registro anterior à integração não antecipa a aprovação de uma execução futura.

Continuam válidos os limites dos relatórios originais, inclusive a ausência de teste com os três profissionais, de comprovação de chegada de e-mail em caixa postal real, de teste com extensão específica de gerenciador de senhas e de ensaio com leitor de tela ou aparelho físico. A integração não transforma protótipos com dados fictícios em operações comerciais definitivas.

Referências preservadas: [issue 3](ISSUE-003.md), [issue 4](ISSUE-004.md), [issue 5](ISSUE-005.md) e [issue 6](ISSUE-006.md).

