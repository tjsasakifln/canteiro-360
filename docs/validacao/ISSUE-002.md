# Validação da issue 2: persistência e transações

Data: 11/11/2026. Base aceita: issue 1, integrada no commit `3d18ec8ae4a86f0dbf9b919895b841068c016ee2`.

Este registro cobre a fundação PostgreSQL da issue 2. Os arquivos executáveis, configurações e dados de validação do plano são identificados por SHA256 no [manifesto](issue-002/manifest.json). O commit e a execução de integração contínua correspondentes ficam vinculados ao PR e ao registro de fechamento da issue. Os resultados locais abaixo não substituem a aprovação da CI sobre a versão integrada.

## 1. Escopo entregue

- PostgreSQL e duas migrações SQLx embutidas, com execução administrativa separada e histórico conferido.
- Marcador persistente por ambiente e identidades distintas para provisionamento, migração e execução.
- Identificadores tipados, datas civis, instantes UTC com precisão de microssegundos e versões positivas com incremento verificado.
- Esquema inicial de organizações e unidades, autoria, versão, chaves compostas e políticas de isolamento por organização.
- Unidade de trabalho com consultas parametrizadas, limites de tempo, confirmação explícita, rollback e proteção contra operação cancelada ou erro ignorado.
- Contrato público de erro e adaptação HTTP com código estável, campo controlado, identificador, consequência e recuperação em português.
- Servidor exige banco compatível e papel operacional adequado. A geração isolada de HTML continua disponível sem banco.

Não foi acrescentada tela ou rota pública de gravação. Autenticação, vínculos, permissões por obra, auditoria de negócios, idempotência financeira e consulta pública de operações continuam nas issues próprias. A prontidão comercial permanece na issue 72.

## 2. Resultados locais conferidos

Ambiente: Rust/Cargo 1.99.1, Linux x86_64 e PostgreSQL 17.7 real em instância descartável. Cada ensaio de banco cria suas próprias bases e identidades fictícias. A aplicação usa papel sem propriedade, superusuário ou `BYPASSRLS`.

| Verificação | Resultado | Evidência |
|---|---|---|
| Formatação | Aprovada | `cargo fmt --all -- --check` |
| Clippy, workspace e todos os alvos nativos, avisos como erro | Aprovado | [Registro](issue-002/clippy.log) |
| Testes unitários e HTTP do workspace | 39 aprovados, zero falhas | [Registro](issue-002/native-tests.log) |
| PostgreSQL real, execução explícita | 15 aprovados, zero falhas, zero ignorados; 5,80 s | [Registro](issue-002/postgresql-tests.log) |
| Configuração de banco ausente | Recusada com código 101, antes de conectar | [Prova negativa](issue-002/missing-database-probe.log) |
| Plano e política de código/segredos | 72 issues e 192 regras coerentes; política aprovada | [Registro](issue-002/policy.log) |

Os 17 testes ignorados no comando básico são os 15 ensaios PostgreSQL e os dois ensaios do navegador. Os primeiros foram executados separadamente e aprovados. A CI executa ambos os grupos explicitamente, além da compilação SSR/WASM, orçamento dos arquivos, análise de dependências e licenças. Nenhum teste que depende do banco retorna sucesso por ausência de configuração.

## 3. Aceite por critério

| Critério da issue | Cenário observado |
|---|---|
| Banco vazio e evolução preservam dados e restrições | Aplicação das duas migrações, reaplicação, V1 com dados evoluindo para V2, preservação de nome/autor/versão/instante, data civil e limites. Runtime recusa V1, checksum alterado ou versão futura. |
| FK entre empresas é rejeitada no próprio banco | Inserção e alteração de relação cruzada executadas inclusive por superusuário de teste, independente das políticas de linha, foram recusadas. |
| Falha intermediária não deixa gravação parcial | Segunda escrita inválida, erro ignorado, transação abortada, limite de consulta/bloqueio, cancelamento parcial e cancelamento de tarefa não confirmaram alterações. |
| Pool não conserva contexto indevido | Mesmo PID PostgreSQL após commit, rollback, erro, timeout e descarte; contexto de empresa, ator e operação vazio. Contaminação proposital por `SET SESSION` também foi limpa. |
| Erro técnico produz resposta e diagnóstico seguros | Erro PostgreSQL real com marcador sensível gerou categoria e identificador, sem marcador, SQL, detalhe interno ou empresa. Testes HTTP verificam códigos, campos, cabeçalho e consequência. |

Também foram verificados concorrência com uma versão inicial comum, um único vencedor e um conflito; estouro de versão; runtime proprietário, membro de papel privilegiado ou capaz de alterar metadados; ambiente divergente; migração bloqueada; e erro diferido até o commit. Este último é tratado conservadoramente como resultado desconhecido, sem afirmar ausência de gravação nem reenviar automaticamente.

## 4. Revisão adversarial independente

Um revisor independente examinou código, requisitos, testes e resultados. Os achados foram corrigidos antes da aprovação da versão local:

| Achado | Correção e conferência |
|---|---|
| Cancelar apenas o futuro de uma chamada deixava a unidade apta a confirmar depois | Indicador de operação em andamento antes do `await`; nova chamada aborta a unidade interrompida. Probes com bloqueio real cobrem leitura e escrita canceladas mantendo a unidade viva. |
| Migração podia aguardar um bloqueio indefinidamente | Limites administrativos de bloqueio, consulta e ociosidade. Bloqueio consultivo real encerrou no prazo e não deixou bootstrap parcial. |
| Proteção do marcador de ambiente não cobria todos os privilégios perigosos | Runtime recusa inserção, alteração, exclusão, truncagem e criação de objetos pertinentes. Probes de concessão e associação de papéis passaram. |
| SQLx pode registrar automaticamente erros de callbacks | Todos os callbacks recebem mensagem fixa sanitizada. Conferência estática e captura de diagnóstico real sem entrada sensível. |
| Inicialização não conferia a versão exata do banco | Comparação integral de versão, quantidade, ordem, checksum e conclusão das migrações. Probes V1, histórico alterado e versão futura passaram. |
| Variáveis fictícias administrativas da CI eram herdadas por outros passos | Escopo por passo e remoção das variáveis auxiliares; o navegador recebe somente a conexão operacional. |

A primeira prova de captura de logs apresentou interferência entre testes paralelos. A instrumentação foi corrigida com um único subscriber de teste e fechamento do pool antes da inspeção, mantendo a exigência positiva do evento e do identificador. O caso passou isoladamente e na suíte completa; nenhum critério de sigilo foi removido.

O revisor não identificou bloqueador remanescente no escopo examinado. A conferência do responsável principal repetiu os 15 ensaios reais, os 39 testes nativos e os controles de formatação, Clippy, plano e política. A integração depende ainda dos resultados da CI registrados no PR.

## 5. Operação e reprodução

O [guia de persistência](../persistencia.md) descreve os privilégios, parâmetros externos, migração e tratamento dos resultados. Com as configurações de teste já injetadas:

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo test --locked -p canteiro-persistence -- --ignored --nocapture
cargo xtask check
```

O [workflow](../../.github/workflows/ci.yml) reproduz o banco descartável, a migração administrativa, os testes de banco e os controles completos herdados da fundação. Ele preserva os logs e as evidências de navegador por commit. A alteração não provisiona produção e não usa dados de clientes.
