# Acesso, sessões e recuperação de conta

Este guia descreve a implementação da [issue 6](https://github.com/tjsasakifln/canteiro-360/issues/6), vinculada às regras [OP04](regras/operacao.md#op04--adaptada--issue-06) e [BR21](prontidao-comercial.md#62-revisão-de-segurança). A interface fica em `/acesso`. As políticas abaixo são decisões do Canteiro 360 e complementam a [arquitetura](arquitetura-rust.md#10-segurança-e-operação).

A autenticação identifica a pessoa. Ela não concede, por si, acesso a uma organização, obra, contrato ou operação financeira. Os vínculos e as permissões dos módulos seguintes precisam ser verificados no servidor. A ação de conferência privilegiada desta etapa registra apenas um teste de acesso, sem efeito financeiro.

## 1. Cadastro e confirmação do e-mail

1. A pessoa informa o e-mail e uma senha no cadastro.
2. O servidor cria uma identidade ainda não verificada e prepara um código para o e-mail informado.
3. Em **Confirmar e-mail**, a pessoa cola o código e define a senha que será efetivamente ativada. A senha iniciada nesta aba pode ser revisada nessa etapa.
4. Depois da confirmação, a pessoa entra com a senha. A confirmação não cria uma sessão autenticada.

Definir a senha junto com a prova de titularidade evita ativar uma senha escolhida por terceiro que tenha cadastrado aquele endereço anteriormente. Cadastrar novamente o mesmo e-mail não substitui credenciais nem concede privilégios. O servidor devolve uma orientação genérica para cadastro e pedidos de código, sem confirmar publicamente a existência da conta.

O e-mail é normalizado sem espaços externos e em minúsculas. Esta versão admite endereços ASCII, inclusive caracteres válidos como `+` na parte local, e exige domínio com extensão. A senha admite de 15 a 128 caracteres, incluindo espaços e Unicode, sem exigir combinações arbitrárias de símbolos. Caracteres de controle são recusados. Não há perguntas pessoais ou desafio de memorização.

Gerenciadores de senhas recebem os atributos `username`, `current-password` e `new-password`. A leitura no envio considera o valor real dos campos, inclusive preenchimento automático. Os códigos podem ser colados integralmente.

## 2. Login e ciclo da sessão

O servidor verifica a senha e a confirmação do e-mail. Falhas devolvem a mesma mensagem pública para conta ausente, senha incorreta ou identidade não verificada. Uma conta ausente passa por uma verificação Argon2 equivalente à política inicial, reduzindo a diferença de custo do caminho de login.

| Estado | Prazo no servidor | Acesso permitido |
|---|---|---|
| Anônimo | 30 minutos | Cadastro, confirmação de e-mail e recuperação |
| Segunda etapa pendente | 10 minutos | Concluir ou preparar a segunda etapa exigida; cancelar o acesso |
| Autenticado | Até 8 horas, com expiração após 30 minutos sem atividade | Consultar a própria conta e administrar suas sessões; cada módulo deve conferir suas permissões |

O identificador da sessão muda ao entrar, concluir a segunda etapa, ativar ou substituir o autenticador, sair e redefinir a senha. Ele possui 256 bits aleatórios e somente seu hash fica na tabela de sessões. As sessões permanecem no servidor; o navegador não recebe uma autorização permanente autocontida.

O cookie `__Host-canteiro-session` usa `Secure`, `HttpOnly`, `SameSite=Strict`, `Path=/` e não declara `Domain`. Seu prazo máximo no navegador é de oito horas. A validade efetiva é a do servidor, inclusive para sessões anônimas e de segunda etapa, que têm prazo menor.

A tela permite encerrar uma sessão própria ou todas as sessões da conta. O próximo comando com uma sessão revogada é recusado. Alterações de credencial e confirmação do fator usam transações, trava da identidade e nova verificação da sessão. Um login que calculou o hash antes de uma alteração da senha precisa conferir novamente a versão e o hash atuais antes de criar uma sessão.

Uma sessão recente não substitui autorização por organização. O identificador enviado pelo navegador nunca deve ser usado como prova de vínculo nos módulos consumidores.

## 3. Recuperação da senha

Em **Esqueci minha senha**, a pessoa informa seu e-mail. O retorno público é o mesmo para endereço existente ou ausente. O pedido de recuperação e o pedido de nova confirmação têm piso de resposta de 500 ms, aplicado depois do processamento, para reduzir diferenças observáveis entre os caminhos. Esse piso não é uma promessa de tempo máximo nem uma prova de indistinguibilidade sob toda condição de carga.

O código de recuperação tem 256 bits aleatórios, é representado por texto base64url, expira em 15 minutos e pode ser usado uma vez. O banco guarda o hash com a finalidade correspondente. Um código de confirmação do e-mail não pode ser usado como recuperação de senha, e vice-versa. Pedir outro código invalida o anterior daquela finalidade.

O e-mail contém o código e o endereço fixo da página de acesso. O segredo não é colocado na URL. Em **Definir uma nova senha**, o servidor consome o código e atualiza a credencial na mesma transação. Duas tentativas concorrentes com o mesmo código não podem realizar duas alterações.

Depois de alterar a senha:

- As sessões anteriores são revogadas.
- Uma configuração de autenticador ainda não confirmada é descartada.
- O autenticador já ativo e seus códigos de reserva continuam protegendo a conta.
- A pessoa retorna ao login. A recuperação não cria uma sessão privilegiada automaticamente.

O sistema nunca envia a senha atual ou a nova senha por e-mail. O fluxo segue as decisões da [regra OP04](regras/operacao.md#op04--adaptada--issue-06) e as orientações de [recuperação de senha da OWASP](https://cheatsheetseries.owasp.org/cheatsheets/Forgot_Password_Cheat_Sheet.html).

## 4. Verificação em duas etapas e perda do autenticador

A segunda etapa utiliza um aplicativo autenticador compatível com TOTP, código de seis dígitos e período de 30 segundos. O servidor aceita a janela imediatamente anterior ou posterior para acomodar pequena diferença de relógio e impede reutilizar um passo de tempo já consumido. A fonte de tempo é o servidor.

Um perfil marcado como privilegiado precisa concluir a segunda etapa antes de obter acesso autenticado. Na extensão desta etapa, a marca é atribuída somente por preparação administrativa controlada; não existe campo público para se promover. A associação desse requisito aos papéis completos acompanha os módulos de autorização.

Para ativar o autenticador, a pessoa confirma sua senha, copia a chave de configuração e confirma um código gerado pelo aplicativo. A chave pendente vale até dez minutos e fica vinculada à sessão que iniciou a configuração. Só depois da confirmação o fator é ativado. O servidor então revoga as sessões anteriores e apresenta dez códigos de reserva, cada um utilizável uma vez. O banco guarda somente os hashes dos códigos de reserva.

Se perder o autenticador, a pessoa entra com senha e um código de reserva. Dentro dos cinco minutos seguintes à confirmação da segunda etapa, pode escolher **Substituir autenticador**, confirmar a senha atual e configurar um novo aplicativo. O fator anterior continua ativo até a confirmação do novo código. A conclusão troca o segredo, invalida os códigos de reserva anteriores, revoga as demais sessões e fornece um novo conjunto de reservas. A janela de acesso recente também é conferida ao concluir a troca.

Sem o autenticador e sem código de reserva, a conta permanece bloqueada para essa segunda etapa. A redefinição de senha e dados públicos de cadastro não autorizam desativar a proteção. Esta versão não oferece recuperação por intervenção de suporte. Um procedimento futuro precisa de identidade verificada, autorização, registro e testes próprios antes de ser disponibilizado.

Chave de configuração e códigos de reserva são mostrados apenas na resposta privada da ação que os cria. Não constam do HTML inicial, da URL ou do armazenamento persistente do navegador. A interface permite copiá-los, orienta guardá-los e oferece ocultação dos códigos após a cópia.

## 5. Senhas, criptografia e isolamento do servidor

Senhas usam Argon2id versão 19, salt aleatório por credencial e política inicial de 65.536 KiB de memória, três iterações, paralelismo um e saída de 32 bytes. Os parâmetros acompanham o hash no formato PHC. Na autenticação de uma política antiga aceita, o servidor atualiza os parâmetros insuficientes e preserva os custos superiores já existentes. Os limites de parâmetros aceitos são verificados antes da alocação. Referência: [armazenamento de senhas da OWASP](https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html).

Há no máximo duas operações de senha simultâneas por instância de `AuthService`, compartilhadas pelos seus clones. O custo de memória Argon2 da política inicial é de até 128 MiB para essas duas operações, além das demais alocações do processo. Credenciais de política antiga com memória maior podem elevar esse valor. Esse limite não representa a capacidade total de um conjunto com várias réplicas; os ensaios registram a configuração e os tempos realmente medidos.

A chave externa de autenticação protege os segredos do autenticador e o conteúdo transitório da fila com AES-256-GCM. Os dados autenticados incluem finalidade, objeto e versão, impedindo trocar um conteúdo cifrado de objeto sem que a conferência falhe. O código CSRF usa HMAC-SHA-256 vinculado ao identificador da sessão.

Criptografia, conexão do banco e segredos do provedor ficam nos crates do servidor. O crate de contratos separa respostas públicas de entidades de persistência. Credenciais e respostas que podem carregar segredos transitórios não implementam `Debug`. Erros públicos e logs do transporte não reproduzem corpo de requisição, senha, token ou mensagem do provedor.

## 6. HTTP, CSRF e controle de tentativas

`GET /api/auth/context` devolve o contexto da sessão e o código CSRF. Uma sessão expirada é substituída por contexto anônimo. O endpoint não autentica uma pessoa. A fronteira recusa estabelecimento de contexto com origem incompatível ou sinalização de navegação cruzada.

`POST /api/auth/command` exige cookie, origem completa igual à configurada e um único cabeçalho `X-CSRF-Token` válido para aquela sessão. A comparação de origem inclui esquema, host e porta. Cookies de sessão duplicados são rejeitados. Alteração de estado não é exposta por GET. O corpo JSON tem limite de 8 KiB e campos desconhecidos são recusados pelo contrato.

As respostas de acesso usam `Cache-Control: no-store`, `Pragma: no-cache`, `Referrer-Policy: no-referrer`, `X-Content-Type-Options: nosniff` e variação por cookie e origem.

| Limite | Quantidade | Janela |
|---|---|---|
| Consulta de contexto por endereço de conexão | 120 | 1 minuto |
| Comandos de acesso por endereço de conexão | 100 | 15 minutos |
| Login por e-mail normalizado | 8 | 15 minutos |
| Cadastro por e-mail normalizado | 3 | 15 minutos |
| Pedido de recuperação ou confirmação por e-mail | 3, compartilhadas entre as duas finalidades | 15 minutos |
| Confirmação da senha para configurar autenticador por conta | 5 | 5 minutos |
| Códigos do autenticador, confirmação ou reserva por conta | 6, compartilhadas entre essas ações | 5 minutos |

Os contadores são atômicos no PostgreSQL e permanecem válidos ao criar outra instância do serviço com a mesma chave. Os identificadores dos contadores são derivados criptograficamente; o endereço e o e-mail não são copiados em claro para essa tabela.

O endereço de conexão vem do peer TCP. A aplicação não confia em `X-Forwarded-For` enviado livremente. Atrás de um proxy, as conexões podem compartilhar esse endereço e, portanto, o limite. A topologia de homologação deve considerar essa condição; suporte a IP encaminhado exige uma configuração explícita de proxies confiáveis antes de ampliar essa fronteira.

## 7. Entrega de e-mail e repetição segura

O adaptador envia mensagens pelo contrato oficial [Resend, `POST /emails`](https://resend.com/docs/api-reference/emails/send-email), com `from`, `to`, `subject` e `text`. O remetente precisa estar autorizado no provedor. Não há dependência de SDK do provedor no navegador.

O código e o destinatário ficam cifrados na fila, vinculada ao token correspondente. A geração do código e a inclusão na fila ocorrem na mesma transação. O processo HTTP consulta a fila a cada dois segundos, reserva uma mensagem por vez com concessão de 60 segundos e usa prazo máximo de dez segundos para a chamada de envio, com até três segundos para conexão. Redirecionamentos do provedor não são seguidos.

Cada mensagem conserva o mesmo UUID no cabeçalho `Idempotency-Key` e o mesmo conteúdo nas novas tentativas. O identificador da concessão de processamento não compõe essa chave. A [documentação de idempotência da Resend](https://resend.com/docs/dashboard/emails/idempotency-keys) explica o controle de repetição do provedor.

Antes de reservar mensagens, o serviço descarta o conteúdo cifrado de códigos consumidos, substituídos ou expirados. A confirmação do envio também apaga o conteúdo da fila. O e-mail informa a expiração absoluta do token; esperar na fila não inicia um novo prazo de 15 minutos. Uma mensagem já em trânsito pode chegar depois de o usuário pedir outro código. Nessa situação, só o código vigente será aceito.

Uma resposta de sucesso do provedor representa aceitação do pedido de envio, não comprovação de chegada à caixa de entrada. Testes com receptor HTTP local demonstram o contrato e o comportamento de repetição; não devem ser apresentados como entrega real. A verificação operacional com caixa postal e remetente autorizados é distinta e precisa ser identificada nas evidências quando executada.

## 8. Configuração por ambiente

As configurações de banco e migração continuam seguindo [Persistência](persistencia.md) e [Desenvolvimento](desenvolvimento.md). Os valores secretos são injetados externamente e não pertencem ao repositório.

| Variável | Uso |
|---|---|
| `CANTEIRO_AUTH_KEY` | Chave aleatória externa de 32 bytes, codificada em base64url sem preenchimento. Obrigatória para iniciar a API; chave nula é recusada. Deve permanecer consistente entre instâncias do mesmo ambiente. |
| `CANTEIRO_PUBLIC_ORIGIN` | Origem completa da aplicação, sem caminho, credenciais, parâmetros ou fragmento. Homologação exige HTTPS. Em desenvolvimento, HTTP é aceito apenas em localhost ou loopback. |
| `CANTEIRO_AUTH_MAIL_CREDENTIAL` | Credencial externa do provedor de e-mail. Em desenvolvimento, sua ausência mantém o consumidor desabilitado. Homologação recusa iniciar sem o transporte configurado. |
| `CANTEIRO_AUTH_MAIL_FROM` | Remetente autorizado pelo provedor. Obrigatório quando a credencial de envio estiver configurada. |
| `CANTEIRO_AUTH_MAIL_ENDPOINT` | Endereço do transporte. Padrão: `https://api.resend.com/emails`. HTTP só é aceito para receptor local em desenvolvimento. Endereços com credenciais, parâmetros ou fragmento são recusados. |

A chave de autenticação também é necessária para recuperar segredos cifrados do banco. Guardá-la de modo recuperável junto da política de backup do ambiente, com acesso separado dos dados. Trocar arbitrariamente seu valor impede decifrar os autenticadores e as mensagens existentes. Esta etapa não implementa migração automática entre chaves; rotação exige procedimento de conversão e validação específico.

Para operar uma cópia já provisionada e com as variáveis injetadas:

```sh
target/release/canteiro-db migrate
env -u CANTEIRO_MIGRATION_DATABASE_URL target/release/canteiro-api
```

O servidor HTTP não recebe a identidade de migração. O modo `--render-shell` continua independente do banco e dos segredos, pois só gera a apresentação pública.

## 9. Migração e privilégios

A migração `0003_authentication.sql` cria o schema `canteiro_auth`, com identidades, sessões, tokens por finalidade, códigos de reserva, limites de tentativas, eventos de acesso e fila de e-mail. São identidades globais porque uma pessoa pode participar de várias organizações; essa decisão não altera o isolamento das tabelas de negócio.

A identidade de execução não é proprietária do schema e não recebe permissão de promovê-la ou promover usuários. Ela insere novos usuários somente com identificador, e-mail e hash de senha; a marca de privilégio permanece fora dessa concessão. A auditoria aceita inclusão e consulta, com atualização, exclusão e truncamento proibidos. A validação da conexão verifica também privilégios por coluna, evitando que uma concessão indireta permita modificar eventos ou inserir usuários privilegiados.

A migração é executada explicitamente pelo processo administrativo. O histórico e a integridade das migrações precisam coincidir com o binário. A versão anterior do aplicativo não deve ser iniciada sobre o novo esquema presumindo reversão automática; o mecanismo da fundação recusa um histórico incompatível.

## 10. Interface, erros e preservação do trabalho

Os formulários usam rótulos permanentes, campos com preenchimento automático apropriado, colagem, ordem de teclado e retorno de foco. Mostrar uma senha é uma escolha explícita. Erros preservam as entradas da aba. Uma resposta atrasada de uma instância de tela desmontada não atualiza a tela nova nem desloca seu foco.

Depois de expiração ou erro CSRF, **Restabelecer acesso** consulta o contexto sem reenviar a operação anterior. A pessoa decide se deve tentar de novo. A interface limpa os segredos temporários ao sair ou mudar de identidade. Ela não grava credenciais em `localStorage`, `sessionStorage` ou IndexedDB.

O HTML inicial apresenta orientação e campos sem segredos. O envio de acesso depende da hidratação WASM; os controles permanecem desabilitados enquanto ela não termina e há orientação quando scripts estão indisponíveis. As prévias das issues anteriores usam dados fictícios e não passam a gravar operações reais por existir uma sessão de conta.

Esta etapa preserva o formulário na aba, sem prometer rascunho durável após fechar ou recarregar. A persistência de trabalho de campo e sua reconciliação autorizada pertencem à issue 62. Nenhum resultado desta etapa substitui a avaliação integrada de acessibilidade e uso profissional das etapas posteriores.

## 11. Testes e evidências

Os testes unitários cobrem contratos, política de senha, vetores TOTP, proteção contra troca ou alteração de conteúdo cifrado e fronteira HTTP. A integração usa PostgreSQL real e identidades descartáveis. O comando básico de testes deixa esses cenários marcados como `ignored`; eles precisam ser executados explicitamente, com conexão de teste preparada:

```sh
cargo test --locked --workspace
cargo test --locked -p canteiro-persistence -- --ignored --nocapture
cargo test --locked -p canteiro-application -- --ignored --nocapture
cargo test --locked -p canteiro-api --test auth -- --ignored --nocapture
```

Os ensaios de navegador exigem os artefatos compilados, banco de desenvolvimento migrado, chave externa de teste, Chrome e ChromeDriver fixados. Os comandos e condições gerais constam do [guia de desenvolvimento](desenvolvimento.md#6-ensaios-do-navegador).

O conjunto deve demonstrar, na revisão identificada: cadastro por terceiro antes do titular, finalidade e expiração dos tokens, reset concorrente, senha antiga, rotação e revogação de sessão, MFA obrigatório, repetição de TOTP e reserva, substituição autorizada do fator, limites compartilhados, fila cifrada, mensagens obsoletas, CSRF, origem, cookies, concessões excessivas do banco, teclado, preenchimento automático e conservação de campos. O ensaio de Argon2 registra custo observado e recusa quando os dois lugares de processamento estão ocupados.

Este guia descreve o comportamento e o procedimento de verificação. Os resultados, números de testes, capturas, commit e limitações observadas pertencem ao relatório da versão em `docs/validacao`; não são presumidos pela presença de um teste no código.

## 12. Ordem de integração e limite da entrega

A issue 6 depende da aceitação e integração das issues 1 a 5. A implementação foi preparada sobre a PR da issue 5, preservando a sequência. Enquanto o aceite das issues 3, 4 e 5 não estiver resolvido, incluindo o teste com pelo menos três profissionais exigido pela issue 3, esta etapa permanece fora de `main`.

Autenticação implementada e testes técnicos aprovados não equivalem a liberação comercial. A aplicação ainda requer os módulos, permissões, operação, pilotos e aceite de publicação previstos no [plano](BACKLOG.md). A prontidão comercial completa permanece na issue 72.
