# Verificação da issue 6

## 1. Objeto e versão

Este registro acompanha a [PR 78](https://github.com/tjsasakifln/canteiro-360/pull/78) e a [issue 6](../issues/006.md). O objeto é o acesso por senha, confirmação de e-mail, sessões, recuperação e verificação em duas etapas, com persistência PostgreSQL e interface Rust/Leptos em `/acesso`. O [guia de acesso](../seguranca-acesso.md) documenta políticas, configuração, operação e limites.

| Referência | Identificação |
|---|---|
| Base | PR 77, `feat/issue-5-precisao-calendarios` |
| Commit da base | `90034d15372fc209583a90fabb567942935869bd` |
| Fonte anterior de implementação | `17f1c5fcdd16475be79c72253772221fa2ac5423` |
| Árvore dessa fonte | `d8f64f5f20166609d920792042da086d391e51ce` |
| Fonte final com ajuste do empacotamento | `15c4c0710076ea5b2b13fb49bf3402d42186b488` |
| Árvore da fonte final | `c30db5b52d088e49ed0ca73bb023fa6fa8d1dcf4` |
| Migração | `0003_authentication.sql`, histórico de esquema na versão 3 |
| Política inicial de senha | Argon2id v19, 65.536 KiB, três iterações, paralelismo um |

**Validação técnica local concluída em 13/11/2026:** 111 testes nativos, 12 de autenticação com PostgreSQL, 15 de persistência, três testes HTTP com PostgreSQL e nove testes de navegador, sem reprovação nas execuções identificadas. As quatro capturas finais foram abertas e conferidas pela revisão principal e por revisão independente.

O checkout local foi reconstruído a partir dos blobs da base, sem transportar o histórico Git completo. Seu identificador local não é apresentado como commit remoto. A [comparação das fontes](issue-006/local/source-correspondence.json) conferiu 94 arquivos de aplicação, ferramentas, estilos e configuração: todos coincidem com `15c4c071`. Essa fonte acrescenta apenas `--remove-name-section` à revisão anterior; a aplicação e seus testes permanecem iguais.

O [manifesto das evidências](issue-006/manifest.json) associa arquivos, comandos, hashes e revisões. A execução de integração contínua e seu commit constam da PR. Aprovação local, resultado da CI e aceite de produto permanecem identificados separadamente.

## 2. Entrega e alcance

A entrega inclui migração, contratos públicos, serviço de autenticação, armazenamento, auditoria de acesso, transporte de e-mail e tela completa. O fluxo permite cadastrar e confirmar o endereço, entrar e sair, listar e encerrar sessões, redefinir a senha, ativar um autenticador, usar uma reserva e substituir o autenticador perdido com prova de acesso recente.

A identidade de execução do banco é distinta da identidade de migração. Ela não pode promover usuários nem alterar ou excluir auditoria. O perfil privilegiado do ensaio é preparado administrativamente; o contrato público recusa campos desconhecidos e não contém promoção de privilégio.

Tokens de sessão, confirmação, recuperação e reserva possuem 256 bits aleatórios. As tabelas de validação guardam hashes com finalidades separadas. O conteúdo transitório necessário ao envio de e-mail fica cifrado e é apagado após confirmação ou descarte. O segredo do autenticador também fica cifrado com chave externa ao banco. A confirmação de e-mail ativa a senha definida pelo titular naquele momento.

O reset usa código válido por 15 minutos, consome-o na mesma transação da troca de senha, revoga sessões e preserva o fator ativo. O retorno é anônimo. A verificação em duas etapas é obrigatória para o perfil privilegiado de teste; códigos do autenticador e reservas consumidos não podem ser reutilizados.

Autenticação não concede acesso automático a organização, obra ou contrato. Autorizações desses módulos e efeitos financeiros pertencem às respectivas etapas. A preservação da interface conserva campos na aba, sem prometer rascunho durável após recarga ou fechamento. Persistência e reconciliação de trabalho de campo seguem a issue 62.

## 3. Critérios próprios da issue

| Critério | Resultado demonstrado | Evidência |
|---|---|---|
| Cookies seguros e CSRF sem mutação | Cookie `__Host-` com `Secure`, `HttpOnly`, `SameSite=Strict`, sem `Domain`; origem completa e CSRF da sessão exigidos; duplicidade de cookie recusada | Testes nativos da API e `csrf_origin_cookie_rotation_and_revocation_are_enforced_over_http` |
| Recuperação com finalidade, expiração e uso único | Token expirado, consumido ou de outra finalidade falha; dois resets concorrentes produzem uma única alteração; resposta pública genérica | `password_reset_is_single_use_expiring_revokes_and_does_not_authenticate`, teste HTTP de reset e de respostas públicas |
| Revogação e MFA obrigatório | Próximo comando recusa sessão revogada; revogação respeita o dono; perfil privilegiado permanece pendente até concluir o fator | `revocation_is_scoped_and_next_command_checks_live_state` e `mfa_privilege_recovery_replay_and_reset_preservation` |
| Preenchimento automático, colagem e teclado | Atributos apropriados, leitura do valor real dos campos e interação por WebDriver com Tab, Enter e Ctrl+V | Ensaio de acesso aprovado e capturas conferidas; limites de cobertura descritos na seção 5 |
| Salts, hashes e proteção no banco | Senhas iguais geram hashes diferentes; parâmetros acompanham a credencial; senhas e tokens não aparecem em claro nas tabelas ou na auditoria examinadas | Testes criptográficos e `identity_verification_rotation_and_opaque_storage` |
| Reset conserva MFA e não autentica | Senha anterior deixa de funcionar; sessões anteriores são revogadas; fator e reservas ativos permanecem exigidos | Testes de reset, MFA, fronteira HTTP e percurso real no navegador |

O controle de enumeração verifica mensagens públicas iguais, verificação de senha equivalente no caminho de conta ausente e piso de 500 ms nos pedidos de recuperação e confirmação. Isso reduz diferenças observáveis, sem afirmar indistinguibilidade sob qualquer carga ou topologia.

## 4. Verificações executadas

| Verificação | Resultado observado |
|---|---|
| `cargo test --locked --workspace` | 111 aprovados, zero reprovados; 39 cenários externos ignorados apenas neste comando |
| Autenticação com PostgreSQL real | 12 aprovados, zero reprovados; 163,92 s, repetidos na fonte final `15c4c071` |
| Persistência com PostgreSQL real | 15 aprovados, zero reprovados; 11,90 s |
| Fronteira HTTP com PostgreSQL real | Três aprovados, zero reprovados; 36,58 s |
| Clippy nativo, todos os alvos, e Clippy WASM | Aprovados com avisos tratados como erro; o módulo de empacotamento foi conferido novamente depois do ajuste |
| `cargo deny --locked check` | Avisos de segurança, dependências, licenças e origens aprovados; ressalvas não bloqueantes preservadas no log |
| Formatação e política do repositório | Aprovadas; plano com 72 issues e 192 regras, código e documentação conferidos novamente na consolidação |
| Compilação otimizada e orçamento servido | Aprovados, inclusive após a remoção da seção de nomes do WASM |
| Navegador, percurso próprio de acesso | Um teste aprovado novamente na execução completa, em 9,94 s |
| Navegador, regressões das etapas anteriores | Oito regressões aprovadas na mesma execução do novo cenário: dois testes da fundação, dois de cálculo, três de formulários e um do protótipo |

Os 39 ignorados correspondem a 15 testes de persistência, 12 de autenticação PostgreSQL, três HTTP e nove de navegador. Os testes de banco, HTTP e navegador possuem execuções explícitas. O [log completo de navegador](issue-006/local/browser-all.log) registra as nove funções aprovadas; os 74 vetores independentes voltaram a coincidir na comparação nativa e WASM, conforme o [relatório de paridade](issue-006/local/calculation-parity.json). Cenários internos de cada função, incluindo os vetores numéricos herdados, não são somados novamente como funções aprovadas.

Logs preservados: [nativos](issue-006/local/native.log), [autenticação PostgreSQL](issue-006/local/auth-application.log), [persistência](issue-006/local/persistence.log), [HTTP](issue-006/local/auth-http.log), [Clippy nativo](issue-006/local/clippy-native.log), [Clippy WASM](issue-006/local/clippy-wasm.log), [empacotamento após o ajuste](issue-006/local/clippy-packaging.log), [dependências](issue-006/local/dependencies.log), [regras](issue-006/local/policy.log) e [compilação](issue-006/local/build.log). O manifesto identifica a revisão de cada execução; resultados anteriores não são apresentados como reteste de alterações posteriores.

## 5. Interface e recursos servidos

O ensaio aprovado percorreu cadastro, confirmação com senha do titular, expiração com preservação dos campos, restabelecimento sem repetição automática, duas sessões, revogação remota, ativação do fator, acesso por reserva, troca do autenticador, rejeição de reservas antigas, reset em outro navegador, nova exigência de MFA e encerramento de todas as sessões.

O procedimento usa Tab e Enter para envio e Ctrl+V para colar códigos. A preparação da área de transferência não modifica campos, manipuladores ou estado da aplicação. Chaves de configuração e reservas são ocultadas antes das capturas. A ausência de credenciais no HTML público, nos logs do navegador e nos armazenamentos local e de sessão é uma verificação explícita do ensaio.

O [relatório da interface](issue-006/local/auth-interface.json) confirma as larguras de 360 e 1280 CSS px, rótulos associados, campos com 44,39 CSS px de altura e ausência de vazamento horizontal. Foram abertas e conferidas as telas de entrada em [360 px](issue-006/local/auth-login-360.png) e [1280 px](issue-006/local/auth-login-1280.png), e a conta em [360 px](issue-006/local/auth-account-360.png) e [1280 px](issue-006/local/auth-account-1280.png). O texto permanece legível, os controles não se sobrepõem e o retorno de foco aparece na mensagem da conta. Não há chaves ou reservas nas imagens.

O console registra respostas HTTP 401 nos casos deliberados de sessão revogada, credencial inválida e reserva obsoleta. Esses resultados negativos fazem parte do percurso aprovado e estão identificados no relatório; não foram classificados como console vazio nem ocultados.

A verificação de gerenciadores de senhas é limitada aos atributos de preenchimento automático e à leitura dos valores reais. Não houve demonstração com extensão específica, leitor de tela ou aparelho físico. Emulação de largura e navegação por teclado não equivalem a certificação integral de acessibilidade.

O [relatório de compilação](issue-006/local/build.json) mede os recursos destinados à aplicação, antes da inclusão de um módulo separado usado somente nos testes numéricos. A seção de nomes de depuração aumentava o WASM bruto para 29.016.202 B. Sua remoção pelo empacotador reduziu-o para 2.185.471 B, sem alterar a política de tamanho ou a lógica de negócio.

| Recurso | Medida local | Limite |
|---|---:|---:|
| WASM Brotli | 507.156 B | 1.048.576 B |
| JavaScript externo Brotli mais inline bruto da rota `/` | 8.716 B | 81.920 B |
| CSS Brotli | 7.834 B | 61.440 B |

Esses valores pertencem ao orçamento definido pelo projeto para a rota `/`. Não são medidas de rede, tempo percebido ou tamanho individual de cada rota. A integridade dos arquivos Brotli é conferida contra os bytes efetivamente servidos.

## 6. Revisão independente e correções

A revisão independente examinou requisitos, credenciais, transações, fronteira HTTP, privilégios, fila de mensagens e estados da interface. Uma conferência final de código e logs não encontrou nova falha material nesses caminhos. O relatório da interface e as capturas também foram abertos, com as limitações descritas na seção 5.

| Achado examinado | Correção aplicada | Verificação |
|---|---|---|
| Cadastro prévio por terceiro poderia ativar uma senha conhecida pelo terceiro quando o titular confirmasse o endereço | Confirmação recebe e ativa a senha escolhida junto com a prova de titularidade | `email_owner_chooses_password_after_pre_registration_attack` |
| Concessões por coluna poderiam permitir inserção de privilégio ou alteração de auditoria apesar das proibições por tabela | Validação da identidade também inspeciona permissões de coluna e recusa conexão excessivamente autorizada | `execution_identity_rejects_privileged_insert_and_column_audit_updates` |
| Atualizar uma política antiga poderia reduzir parâmetros já superiores | Rehash eleva apenas parâmetros insuficientes e conserva os custos superiores, com limites antes da alocação | Teste criptográfico de rehash e leitura da implementação |
| Usar uma reserva não resolvia a perda do autenticador | Fluxo permite substituir o fator após senha e MFA recente, mantendo o anterior até confirmar o novo | `replacement_factor_requires_recent_mfa_and_invalidates_old_recovery_codes` |
| Configuração pendente poderia ser concluída por outra sessão ou depois da janela de acesso recente | Configuração vinculada à sessão; janela conferida no início e na confirmação | `mfa_configuration_is_bound_to_session_and_fresh_authentication` |
| Mensagens substituídas ou consumidas poderiam continuar na fila, ou anunciar validade reiniciada no envio | Fila vinculada ao token ativo, descarte com remoção do conteúdo e expiração absoluta na mensagem | `obsolete_mail_is_not_claimed_and_public_requests_have_a_uniform_floor` |
| Repetição do envio poderia mudar o conteúdo ou a identidade do pedido a cada concessão | Conteúdo determinístico, UUID estável de idempotência e confirmação vinculada à concessão atual | Teste do transporte com falha 503 seguida de sucesso; teste de fila e concessões |
| Caminhos públicos de recuperação e confirmação tinham diferença de custo observável | Piso de 500 ms e mesma orientação pública, sem revelar existência | Testes de serviço e HTTP para conta existente e ausente |
| Tipos compostos da nova tela excediam a profundidade da geração de código | Fronteiras de componentes usam `.into_any()`, sem ampliar o limite do compilador | Nova compilação nativa e otimizada aprovada |
| Nomes de depuração ocupavam cerca de 26 MB no WASM servido | Remoção da seção de nomes no empacotamento de produção | Novo build e conferência do orçamento, com limites preservados |

As conclusões de leitura não são contabilizadas como testes executados. Falhas anteriores motivaram correção e reteste; a existência de um caso no código, isoladamente, não é tratada como aprovação.

## 7. Custo de senha, ambiente e reprodução

O ensaio nativo sem otimização registrou 1.979 ms para gerar o hash e 1.940 ms para verificar a senha, com Argon2id de 65.536 KiB, três iterações e paralelismo um. A [repetição otimizada](issue-006/local/argon2-release.log) registrou 109 ms e 104 ms, respectivamente, e aprovou a recusa com os dois lugares ocupados. É a repetição de um dos 12 testes de integração, sem acréscimo na contagem. Esses tempos pertencem ao executor compartilhado e não representam capacidade ou latência de produção.

O serviço limita a duas operações de senha simultâneas por instância, compartilhadas pelos clones, e recusa a próxima enquanto ambos os lugares estão ocupados. A política inicial utiliza até 128 MiB de memória Argon2 nessa condição, além das demais alocações. Políticas antigas com memória maior e múltiplas instâncias exigem dimensionamento próprio.

O ambiente usa Rust/Cargo 1.99.1, wasm-bindgen 0.2.130, cargo-deny 0.20.2 e PostgreSQL 17.7. Os testes usam somente dados fictícios, chave efêmera injetada e identidades distintas para administração, migração e execução. A chave e os códigos não integram as evidências publicadas. O ambiente local de navegador usa Chrome Headless Shell e ChromeDriver 156.0.8078.80; a CI conserva as distribuições fixadas no workflow.

Os comandos de reprodução constam do [guia de desenvolvimento](../desenvolvimento.md) e do [guia de acesso](../seguranca-acesso.md#11-testes-e-evidências). Os cenários ignorados pelo comando geral precisam ser executados explicitamente. No executor local, banco e ensaio são iniciados na mesma sessão porque processos de chamadas separadas não compartilham a rede; uma falha de conexão nessa preparação não comprova defeito nem aprovação da aplicação.

O transporte de e-mail foi conferido contra um receptor HTTP local, incluindo repetição, identidade do pedido e conteúdo. Não houve envio a uma caixa postal real nem comprovação de entrega pelo provedor. A homologação exige origem HTTPS, remetente autorizado e credencial externa configurados. A chave de cifragem precisa ser recuperável; sua rotação automática não integra esta etapa. O limite por rede usa o peer TCP, portanto uma instalação atrás de proxy precisa considerar o compartilhamento desse endereço.

## 8. Situação de integração

A PR 78 usa a PR 77 como base, preservando a ordem obrigatória. As issues 3, 4 e 5 ainda dependem de aceite e integração. Em particular, o teste da issue 3 com pelo menos três profissionais reais não pode ser substituído por estas verificações automatizadas. Esta entrega não realiza merge nem fecha a issue 6.

Os critérios técnicos próprios foram atendidos nos ensaios e na revisão registrados, dentro dos limites de método descritos. Autenticação implementada não representa aceite comercial, autorização dos módulos futuros ou prontidão da versão publicada, cujo aceite permanece na issue 72.
