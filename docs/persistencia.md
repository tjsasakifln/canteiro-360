# PostgreSQL, migrações e transações

Esta fundação implementa a issue 2. Ela fornece tipos de domínio, integridade entre organizações, migrações, acesso transacional e erros públicos. Os cadastros completos, a autenticação, a autorização por vínculo e obra, a auditoria imutável de negócios e a prevenção de duplicidade financeira serão ligados nas etapas correspondentes do plano. Não há endpoint público de gravação nesta entrega.

## 1. Separar ambiente e identidade

Cada ambiente usa banco e identidades próprios. Não apontar desenvolvimento ou testes a dados reais. A ferramenta de migração grava um marcador com o ambiente selecionado e recusa tentativas de alterá-lo por configuração. A API aceita `development` e `homologation`; os ensaios internos usam `test`. Produção ainda não está habilitada.

| Identidade | Uso | Privilégios necessários |
|---|---|---|
| Administrador de provisionamento | Criar banco e identidades fora da aplicação | Administração da instância, restrita ao provisionamento |
| Migrador | Executar `canteiro-db migrate` | Dono do banco e dos objetos da aplicação; sem superusuário ou `BYPASSRLS` |
| Execução | API e futuras operações autorizadas | Conexão e privilégios concedidos explicitamente pelo migrador; sem propriedade de tabelas ou capacidade de assumir papéis privilegiados |
| Administrador de teste | Ensaios automatizados em instância descartável | Superusuário somente nessa instância, para criar e remover as bases e identidades exclusivas dos ensaios |

Exemplo de provisionamento de desenvolvimento, executado pelo administrador em uma instância destinada a esse ambiente:

```sql
CREATE ROLE canteiro_development_migrator
    LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT NOREPLICATION NOBYPASSRLS;
CREATE ROLE canteiro_development_runtime
    LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT NOREPLICATION NOBYPASSRLS;
CREATE DATABASE canteiro_development OWNER canteiro_development_migrator;
REVOKE ALL ON DATABASE canteiro_development FROM PUBLIC;
```

Configure a autenticação dessas identidades na infraestrutura e mantenha os segredos fora do repositório. As instruções SQL acima não criam senhas nem habilitam acesso de rede. Em homologação, use outro banco, outros papéis e autenticação própria. Não conceda ao papel de execução associação ao migrador, privilégios administrativos, `TRUNCATE`, criação de objetos nos esquemas da aplicação ou em `public`, nem escrita nos metadados, inclusive por coluna.

O isolamento por linhas não substitui autorização de usuário. Quem chama `Database::begin` deve obter empresa e ator de contexto autorizado no servidor. Até as etapas de autenticação e vínculos, a biblioteca permanece sem rotas de mutação acessíveis pelo navegador.

## 2. Aplicar migrações

Injete no processo administrativo, a partir do ambiente de execução ou do cofre:

| Variável | Valor esperado |
|---|---|
| `CANTEIRO_ENV` | `development`, `homologation` ou `test`, explicitamente escolhido |
| `CANTEIRO_MIGRATION_DATABASE_URL` | Conexão PostgreSQL da identidade de migração |
| `CANTEIRO_DATABASE_URL` | Conexão PostgreSQL da identidade de execução para o mesmo banco |

Não há carregamento automático de `.env`. Conexões reais que carregam credenciais permanecem fora de comandos versionados, mensagens, capturas e logs. Os endereços sem senha da integração contínua são dados fictícios restritos ao contêiner descartável.

Com as variáveis já injetadas:

```sh
cargo run --locked -p canteiro-persistence --bin canteiro-db -- migrate
```

Depois de compilar os artefatos de distribuição, o comando equivalente é:

```sh
target/release/canteiro-db migrate
```

O comando verifica que as duas conexões usam identidades diferentes e o mesmo banco, registra ou confere o ambiente, executa as migrações SQLx e concede os privilégios de execução previstos. Por último, conecta com a identidade de execução e verifica se ela pode ser usada pela aplicação. Uma falha nessa conferência termina o processo com erro; não transforma uma configuração inadequada em sucesso.

| Migração | Conteúdo |
|---|---|
| `0001_tenant_foundation.sql` | Organizações e unidades, autoria, instantes, versão, chaves compostas, relações por empresa e políticas de isolamento |
| `0002_civil_dates_and_indexes.sql` | Data civil opcional, índices por organização e limites dos instantes, por expansão aditiva |

As migrações são embutidas no binário. O histórico e as verificações de integridade do SQLx permitem conferir o que já foi aplicado. Não edite uma migração aplicada: crie a próxima versão. Uma repetição sem alterações não recria os dados. Os testes com banco real exercitam tanto a criação vazia quanto a evolução da primeira versão com dados existentes.

Não há comando de reversão destrutiva automática. Uma restauração ou migração futura que remova dados exige procedimento próprio, cópia de segurança verificada e conversão planejada. Voltar apenas o binário não desfaz uma migração.

## 3. Iniciar o servidor

Após as migrações, execute a API apenas com a identidade operacional:

```sh
env -u CANTEIRO_MIGRATION_DATABASE_URL target/release/canteiro-api
```

O processo exige `CANTEIRO_DATABASE_URL`, confere o ambiente persistido e recusa papéis que possam ignorar o isolamento. Também compara o histórico de migrações ao conjunto embutido no binário, verificando versão, integridade e conclusão: banco antigo, incompleto, alterado ou com migração desconhecida é recusado. A identidade de execução tem leitura somente das colunas de histórico necessárias a essa checagem, sem poder alterá-las. O pool fica disponível na camada do servidor; a configuração não é enviada ao HTML ou ao WebAssembly. A rota `/healthz` informa que o processo HTTP está vivo, sem detalhar a infraestrutura ou declarar prontidão comercial.

O comando `target/release/canteiro-api --render-shell` continua sem conexão ao banco. Ele permite compilar e medir o HTML da página informativa; não inicia um serviço apto a gravar dados.

## 4. Integridade e ciclo da transação

As entidades de cliente carregam `tenant_id`. A chave de uma unidade é `(tenant_id, id)`; sua relação com a unidade superior também inclui `tenant_id`. Assim, apontar a uma unidade de outra organização viola a chave estrangeira no próprio PostgreSQL. As políticas de isolamento restringem leitura e gravação, inclusive sem contexto. O papel de execução não é dono das tabelas e não tem permissão para ignorar essas políticas.

`TransactionContext` reúne empresa, ator e identificador de operação. O contexto é definido com escopo local da transação. A limpeza ao criar, devolver e adquirir conexões evita recuperar uma empresa ou um ator deixados por uso anterior do pool. O pool e o executor SQL bruto não são expostos pela API pública da biblioteca.

`UnitOfWork` mantém o conjunto de alterações até a confirmação. As consultas usam parâmetros preparados pelo SQLx. Erro de validação ou SQL interrompe a unidade, mesmo se o chamador ignorar o `Result`. A edição exige a versão esperada; duas alterações concorrentes não sobrescrevem silenciosamente a mesma versão. O valor entregue por uma consulta durante a unidade ainda é provisório. Somente `Committed<T>`, produzido após confirmação positiva do banco, representa o resultado confirmado.

Falha anterior à tentativa de confirmação causa reversão. Descartar uma unidade não confirma suas alterações. Uma falha de conexão ou de tempo durante a confirmação exige tratamento distinto: o servidor pode não saber se o banco confirmou. Nesse caso, a resposta é `C360_OUTCOME_UNKNOWN`, sem repetição automática e sem afirmar que nada foi gravado. O identificador de operação deve acompanhar a investigação. A consulta pública de estado e os efeitos financeiros idempotentes pertencem às etapas posteriores; não há endpoint fictício de recuperação nesta fundação.

## 5. Tipos e limites

| Valor ou limite | Regra |
|---|---|
| Identificadores | Tipos distintos de UUID; valores nulo e máximo são recusados |
| Data civil | `YYYY-MM-DD`, anos 0001 a 9999; sem horário ou conversão de fuso |
| Instante | UTC, até microssegundos, anos 0001 a 9999 após normalização; precisão excedente é recusada |
| Versão | Inteiro positivo de 64 bits; transporte como texto; incremento com verificação de estouro |
| Pool padrão | Até 10 conexões |
| Espera padrão por conexão | 5 segundos |
| Consulta padrão | Até 5 segundos |
| Espera padrão por bloqueio | Até 1 segundo |
| Unidade de trabalho padrão | Até 15 segundos |

`TransactionLimits::new` permite ajustes explícitos pelo servidor, com 1 a 50 conexões e tempos entre 10 e 60.000 milissegundos. A espera por bloqueio não pode superar a consulta; a consulta não pode superar o limite da unidade. Não existe configuração livre enviada pelo cliente para remover esses limites.

## 6. Erros públicos e diagnóstico

A resposta pública contém `code`, `field`, `message`, `operation_id`, `write_outcome` e `recovery`. O adaptador HTTP repete o identificador em `x-operation-id` e usa `Cache-Control: no-store`. Os textos vêm de um catálogo fixo; detalhes de SQL, nome de restrição, configuração externa e conteúdo fornecido pelo usuário não são incorporados à mensagem pública.

| Código | Resposta HTTP | Orientação |
|---|---:|---|
| `C360_INVALID_INPUT` | 422 | Revisar os dados sem apagar os demais campos |
| `C360_NOT_FOUND` | 404 | Conferir registro, empresa e acesso |
| `C360_CONFLICT` | 409 | Conferir o estado ou a versão antes de tentar novamente |
| `C360_UNAVAILABLE` | 503 | Preservar o preenchimento e tentar após a indisponibilidade |
| `C360_TIMEOUT` | 504 | Nenhuma alteração confirmada; preservar os dados e tentar novamente |
| `C360_INTERNAL` | 500 | Preservar dados e informar a operação ao suporte |
| `C360_OUTCOME_UNKNOWN` | 500 | Não reenviar; verificar se a alteração ocorreu |

`write_outcome=not_committed` só é apropriado quando a confirmação não ocorreu. `unknown` indica resultado indeterminado durante a confirmação. O diagnóstico da persistência registra categoria e identificador opaco da operação, sem empresa, ator, parâmetros, consulta ou erro bruto do banco. O adaptador desativa o registro de consultas SQLx para não levar valores ao log. Erros de negócio futuros devem preservar esse contrato.

## 7. Reproduzir os testes com PostgreSQL real

Disponibilize uma instância PostgreSQL descartável e injete `CANTEIRO_TEST_DATABASE_URL` com a identidade administrativa de teste. A suíte exige acesso por `127.0.0.1`, `localhost` ou `::1`, banco de conexão cujo nome contenha `test` e superusuário nessa instância descartável. Essa identidade cria as bases e papéis dos ensaios; ela nunca deve ser a identidade da API. Não use dados de cliente nesse ambiente. O [workflow](../.github/workflows/ci.yml) contém um exemplo completo de provisionamento descartável.

```sh
cargo test --locked -p canteiro-persistence -- --ignored --nocapture
```

Os testes usam bases próprias e dados fictícios. O comando falha quando a configuração necessária está ausente. A marcação `ignored` separa ensaios que precisam da infraestrutura externa dos testes unitários; ela não dispensa sua execução na integração contínua nem no aceite da issue.

Para verificar a versão completa, prepare também o banco de desenvolvimento migrado da API, a conexão de execução, Chrome e ChromeDriver e execute:

```sh
cargo xtask ci
```

A integração contínua executa os testes de banco antes dos ensaios do navegador e preserva os logs em `target/evidence/`. Os resultados de execução e da revisão adversarial devem ser associados ao commit efetivamente conferido, sem usar evidência de uma revisão antiga como aprovação da atual.
