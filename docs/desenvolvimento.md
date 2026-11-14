# Desenvolvimento em Rust e PostgreSQL

Este procedimento cobre a fundação, a persistência PostgreSQL e a sequência de interface, cálculos, autenticação e organizações até a issue 7. O aplicativo permanece em desenvolvimento. Cadastro verificado, sessões, recuperação, segundo fator e fila de e-mail de acesso estão descritos no [guia de segurança e acesso](seguranca-acesso.md). A rota `/organizacoes`, os convites, membros, pedidos de acesso e isolamento estão no [guia de empresas](organizacoes.md). Operações comerciais de obras e a fila geral de processamento acompanham as etapas seguintes.

## 1. Requisitos

- Git e Rustup em Linux x86_64, com compilador C/C++ e ferramentas de ligação disponíveis.
- Acesso aos registros oficiais de Rust, crates.io e ao banco de avisos RustSec.
- Pelo menos 8 GiB de RAM recomendados para compilar; o repositório limita o paralelismo a dois processos e remove símbolos de depuração dos perfis de desenvolvimento/teste para controlar memória.
- Chrome e ChromeDriver compatíveis somente para os ensaios do navegador. A integração contínua usa Chrome for Testing e ChromeDriver `156.0.8078.80`, ambos fixados.
- PostgreSQL para executar a API, as migrações e os testes de integração. A integração contínua fixa a imagem `postgres:17.7-bookworm`; o [guia de persistência](persistencia.md) descreve identidades, bancos e configuração externa.

`rust-toolchain.toml` instala Rust `1.99.1`, `rustfmt`, `clippy` e `wasm32-unknown-unknown`. As dependências diretas têm versões exatas; `Cargo.lock` fixa o grafo completo. O servidor não precisa de Node e não existe aplicação JS/TS própria.

## 2. Compilar um checkout limpo

Na raiz do repositório:

```sh
rustup show
cargo xtask setup
cargo xtask check
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo test --locked -p canteiro-persistence -- --ignored --nocapture
cargo test --locked -p canteiro-application -- --ignored --nocapture --test-threads=1
cargo deny --locked check
cargo xtask build
cargo clippy --locked -p canteiro-web --no-default-features --features hydrate --target wasm32-unknown-unknown -- -D warnings
cargo xtask budget
```

Os testes ignorados de persistência e aplicação exigem `CANTEIRO_TEST_DATABASE_URL` previamente injetada e acesso administrativo a uma instância descartável, exclusivamente de teste. Eles criam bases e identidades próprias. A ausência dessa configuração produz falha, sem substituir o PostgreSQL por simulação. A compilação e os testes unitários não exigem banco; os cenários de integração precisam ser executados explicitamente. Depois de provisionar a base de desenvolvimento e injetar uma única `CANTEIRO_AUTH_KEY` temporária para essa base, executar também `cargo test --locked -p canteiro-api --test auth -- --ignored --nocapture --test-threads=1` e a suíte de navegador. A chave da fixture não deve mudar entre execuções que conservam a mesma base.

`setup` instala `wasm-bindgen-cli 0.2.130` e `cargo-deny 0.20.2`. O comando conserva uma instalação existente somente se a versão coincidir. A versão do gerador de interoperabilidade também precisa coincidir com `wasm-bindgen` no lock.

`build` compila separadamente:

```sh
cargo build --locked -p canteiro-api -p canteiro-jobs -p canteiro-persistence --release
cargo build --locked -p canteiro-web --no-default-features --features hydrate --target wasm32-unknown-unknown --profile wasm-release
```

Em seguida, produz a interoperabilidade com `wasm-bindgen`, copia o CSS, gera os arquivos Brotli, renderiza a rota inicial pelo próprio serviço HTTP e mede o conjunto resultante. Não exporte `LEPTOS_OUTPUT_NAME`: o processo de compilação remove essa variável, pois ela muda a convenção de nome do WASM dentro do Leptos. O nome em execução é configurado por `LeptosOptions`.

Não use `--all-features` para compilar servidor e navegador juntos. `ssr` e `hydrate` pertencem a alvos diferentes; cada um possui sua verificação própria.

Os arquivos distribuíveis são `target/release/canteiro-api`, `target/release/canteiro-db`, `target/release/canteiro-jobs` e `target/site/`. Não versione `target/`. O binário, as migrações embutidas e os arquivos da interface devem ser publicados como uma única versão, sem misturar resultados de compilações diferentes.

## 3. Executar localmente

Provisione o banco e as identidades separadas conforme o [guia de persistência](persistencia.md). Injete `CANTEIRO_ENV`, `CANTEIRO_MIGRATION_DATABASE_URL` e `CANTEIRO_DATABASE_URL` no processo administrativo. As duas conexões precisam apontar ao mesmo banco e a usuários diferentes.

Para iniciar a API, injete também `CANTEIRO_AUTH_KEY` e configure `CANTEIRO_PUBLIC_ORIGIN`. O envio usa `CANTEIRO_AUTH_MAIL_CREDENTIAL` e `CANTEIRO_AUTH_MAIL_FROM` com remetente verificado na Resend. Homologação exige HTTPS e transporte configurado; os detalhes estão no [guia de acesso](seguranca-acesso.md#8-configuração-por-ambiente). O modo `--render-shell` não recebe nem requer esses segredos.

```sh
target/release/canteiro-db migrate
env -u CANTEIRO_MIGRATION_DATABASE_URL target/release/canteiro-api
```

O primeiro comando aplica e confere as migrações; o segundo inicia a API somente com a identidade de execução. A inicialização normal exige a conexão de execução, esquema acessível, ambiente compatível e papel sem privilégios que contornem o isolamento. O histórico de migrações precisa coincidir com a versão embutida no binário, incluindo versões, integridade e execução concluída. Banco antigo, migração desconhecida ou integridade divergente são recusados. A migração não ocorre automaticamente no servidor HTTP. `--render-shell` continua disponível sem conexão ao banco para a compilação e a medição dos artefatos.

Abrir `http://127.0.0.1:3000`. A página contém apresentação útil mesmo sem scripts. Quando a hidratação termina, o botão abre detalhes apenas nesta aba. Se o carregamento não termina em dez segundos, aparece a orientação para conferir a conexão e recarregar. A ação não cria nenhum registro.

`GET /healthz` informa apenas que o processo HTTP está em execução. Não é uma verificação de banco, autenticação ou prontidão comercial. Endereços inexistentes retornam 404; arquivos de configuração e do repositório não são publicados pelo servidor.

```sh
target/release/canteiro-jobs --check
```

O comando acima verifica o executável reservado ao processamento. Sem `--check`, ele termina com erro explicando que o consumidor ainda não foi implementado. Não permanece ativo simulando uma fila funcional.

## 4. Configuração e segredos

| Entrada | Desenvolvimento | Homologação |
|---|---|---|
| `CANTEIRO_ENV` | `development`, padrão | `homologation` |
| Endereço padrão | `127.0.0.1:3000` | `0.0.0.0:3000` |
| `CANTEIRO_BIND_ADDR` | Sobrescreve somente o endereço público de escuta | Mesma regra |
| `CANTEIRO_SITE_ROOT` | Diretório dos arquivos compilados, padrão `target/site` | Caminho do artefato da mesma versão |
| `CANTEIRO_DATABASE_URL` | Conexão externa da identidade de execução | Conexão de outro banco e outra identidade, próprios da homologação |
| `CANTEIRO_MIGRATION_DATABASE_URL` | Somente no processo de migração | Somente no processo de migração da homologação |

As bases públicas estão em `config/development.toml` e `config/homologation.toml`. Ambientes não reconhecidos são recusados. A fundação ainda não habilita o ambiente de produção.

Exemplo de execução de homologação em ambiente controlado:

```sh
CANTEIRO_ENV=homologation CANTEIRO_SITE_ROOT=target/site env -u CANTEIRO_MIGRATION_DATABASE_URL target/release/canteiro-api
```

A conexão externa de execução e as migrações da homologação precisam estar preparadas antes desse comando. A exposição, TLS e controle de acesso pertencem à infraestrutura de homologação. Este comando não provisiona hospedagem nem configura um domínio.

Segredos devem ser injetados no ambiente do processo por um cofre ou pelo serviço de execução. `.env.example` contém apenas configurações públicas e não é carregado automaticamente. Arquivos `.env` locais e valores secretos literais são recusados pela política de validação; mantenha os segredos fora do diretório do repositório.

O adaptador recebe uma URL externa por `DatabaseSettings::from_external_url`, não serializa essa configuração e oculta seu conteúdo em erros e diagnóstico. A API consulta somente `CANTEIRO_DATABASE_URL`; a ferramenta administrativa consulta também a configuração de migração. A credencial administrativa não deve ser fornecida ao processo HTTP. Credenciais jamais devem entrar no crate web ou nos contratos públicos. Um marcador persistido identifica `development`, `homologation` ou `test`; tentar reutilizar o banco com outro ambiente é recusado.

## 5. Verificações

| Comando | O que confere |
|---|---|
| `cargo xtask plan` | Sequência das 72 etapas, dependências, fontes e correspondência das regras entre plano, matriz e registros |
| `cargo xtask policy` | Ausência de lógica JS/TS própria, scripts inline próprios e credenciais embutidas detectáveis |
| `cargo xtask check` | Plano e política |
| `cargo xtask build` | Servidor, migrador, processamento, WASM, interoperabilidade, Brotli e orçamento |
| `cargo xtask budget` | Integridade das versões Brotli e soma do conjunto inicial, incluindo arquivos adicionais |
| `cargo test --locked -p canteiro-persistence -- --ignored --nocapture` | Migrações, isolamento, falhas e concorrência contra PostgreSQL real |
| `cargo xtask ci` | Formatação, análise, testes unitários e de banco real, dependências, compilação e navegador; requer instância de teste, banco da API migrado, ferramentas, Chrome e driver previamente configurados |

As verificações de segredos são uma barreira automatizada de formatos e padrões conhecidos, complementada pela revisão do código. A única exceção de manutenção de dependências está justificada na [decisão técnica da fundação](decisoes/0001-fundacao.md); outras advertências de segurança não recebem exceção automática.

Os limites de UX13 são 1 MiB para WASM, 80 KiB para interoperabilidade JS e 60 KiB para CSS, com Brotli nos arquivos externos. Os scripts inline são cobrados pelo tamanho sem compressão, de forma conservadora, pois fazem parte da resposta HTML inteira. O orçamento não apresenta compressões isoladas desses trechos como se fossem transferências reais.

## 6. Ensaios do navegador

Prepare o banco de desenvolvimento com as migrações e mantenha `CANTEIRO_DATABASE_URL` disponível para os servidores iniciados pelo ensaio. Defina caminhos de Chrome e ChromeDriver compatíveis, já instalados:

```sh
CANTEIRO_BROWSER_BIN=/caminho/chrome CANTEIRO_WEBDRIVER_BIN=/caminho/chromedriver cargo test --locked -p canteiro-e2e -- --ignored --nocapture
```

Os testes iniciam os processos e usam cópias temporárias dos arquivos compilados. Conferem SSR com scripts desativados, hidratação, ausência de divergência, teclado, foco, largura móvel e recuperação após falta do WASM. O teste de desempenho separa computador e celular, cache vazio e repetição, com vinte navegações em cada grupo. Não executá-los com cargas concorrentes se o resultado for usado para comparação de desempenho.

O navegador e seu driver ausentes fazem o ensaio falhar. Os testes marcados `ignored` no comando básico são executados explicitamente pela integração contínua depois da compilação dos artefatos.

Medidas, imagens e resultados ficam em `target/evidence/`. O workflow preserva esse diretório como artefato associado ao commit. As evidências anteriores da fundação permanecem no [registro da issue 1](validacao/ISSUE-001.md); não substituem a execução da revisão atual.

## 7. Banco real na integração contínua

O workflow mantém as verificações de Rust, dependências, SSR, WASM, orçamento e navegador. Acrescenta PostgreSQL `17.7`, executa explicitamente os testes marcados `ignored` da persistência, provisiona um banco de desenvolvimento com migrador e execução separados e aplica `canteiro-db migrate` antes de iniciar o navegador.

A instância é descartável e não contém dados reais. A porta do contêiner é publicada apenas em `127.0.0.1`; autenticação local sem senha é uma configuração exclusiva desse ensaio. Os endereços fictícios no workflow não são credenciais de cliente e não devem ser reutilizados em hospedagem ou homologação. A conexão administrativa dos testes e a conexão de migração ficam restritas aos respectivos passos. O passo de navegador recebe somente a conexão de execução.

`target/evidence/persistence.log` e `target/evidence/migrations.log` registram a execução sem imprimir as configurações externas. Os passos usam `pipefail`, de modo que preservar a saída com `tee` não oculta falhas dos testes ou da migração.
