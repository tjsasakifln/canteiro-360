# Reprodução nativa do ciclo de vida da planilha

Esta prova reduz a sequência de consulta, troca de tarefa, retorno pelo histórico e saída para Home a efeitos reativos aninhados, um `Resource` e 25 controles. Usa as versões fixadas de `reactive_graph 0.2.16`, `leptos_server 0.8.8` e `tachys 0.2.19`. Não contém implementação de cálculo, dados reais nem código de dependências.

O [código Rust](src/main.rs), [manifesto Cargo](Cargo.toml), [lockfile](Cargo.lock), [comparação original](comparison.json) e quatro logs foram copiados sem alterações da reprodução local. O [manifesto de arquivos](manifest.json) registra seus hashes. A [conferência independente](verification.json) reconta os panics e as limpezas por variante e por etapa.

| Variante | Argumentos | Panics registrados | Limpezas da planilha | Limpezas do shell |
|---|---|---:|---:|---:|
| Padrão | Nenhum | 50 | 2 | 1 |
| Proprietário estável | `--stable-owner` | 25 | 2 | 1 |
| Prontidão idempotente | `--idempotent-ready` | 0 | 2 | 1 |
| Ambos | `--stable-owner --idempotent-ready` | 0 | 2 | 1 |

Na variante padrão, 25 panics aparecem ao trocar para Documentos e 25 ao sair para Home. Fixar apenas o proprietário elimina o primeiro conjunto, mas conserva o segundo. A transição de prontidão somente de `false` para `true` elimina os panics nas duas etapas desse programa, mantendo as limpezas observadas.

Os resultados sustentam uma hipótese causal para a correção da fonte `5588b0c695c13259e6e19b0a5a1256e39112a6c8`: uma notificação redundante de prontidão pode agendar efeitos de controles que tentam ler sinais após a limpeza de seu proprietário. O log da CI 43220954664 mostra acesso descartado em `RwSignal<bool>::get`, mas não identifica diretamente qual variável booleana foi lida. A reprodução não autoriza atribuir a variável específica da CI nem equiparar seus 50 panics aos três panics daquele navegador. O resultado de DOM, histórico e rascunho depende da CI da fonte corrigida, registrada em [ISSUE-004](../../ISSUE-004.md).

## Como repetir

Use Rust 1.99.1 e as dependências do lockfile disponíveis no cache local. Os comandos abaixo partem da raiz do repositório. A cópia para um diretório temporário fora do workspace mantém os arquivos originais e evita tratar este laboratório como pacote da aplicação.

```sh
lifecycle_copy="$(mktemp -d)"
cp -R docs/validacao/issue-004/lifecycle-probe/. "$lifecycle_copy/"
cd "$lifecycle_copy"
cargo run --offline --locked > repeated-baseline.log 2>&1
cargo run --offline --locked -- --stable-owner > repeated-owned.log 2>&1
cargo run --offline --locked -- --idempotent-ready > repeated-idempotent.log 2>&1
cargo run --offline --locked -- --stable-owner --idempotent-ready > repeated-combined.log 2>&1
rg -n 'panicked at|cleanup Worksheet signals|cleanup shell signals|^---' repeated-*.log
```

Para cada arquivo, conte `panicked at`, `cleanup Worksheet signals` e `cleanup shell signals` e confira as etapas delimitadas por `---`. A ausência de correspondências de `panicked at` significa zero registros desse evento. O critério é o conteúdo dos logs, incluindo a ordem das etapas e limpezas. Não use apenas o código de saída: o executor pode manter o processo vivo após o panic de uma tarefa.

Esse laboratório não substitui o navegador real, não executa o tratamento de empresas e não comprova preservação de rascunho, foco ou isolamento de respostas atrasadas. Esses critérios permanecem nos ensaios integrados da aplicação.
