# Verificação da issue 7

## 1. Objeto e versão

Este registro acompanha a [PR 79](https://github.com/tjsasakifln/canteiro-360/pull/79) e a [issue 7](../issues/007.md). O objeto é a organização do acesso por empresa: cadastro, seleção explícita, membros, convites, validade, pedidos de acesso, busca e exportação. O [guia de empresas](../organizacoes.md) descreve as políticas, os percursos e os limites.

| Referência | Identificação |
|---|---|
| Base original | PR 78, `feat/issue-6-autenticacao` |
| Commit da base original | `5b711d9c11c3c4f99c53f5ef05fa12e9c4a52a05` |
| Árvore da base original | `10782a0176e3cb643cfd4e652cbac633bbd638ab` |
| Fonte da implementação | `c12e0e404212ecbe232bcd0e9a3f23a0e6443050` |
| Complemento de prova da identidade | `c27688ef83fa7b347d88415eac2af59ccbcde9ce` |
| Fonte final dos testes e da captura | `c1bc05b546d89bf0e5719007634d5a97ba67e34f` |
| Árvore dessa fonte | `88260b90326ee2348c5696b02936929332f12256` |
| Head da CI aprovada | `6a7dfce7e1378ca105237b637309c0e0f1f0a7ba` |
| Merge temporário executado pela CI | `5c6d1af14afb9ceaa30af17833acbe833be5da98` |
| Árvore executada pela CI | `6f78a7f0d3b49a4513de97c7f45b7f90be983981` |
| Base atual após a integração anterior | `main`, `3a2bbb2957da639a926a45f4fcee81716f8c31b1` |
| Migração | `0004_organizations_and_memberships.sql`, esquema na versão 4 |

As duas revisões posteriores acrescentam a prova direta de aceitação por outra identidade e melhoram a preparação, a captura e o diagnóstico do teste de navegador. A produção permanece igual à fonte de implementação. O checkout local foi reconstruído a partir dos blobs remotos; seu identificador local não é apresentado como commit do GitHub. A [comparação de fontes](issue-007/local/source-correspondence.json) confere 109 arquivos de aplicação, testes, ferramentas, estilos e configuração, sem divergência em relação à fonte final. O [manifesto](issue-007/manifest.json) identifica comandos, alcance, arquivos e hashes. As evidências anteriores do repositório foram preservadas.

**Resultado da CI de 13/11/2026:** a [execução 43422855788](https://github.com/tjsasakifln/canteiro-360/actions/runs/43422855788) concluiu com sucesso todas as 25 etapas e aprovou 188 funções de teste, sem reprovação. O cenário completo de organizações passou no Chrome em 46,35 s, inclusive a troca de conta durante uma alteração em andamento. O [resumo da CI](issue-007/ci/summary.json), o [log integral](issue-007/ci/workflow.log) e o [relatório do navegador](issue-007/ci/organizations-interface.json) preservam a comprovação. O resultado pertence ao head e ao merge temporário identificados acima; a atualização posterior de documentos é conferida separadamente na seção 9.

**Resultados locais:** testes nativos, PostgreSQL, HTTP, análises estáticas e compilação otimizada aprovados. As quatro capturas foram abertas e conferidas pela revisão principal e por revisão independente. O caso completo de organizações no navegador não foi aprovado localmente, pela limitação de ativação de abas descrita na seção 4. Esse resultado local permanece registrado e não é reclassificado pelo sucesso posterior da CI.

## 2. Critérios próprios da issue

| Critério | Comportamento demonstrado | Testes e evidência |
|---|---|---|
| A não lê nem altera B por API, ID, busca ou exportação | Referência cruzada, seleção indevida, busca, contagem, membro e TSV alheios são recusados sem devolver dados da outra empresa | Aplicação: `organizations_ids_search_export_and_private_members_never_cross` e `stale_selection_blocks_cross_tab_writes_and_foreign_locks_reveal_nothing`; persistência: `organization_rls_hides_foreign_rows_ids_counts_and_identity_directory`; HTTP: `organization_http_requires_confirmed_mfa_and_serves_only_current_company_export` |
| Conexão reutilizada não conserva contexto | A mesma conexão física volta sem a pessoa ou empresa anteriores; seleção pertence à sessão e tem geração | `organization_selection_is_session_owned_versioned_and_pool_context_is_local` e a regressão `one_connection_pool_clears_context_after_commit_rollback_failure_timeout_and_drop` |
| Convite inválido não concede direitos | Expiração, revogação, outra identidade, finalidade incompatível, consumo e repetição são recusados; aceitação concorrente produz no máximo um efeito | `invitations_bind_identity_expiry_revocation_purpose_and_single_use`, `concurrent_invitation_acceptance_has_one_effect_and_revoke_is_serialized` e `organization_invites_and_requests_are_versioned_one_use_and_revocable` |
| Troca de empresa limpa dados incompatíveis | A aplicação recusa contexto antigo; o modelo da interface vincula rascunhos à identidade e empresa e descarta respostas antigas | Testes de aplicação anteriores e dez unidades de organizações da UI aprovados; o cenário integrado `organization_membership_context_and_invitations_work_through_the_browser` passou na CI, com a distinção de ambiente da seção 4 |

Os testes da aplicação usam PostgreSQL real e uma fixture que prepara identidades e sessões para exercitar autorização e concorrência. Os testes HTTP percorrem cadastro, confirmação de e-mail, sessão e segunda etapa reais da aplicação. Os testes de persistência usam conexões reais, papéis distintos e políticas obrigatórias de segurança por linha.

## 3. Verificações executadas

A CI aprovada executou 129 testes nativos, 23 de persistência com PostgreSQL, 20 de aplicação com PostgreSQL, seis HTTP e dez de navegador: 188 aprovados, zero reprovados. Os 59 testes externos ignorados no comando nativo foram executados nas etapas próprias. Repetições, amostras de desempenho e vetores de cálculo não aumentam essa contagem.

A tabela abaixo conserva os resultados e tempos dos ensaios locais, distintos dos tempos da CI.

| Verificação | Resultado observado |
|---|---|
| `cargo test --locked --workspace` | 129 aprovados, zero reprovados; 59 externos ignorados somente nesse comando |
| Persistência PostgreSQL | 23 aprovados, zero reprovados, 6,09 s; quatro threads de teste e conexões concorrentes internas nos cenários aplicáveis |
| Aplicação PostgreSQL | 20 aprovados, zero reprovados, 310,34 s, incluindo 12 regressões de autenticação e oito de organizações |
| Repetição dos oito casos de organizações após ajustes finais do serviço | Oito aprovados, zero reprovados, 31,40 s |
| Complemento de convite com identidade errada | Um caso repetido e aprovado, zero reprovados, 3,18 s |
| HTTP PostgreSQL, revisão final | Seis aprovados, zero reprovados, 100,87 s, incluindo três regressões e três casos de organizações |
| Modelo e rede da interface após ajuste final | Dez aprovados, zero reprovados |
| Clippy nativo, WASM e cenário e2e | Aprovados com avisos tratados como erro |
| Formatação, plano e política | Aprovados; 72 issues e 192 regras preservadas |
| Auditoria de dependências | `advisories`, `bans`, `licenses` e `sources` aprovados; avisos não bloqueantes de duplicidade e licenças permitidas não encontradas permanecem no log |
| Compilação otimizada SSR/WASM e orçamento | Aprovados na fonte de produção congelada |
| Navegador, regressões das etapas anteriores | Nove aprovados, zero reprovados: autenticação, fundação, cálculos, formulários e protótipo |
| Navegador, caso completo de organizações no ambiente local | Não aprovado: timeout no último cenário, com diagnóstico e limite descritos na seção 4 |

As repetições conferem alterações específicas e não são contadas como testes novos. Os [nove casos de navegador herdados](issue-007/local/browser-regressions.log) passaram: um de autenticação, dois da fundação, dois de cálculos, três de formulários e um de protótipo. As 80 amostras de desempenho e os 74 vetores de [paridade nativa/WASM](issue-007/local/calculation-parity.json) pertencem a essas funções e não aumentam a contagem. Os 59 externos correspondem a 23 de persistência, 20 de aplicação, seis HTTP e dez de navegador. Os 12 casos de autenticação não foram alterados pelo ajuste final do serviço de organizações. Os oito casos afetados foram repetidos; o último complemento modifica apenas o teste de convite e foi executado novamente.

Logs preservados: [nativos](issue-007/local/native.log), [persistência](issue-007/local/persistence.log), [aplicação](issue-007/local/application.log), [organizações após ajuste](issue-007/local/application-organizations-final.log), [HTTP](issue-007/local/http.log), [UI](issue-007/local/ui-unit.log), [Clippy nativo](issue-007/local/clippy-native.log), [Clippy WASM](issue-007/local/clippy-wasm.log), [Clippy e2e](issue-007/local/clippy-e2e.log), [dependências](issue-007/local/dependencies.log) e [política](issue-007/local/policy.log). O [complemento de convite](issue-007/local/invitation-identity-final.log) preserva sua repetição específica. O [build final](issue-007/local/build.log) e suas [medidas](issue-007/local/build.json) identificam o pacote conferido.

## 4. Interface e recursos servidos

O percurso local `organization_membership_context_and_invitations_work_through_the_browser` chegou ao cenário final após exercitar criação, seleção explícita, convite e aceitação, escopo do membro, recusa do último administrador, falha de rede, teclado, troca de empresa, pesquisa atrasada, pedidos de acesso, decisão e remoção. Foram produzidas e revisadas quatro capturas com dados fictícios. As verificações de largura, rótulos e alvos de pelo menos 44 pixels passaram antes de gerar as imagens. O foco visível foi colocado no seletor por Tab real, sem ocultar elementos para a captura.

| Papel | Celular, 360 px | Desktop, 1280 px |
|---|---|---|
| Administrador | [Captura](issue-007/local/organizations-administrator-360.png) | [Captura](issue-007/local/organizations-administrator-1280.png) |
| Membro | [Captura](issue-007/local/organizations-member-360.png) | [Captura](issue-007/local/organizations-member-1280.png) |

**O cenário completo não foi aprovado localmente.** O Chrome completo foi impedido de iniciar pela restrição de sockets deste executor. No Headless Shell 156, a ativação nativa de abas não produziu eventos de foco ou visibilidade, comportamento também observado em uma sonda independente da aplicação. O teste local de troca de conta durante POST atrasado terminou por timeout. Na CI, o mesmo cenário obrigatório passou com Chrome for Testing completo e `--headless=new`. O relatório aprovado registra uma alteração atrasada em aproximadamente dez segundos e a consulta de identidade somente depois de sua conclusão, preservando o descarte da resposta antiga.

O teste preserva a exigência de ocultação imediata, espera pela mutação em voo, descarte da resposta antiga e confirmação da identidade atual. Não há evento JavaScript fabricado, atalho, `skip` ou alteração de produção para acomodar o ambiente. A observação do teste registra eventos e tempos, sem códigos ou identidades.

Evidências do limite local: [falha de inicialização](issue-007/local/organizations-browser-attempt-1-environment.log), [último percurso com ativação nativa](issue-007/local/organizations-browser-attempt-4-native-activation.log), [diagnóstico no aplicativo](issue-007/local/organizations-lifecycle-failure.json) e [sonda independente de foco](issue-007/local/headless-focus-probe.json). O marcador `organizations-interface.json` da tentativa local permaneceu em execução porque o teste não chegou ao final; ele não é apresentado como certificado de conclusão.

O [orçamento dos arquivos servidos](issue-007/local/budget.json) registra 664.955 bytes de WASM Brotli, 8.718 bytes de JavaScript contabilizado e 8.989 bytes de CSS, abaixo dos limites preservados de 1.048.576, 81.920 e 61.440 bytes. São medidas do pacote da aplicação, distintas de latência real em uma rede de usuário. O [ensaio de desempenho](issue-007/local/browser-performance.json) aprovou 80 navegações em quatro combinações de dispositivo e cache, com CLS zero. Essas medidas de laboratório não prometem os mesmos tempos em produção.

## 5. Revisão adversarial e correções

| Achado | Correção e verificação |
|---|---|
| Bloqueios de usuário e chaves estrangeiras de membros poderiam formar ciclo | Identidade usa `FOR NO KEY UPDATE`; empresas são bloqueadas em ordem estável; teste com dois atores autenticados e teste de referências alheias aprovados |
| Duas reduções de papel poderiam retirar os administradores simultaneamente | Guarda no serviço e no banco; teste concorrente com duas conexões e repetição em `REPEATABLE READ` aprovados |
| Limpeza da seleção usava categoria inadequada de auditoria | Seleção registra a operação no contexto da organização; cenário com uma conexão aprova criação, seleção e limpeza |
| Segunda etapa anteriormente confirmada não bastava se a configuração do fator desaparecesse | Autorização confere sessão e configuração atual; teste de perda do fator recusa administração e limita os dados |
| Mudança de acesso podia manter busca administrativa antiga na mesma empresa | Revalidação descarta resultados e códigos, mesmo sem mudança do identificador de empresa; unidades específicas aprovadas |
| Resposta antiga de uma mutação podia chegar depois da troca de identidade | A tela oculta e invalida imediatamente; espera o POST terminar, descarta sua resposta e consulta o contexto atual; unidades aprovadas; o cenário completo passou na CI, preservado o registro da limitação local |
| Ocultações repetidas podiam substituir o rascunho suspenso por campos vazios | Suspensão conserva o primeiro rascunho vinculado; teste de ocultação antes de reselecionar a mesma empresa aprovado |
| A prova da identidade do convite cobria a prévia, mas não a aceitação diretamente | Teste final também tenta aceitar com outra identidade, exige recusa e diretório vazio; executado com PostgreSQL real |

Leituras de código e configuração não são contabilizadas como execuções. A revisão de requisitos confirmou a configuração da CI: unidades pelo workspace, testes PostgreSQL ignorados executados explicitamente, módulo HTTP incluído em `--test auth` e descoberta automática do novo teste de navegador.

## 6. Alcance e limites

A migração entrega organizações, membros, convites, solicitações, seleção da sessão e auditoria. Administração exige segunda etapa confirmada. Membros comuns recebem apenas o próprio vínculo. A versão da empresa, a geração da seleção e as versões dos registros protegem alterações antigas. O último administrador permanece protegido também no banco. Administradores não possuem término automático; membros comuns podem ter validade.

O código de convite tem finalidade própria, 256 bits aleatórios, destino vinculado ao e-mail verificado e validade entre uma e 168 horas. O administrador copia e envia o código manualmente. Esta etapa não comprova envio automático ou entrega por e-mail. Existem limites de 500 convites e 500 pedidos pendentes por empresa; as consultas priorizam pendências e mostram até 500 registros. A busca admite páginas de até 100 membros, usando 50 na interface. A exportação recusa mais de dez mil resultados e orienta a restringir o filtro, sem truncamento silencioso.

Os seis testes de `organization_resources::tests` verificam interfaces de isolamento para objetos, cache, índices, filas e exportações. Pesquisa e TSV de membros já consomem a fronteira autorizada. Provedores de arquivos, cache externo, índice externo e fila geral não estão implantados nesta etapa. Chaves serializadas não concedem direitos; os consumidores futuros devem reautorizar execução e entrega.

Campos não secretos ficam apenas na memória da aba, vinculados à identidade e à empresa. Códigos privados não são preservados na suspensão. Fechar ou recarregar pode perder trabalho não enviado. A camada geral de idempotência pertence à issue 9; criar empresa ainda requer conferir a lista antes de repetir uma resposta perdida. Não há repetição automática de mutações na interface.

## 7. Ambiente e reprodução

Rust/Cargo 1.99.1, wasm-bindgen 0.2.130, cargo-deny 0.20.2 e PostgreSQL 17.7. O ensaio local usa Chrome Headless Shell e ChromeDriver 156.0.8078.80; a CI usa Chrome for Testing completo da mesma versão com `--headless=new`. Os ensaios usam dados fictícios, chave de autenticação efêmera e identidades diferentes para migração e execução. Chaves e códigos não são publicados nas evidências. Tempos de testes pertencem ao executor compartilhado e não representam latência ou capacidade de produção.

Com o ambiente preparado conforme os guias de [desenvolvimento](../desenvolvimento.md), [persistência](../persistencia.md) e [acesso](../seguranca-acesso.md), os comandos centrais são:

```sh
cargo fmt --all -- --check
cargo xtask check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo clippy --locked -p canteiro-web --no-default-features --features hydrate --target wasm32-unknown-unknown -- -D warnings
cargo test --locked --workspace
cargo test --locked -p canteiro-persistence -- --ignored --nocapture --test-threads=4
cargo test --locked -p canteiro-application -- --ignored --nocapture --test-threads=1
cargo test --locked -p canteiro-api --test auth -- --ignored --nocapture --test-threads=1
cargo deny --locked check
cargo xtask build
cargo test --locked -p canteiro-e2e --no-fail-fast -- --ignored --nocapture
```

O teste de paridade no navegador exige o verificador WASM adicional, cujo comando está no workflow da CI. Neste executor local, banco e teste são iniciados na mesma sessão, pois chamadas distintas não compartilham a rede. Essa preparação não altera o código ou o modelo de autorização.

## 8. Situação de integração

As PRs 75 a 78 foram integradas em `main` em 13/11/2026, conforme o [registro de integração das issues 3 a 6](INTEGRACAO-003-006.md). A PR 79 passa a usar `main` como base. A sincronização incorpora esse registro e a correção posterior do guia de acesso pela PR 80, preservando os arquivos de aplicação, testes e configuração já executados na CI.

O teste da issue 3 com pelo menos três profissionais reais continua pendente, e a execução automatizada não substitui esse critério. As pendências de aceite registradas no guia de acesso também permanecem válidas. A autorização da integração anterior se refere às PRs 75 a 78. Esta entrega não realiza merge nem fecha automaticamente a issue 7.

Participar da empresa não concede acesso automático a obras e contratos. Os papéis e permissões dos próximos módulos continuam sujeitos às respectivas etapas. A validação técnica registrada não representa implantação em produção, aceite comercial ou aceite da versão publicada, que permanece na issue 72.

## 9. Correspondência da CI e atualização documental

A CI executou o merge temporário `5c6d1af14afb9ceaa30af17833acbe833be5da98`, com árvore `6f78a7f0d3b49a4513de97c7f45b7f90be983981`. Comparada ao head `6a7dfce7e1378ca105237b637309c0e0f1f0a7ba`, essa árvore acrescenta somente `docs/validacao/INTEGRACAO-003-006.md`, já presente na base naquele momento. Os demais 563 arquivos são idênticos por caminho e SHA, inclusive aplicação, testes e workflow. A árvore completa do merge não é, portanto, igual à árvore do head.

A atualização posterior acrescenta as evidências da CI e corrige a situação de integração nos documentos. O guia de acesso incorpora a versão de `main` em `3a2bbb2957da639a926a45f4fcee81716f8c31b1`, conservando a execução HTTP com uma thread. O registro anterior de integração é copiado sem alteração. A comparação dos 109 arquivos de aplicação, testes, ferramentas, estilos e configuração permanece sem divergências.

Uma nova execução automática decorrente dessa sincronização tem seu próprio resultado e commit na PR. O sucesso da execução 43422855788 não é atribuído antecipadamente a essa nova execução.
