# Desenvolvimento da fundação Rust

Este procedimento entrega a issue 1. A página inicial, o servidor e o executável de processamento compilam; o aplicativo permanece em desenvolvimento. Cadastro, autenticação, banco transacional, fila e operações de obras serão implementados nas próximas issues.

## 1. Requisitos

- Git e Rustup em Linux x86_64, com compilador C/C++ e ferramentas de ligação disponíveis.
- Acesso aos registros oficiais de Rust, crates.io e ao banco de avisos RustSec.
- Pelo menos 8 GiB de RAM recomendados para compilar; o repositório limita o paralelismo a dois processos e remove símbolos de depuração dos perfis de desenvolvimento/teste para controlar memória.
- Chrome e ChromeDriver compatíveis somente para os ensaios do navegador. A integração contínua usa Chrome for Testing e ChromeDriver `156.0.8078.80`, ambos fixados.

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
cargo deny --locked check
cargo xtask build
cargo clippy --locked -p canteiro-web --no-default-features --features hydrate --target wasm32-unknown-unknown -- -D warnings
cargo xtask budget
```

`setup` instala `wasm-bindgen-cli 0.2.130` e `cargo-deny 0.20.2`. O comando conserva uma instalação existente somente se a versão coincidir. A versão do gerador de interoperabilidade também precisa coincidir com `wasm-bindgen` no lock.

`build` compila separadamente:

```sh
cargo build --locked -p canteiro-api -p canteiro-jobs --release
cargo build --locked -p canteiro-web --no-default-features --features hydrate --target wasm32-unknown-unknown --profile wasm-release
```

Em seguida, produz a interoperabilidade com `wasm-bindgen`, copia o CSS, gera os arquivos Brotli, renderiza a rota inicial pelo próprio serviço HTTP e mede o conjunto resultante. Não exporte `LEPTOS_OUTPUT_NAME`: o processo de compilação remove essa variável, pois ela muda a convenção de nome do WASM dentro do Leptos. O nome em execução é configurado por `LeptosOptions`.

Não use `--all-features` para compilar servidor e navegador juntos. `ssr` e `hydrate` pertencem a alvos diferentes; cada um possui sua verificação própria.

Os arquivos distribuíveis são `target/release/canteiro-api`, `target/release/canteiro-jobs` e `target/site/`. Não versione `target/`. O binário e os arquivos da interface devem ser publicados como uma única versão, sem misturar resultados de compilações diferentes.

## 3. Executar localmente

```sh
target/release/canteiro-api
```

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

As bases públicas estão em `config/development.toml` e `config/homologation.toml`. Ambientes não reconhecidos são recusados. A fundação ainda não habilita o ambiente de produção.

Exemplo de execução de homologação em ambiente controlado:

```sh
CANTEIRO_ENV=homologation CANTEIRO_SITE_ROOT=target/site target/release/canteiro-api
```

A exposição, TLS e controle de acesso desse ambiente pertencem à infraestrutura de homologação. Este comando não provisiona hospedagem nem configura um domínio.

Segredos devem ser injetados no ambiente do processo por um cofre ou pelo serviço de execução. `.env.example` contém apenas configurações públicas e não é carregado automaticamente. Arquivos `.env` locais e valores secretos literais são recusados pela política de validação; mantenha os segredos fora do diretório do repositório.

O adaptador de persistência recebe uma URL externa por `DatabaseSettings::from_external_url`, não serializa essa configuração e oculta seu conteúdo em erros e diagnóstico. A fundação não abre uma conexão nem consulta `CANTEIRO_DATABASE_URL`. A implementação do banco, das migrações e das transações começa na issue 2. Credenciais jamais devem entrar no crate web ou nos contratos públicos.

## 5. Verificações

| Comando | O que confere |
|---|---|
| `cargo xtask plan` | Sequência das 72 etapas, dependências, fontes e correspondência das regras entre plano, matriz e registros |
| `cargo xtask policy` | Ausência de lógica JS/TS própria, scripts inline próprios e credenciais embutidas detectáveis |
| `cargo xtask check` | Plano e política |
| `cargo xtask build` | Servidor, processamento, WASM, interoperabilidade, Brotli e orçamento |
| `cargo xtask budget` | Integridade das versões Brotli e soma do conjunto inicial, incluindo arquivos adicionais |
| `cargo xtask ci` | Verificações locais de formatação, análise, testes, dependências e compilação |

As verificações de segredos são uma barreira automatizada de formatos e padrões conhecidos, complementada pela revisão do código. A única exceção de manutenção de dependências está justificada na [decisão técnica da fundação](decisoes/0001-fundacao.md); outras advertências de segurança não recebem exceção automática.

Os limites de UX13 são 1 MiB para WASM, 80 KiB para interoperabilidade JS e 60 KiB para CSS, com Brotli nos arquivos externos. Os scripts inline são cobrados pelo tamanho sem compressão, de forma conservadora, pois fazem parte da resposta HTML inteira. O orçamento não apresenta compressões isoladas desses trechos como se fossem transferências reais.

## 6. Ensaios do navegador

Defina caminhos de Chrome e ChromeDriver compatíveis, já instalados:

```sh
CANTEIRO_BROWSER_BIN=/caminho/chrome CANTEIRO_WEBDRIVER_BIN=/caminho/chromedriver cargo test --locked -p canteiro-e2e -- --ignored --nocapture
```

Os testes iniciam os processos e usam cópias temporárias dos arquivos compilados. Conferem SSR com scripts desativados, hidratação, ausência de divergência, teclado, foco, largura móvel e recuperação após falta do WASM. O teste de desempenho separa computador e celular, cache vazio e repetição, com vinte navegações em cada grupo. Não executá-los com cargas concorrentes se o resultado for usado para comparação de desempenho.

O navegador e seu driver ausentes fazem o ensaio falhar. Os testes marcados `ignored` no comando básico são executados explicitamente pela integração contínua depois da compilação dos artefatos.

Medidas, imagens e resultados ficam em `target/evidence/`. O workflow preserva esse diretório como artefato associado ao commit. Os resultados efetivamente conferidos desta entrega e suas condições ficam no [registro da issue 1](validacao/ISSUE-001.md).
