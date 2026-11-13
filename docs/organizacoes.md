# Empresas, equipe e convites

Implementação da issue #7 sobre a autenticação da issue #6. A página `/organizacoes` usa contas verificadas e PostgreSQL real. A associação a uma empresa não concede acesso automático a obras, contratos ou projetos; as permissões desses recursos pertencem à issue #8 e aos módulos seguintes.

## 1. Percurso de uso

1. Entrar ou criar a conta em `/acesso` e confirmar o e-mail. Quem já possui conta usa a mesma identidade.
2. Ativar e confirmar a verificação em duas etapas para administrar uma empresa.
3. Abrir **Empresas**, criar a empresa quando necessário e selecioná-la explicitamente. Criar uma empresa ou aceitar um convite não troca a seleção existente.
4. Na empresa ativa, consultar a própria participação. Administradores com segunda etapa confirmada também gerenciam membros, convites, solicitações e exportações.

A empresa ativa aparece no cabeçalho da página. A troca exige confirmação quando há campos, edições ou códigos que seriam descartados. A página limpa filtros e dados do contexto anterior. Códigos e rascunhos não são colocados na navegação, no histórico, no armazenamento persistente do navegador ou no HTML inicial. Consultas e downloads usam o identificador da empresa e o filtro nos parâmetros da requisição; essas referências continuam sujeitas à autorização.

## 2. Papéis desta etapa

| Papel | Acesso entregue |
|---|---|
| Administrador | Gerenciar o nome da empresa, membros, convites, pedidos de acesso e consulta/exportação da equipe, com verificação em duas etapas confirmada na sessão |
| Membro | Selecionar a empresa, consultar o próprio vínculo e encerrá-lo |

Os papéis de obra, segregação entre quem executa/confere/aprova/paga e concessões específicas são entregas posteriores. O nome **Membro** não significa acesso a todas as obras.

O administrador inicial é a pessoa que criou a empresa. Administradores não têm término automático de validade. Antes de retirar o último administrador ou reduzir seu papel, é necessário atribuir a administração a outra pessoa com segunda etapa ativada. Essa proteção vale também quando duas alterações ocorrem simultaneamente.

Membros comuns podem ter validade definida. A interface identifica o horário de Brasília ao informar o fim do dia; o servidor recebe o instante com fuso explícito e o banco armazena o instante. Vínculo vencido ou encerrado perde acesso na próxima operação autorizativa. O histórico permanece.

## 3. Convites

O administrador informa o e-mail, o papel, a validade do convite e, quando aplicável, a validade do vínculo. O prazo do convite varia de **1 a 168 horas**. A interface sugere 72 horas.

O código completo é mostrado somente na resposta à criação. Ele deve ser copiado e enviado pelo administrador ao destinatário. **Esta etapa não envia o convite por e-mail automaticamente.** A fila geral de notificações pertence à issue #15. A mensagem da interface distingue criação do convite de envio da mensagem.

O destinatário entra com a conta do mesmo e-mail verificado, cola o código, confere empresa/papel/validade e aceita. Não é necessário cadastrar outra conta. Aceitar um convite administrativo exige segunda etapa confirmada.

Regras aplicadas:

- Código com 256 bits aleatórios, vinculado à empresa e à finalidade. O banco guarda somente seu resumo criptográfico.
- Convite expirado, revogado, consumido ou destinado a outro e-mail não concede vínculo nem revela os dados da empresa.
- O código de pedido de acesso não pode ser usado como convite.
- Um novo convite para o mesmo e-mail invalida os anteriores ainda pendentes.
- Aceitações concorrentes produzem no máximo uma alteração efetiva.
- Pessoa com vínculo ativo não é promovida silenciosamente por convite. Alterar seu papel exige a operação de gestão de membros.
- Encerrar ou alterar um vínculo revoga os convites pendentes anteriores da pessoa. Reapresentar convite consumido não a reativa.

Há até 500 convites pendentes por empresa. O administrador deve concluir ou revogar convites antes de criar outros quando esse limite for atingido. A consulta prioriza os convites pendentes e mostra até 500 registros, incluindo histórico quando houver espaço.

## 4. Pedidos de acesso

Ao criar a empresa ou gerar outro código, o administrador recebe um código privado de solicitação de acesso. Esse código pode ser compartilhado com alguém que precise pedir um vínculo. Gerar outro código invalida o anterior para novas solicitações.

A pessoa autenticada informa o código e o papel solicitado. O pedido não concede direitos nem passa a listar a empresa. Um administrador confere a solicitação e aprova ou recusa. A decisão identifica o solicitante, o papel e quem decidiu. Aprovar administração exige que o destinatário já tenha ativado o segundo fator; as operações administrativas ainda exigirão uma sessão que o confirmou.

Pedidos repetidos enquanto há uma solicitação pendente não duplicam a pendência. O limite de 500 solicitações pendentes mantém todas as decisões necessárias disponíveis na consulta. Quando o limite é atingido, novas solicitações recebem orientação para procurar o administrador, sem expor contagens da empresa. Encerrar um vínculo também encerra pedidos antigos pendentes daquela pessoa.

## 5. Busca, exportação e recuperação de falhas

A busca de membros é feita no servidor, dentro da empresa autorizada, antes de ordenar, paginar ou contar. A consulta inicial mostra até 100 membros; a busca usa páginas de até 100. O membro comum não recebe a lista administrativa.

O arquivo TSV identifica empresa, revisão, filtro, total e instante de emissão. A exportação verifica a sessão, a seleção e o papel na mesma transação da leitura. Ela permite até 10 mil membros por arquivo e pede um filtro mais restrito acima disso; nunca apresenta um arquivo truncado como completo. Células iniciadas por operadores de planilha recebem proteção contra interpretação como fórmula.

Falhas de validação ou de rede conservam os campos. Uma versão antiga exige atualizar os dados e conferir a alteração antes de reenviar. A página não repete operações automaticamente.

Ao expirar a sessão, os dados privados deixam de aparecer. Campos não secretos podem permanecer suspensos apenas na memória da aba, vinculados à pessoa e à empresa. Só voltam depois de confirmar a mesma identidade e a mesma empresa ainda autorizada. Outra conta, empresa ou revogação descarta esse rascunho. Códigos de convite/acesso não são preservados nessa suspensão. Recarregar ou fechar a página pode perder o trabalho ainda não enviado.

Respostas atrasadas de pesquisa ou comandos não repõem dados de uma empresa anterior. A atualização de acesso também descarta resultados administrativos antigos quando a pessoa perde a administração, mesmo que a empresa continue selecionada.

## 6. Contrato HTTP

| Método e caminho | Finalidade |
|---|---|
| `GET /api/organizations/context` | Lista de empresas autorizadas e estado da seleção da sessão |
| `POST /api/organizations/command` | Criação, seleção, membros, convites e pedidos |
| `GET /api/organizations/{id}/members` | Busca e paginação de membros para administradores |
| `GET /api/organizations/{id}/members.tsv` | Exportação autorizada da busca |

O contrato versionado está em `crates/contracts/src/organizations.rs`. Toda mutação sobre a empresa ativa inclui `organization_id`, `context_version` e `organization_version`. Alterações de membro, convite ou pedido também incluem a versão do registro. Consultas e exportações recebem a empresa e a versão do contexto e leem a revisão atual no banco. Seleções de outra aba invalidam o contexto antigo.

A camada geral de chaves de operação idempotente pertence à issue #9. Esta etapa impede reutilização de convites, duplicação de pedidos pendentes e repetição de mutações com versão antiga. Criar empresas ainda não usa essa chave geral: se a resposta se perder, atualize a lista antes de repetir a criação, conforme orientado pela interface.

A identidade vem do cookie HTTPOnly da issue #6. A API não aceita usuário ou privilégios informados no corpo como credencial. Mutação exige origem completa correspondente e token CSRF. Consultas privadas também recusam origem estrangeira e não podem ser armazenadas em cache. Erros não expõem SQL, segredos, nomes ou contagens de outra empresa.

## 7. Persistência e isolamento

Aplicar a migração `0004_organizations_and_memberships.sql` pelo procedimento já documentado em [Persistência](persistencia.md). Não há variável secreta nova nesta etapa. O papel de migração permanece distinto do papel de execução.

- Todas as tabelas de empresa usam RLS habilitado e obrigatório. O papel operacional não é proprietário e não tem `BYPASSRLS`, privilégios de administração ou capacidade de alterar a auditoria.
- O contexto de pessoa/empresa é local à transação e é limpo antes/depois do empréstimo da conexão. O diretório sem empresa selecionada permite ler apenas os próprios vínculos.
- A seleção pertence à sessão autenticada e tem geração. Selecionar uma empresa não dispensa revalidar o vínculo em cada operação.
- A sessão, o vínculo, os papéis e as versões são conferidos na transação que grava. Empresa e referência não autorizadas são recusadas antes de bloquear a empresa alheia. As guardas são repetidas após adquirir os bloqueios.
- As organizações necessárias a uma operação são bloqueadas em ordem estável. A identidade usa `FOR NO KEY UPDATE` para continuar serializando credenciais sem formar ciclo com chaves estrangeiras de membros.
- A proteção do último administrador também existe no banco. Ela impede alterações da identidade do vínculo e preserva ao menos um administrador nas corridas testadas em `READ COMMITTED` e `REPEATABLE READ`.
- Criação de empresa, primeiro administrador e auditoria fazem parte da mesma transação. Uma falha não deixa empresa órfã.

A migração preserva organizações legadas da fundação e não inventa vínculos para pessoas. A fundação anterior não oferecia cadastro comercial de empresas. Dados existentes só devem receber associações por procedimento administrativo autorizado, com identificação e evidência próprias.

### Recursos fora do banco

`crates/application/src/organization_resources.rs` define referências e chaves por empresa para objetos, cache, busca, filas e exportações. A fronteira é criada internamente após autorização. As chaves incluem os identificadores e versões aplicáveis; o cache também separa pessoas. Nenhuma chave serializada concede acesso.

Busca e exportação de membros já usam essa fronteira. Os adaptadores de arquivos, cache externo, índice de busca externo e processamento geral ainda pertencem aos módulos consumidores. Os testes desta etapa conferem seus contratos de isolamento, sem afirmar que esses provedores já foram implantados. O consumidor deve reautorizar antes de executar e antes de entregar o resultado de uma tarefa.

## 8. Rastreabilidade e verificação

| Requisito | Implementação nesta etapa |
|---|---|
| OP03 | Pedido identifica pessoa, empresa, papel solicitado e decisão; convite tem destino e concessão explícitos |
| ORC03 | Diretório próprio, autorização antes de consulta/contagem/exportação e recusa segura de referências cruzadas |
| BR05 | RLS e transações com empresa, interfaces de objetos/cache/índices/filas, busca/exportação protegidas |
| UX02 | Empresa visível, escolha explícita, geração de contexto e descarte controlado de dados incompatíveis |

As regras de origem e as adaptações continuam na [matriz](MATRIZ-RASTREABILIDADE.md). Decisões de MFA, códigos, limites e isolamento são decisões do produto; não são atribuídas ao SICOP.

O [registro da issue #7](validacao/ISSUE-007.md) identifica a revisão, os comandos, as capturas e os resultados efetivamente observados. Os testes PostgreSQL são executados explicitamente; não são substituídos por testes unitários. A CI também executa as regressões de autenticação, a fronteira HTTP e o navegador real.

Referências técnicas consultadas: [bloqueios do PostgreSQL 17](https://www.postgresql.org/docs/17/explicit-locking.html) e [políticas de segurança por linha](https://www.postgresql.org/docs/17/ddl-rowsecurity.html).

## 9. Integração

As PRs #75 a #78 foram integradas em `main` em 13/11/2026, conforme o [registro de integração](validacao/INTEGRACAO-003-006.md). A PR #79 passa a usar `main` como base e preserva o código aprovado na CI. O teste da issue #3 com pelo menos três profissionais reais continua pendente. A integração técnica anterior não elimina esse critério nem autoriza por si só o merge desta PR. Esta entrega não fecha automaticamente a issue #7 e não representa implantação em produção nem aceite comercial da issue #72.
