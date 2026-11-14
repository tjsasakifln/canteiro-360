# Histórico de contratos e gravação segura

Em `/historico`, selecione a empresa ativa e um contrato que você pode consultar. O histórico registra o cadastro e as revisões do título da referência contratual. Cada linha identifica o autor, a origem, a versão, o motivo e os títulos anterior e salvo. O relatório CSV usa a mesma ordem de registro. Uma alteração informada depois da data em que ocorreu mostra as duas datas sem mudar a posição das revisões anteriores.

Um gestor precisa das permissões **Consultar** e **Editar referência do contrato** naquele contrato. A reserva de edição vale dois minutos para a sessão e a aba que a obtiveram. A renovação é explícita. Salvar libera a reserva. Se a sessão cair, a reserva expira e outra sessão pode continuar. Uma sessão revogada deixa de manter a reserva.

O rascunho mostra a versão de partida, o autor e o momento da edição. Ele fica associado à conta, à empresa e ao contrato na aba autenticada. Quando há conflito, compare a versão de partida, o rascunho e a versão salva. Para reaplicar, confirme a nova base antes de gravar. Se a resposta se perder, consulte a tentativa pelo botão de recuperação. Uma revisão já gravada devolve o mesmo recibo. Se ainda estiver pendente, a tela permite reenviar a mesma tentativa.

## Registro e reexecução

- A revisão confere sessão, seleção de empresa, participação, vigência e permissões explícitas. O bloqueio da organização serializa mudanças de acesso e o operador SQL só altera a referência através da função que também verifica o contrato, a reserva e a versão esperada.
- A alteração, a próxima revisão, o recibo da operação e a pendência de notificação são confirmados na mesma transação. A identidade da operação é a empresa, o ato e uma chave UUID v4. O hash usa dados de negócio tipados e normalizados, incluindo objeto e versão esperada. O recibo é derivado do fato confirmado. O servidor verifica o acesso e o autor antes de devolver uma tentativa anterior.
- O histórico cadastral e a trilha de alçadas são somente de inserção. O usuário de execução recebe privilégios mínimos e a conexão recusa privilégios de alteração indevidos, inclusive por coluna. O autor é preservado quando o vínculo é desativado. O histórico não recebe contexto de sessão, chaves, credenciais ou corpos completos dos comandos.
- Pedidos e decisões de alçada também aceitam a chave `Idempotency-Key`. A autorização monetária final possui identidade única por empresa, pedido e versão, independente da chave da tentativa. Antes e depois apontam para as versões imutáveis de onde vêm os valores exatos. As decisões anteriores à migração permanecem nas versões dos pedidos, nas assinaturas internas e na auditoria da empresa. A alçada confirma a autorização para aquela versão. Fatura, liquidação e recebimento pertencem aos seus módulos.
- Para recuperação após perda de resposta, o histórico consulta `GET /api/reliability/contracts/{id}/operations/{key}`. Pedidos e decisões consultam `GET /api/access/operations/{ato}/{key}` com `authority_propose`, `authority_revise` ou `authority_decide`. As duas rotas são privadas e exigem a empresa ativa e a versão atual do contexto. A chave é um identificador opaco.

## Consumidor de notificações

Após aplicar as migrações, execute `target/release/canteiro-jobs` em processo separado com `CANTEIRO_ENV` e `CANTEIRO_DATABASE_URL` do ambiente. A credencial de migração fica no processo administrativo. O comando `--check` verifica o executável sem iniciar a fila.

O consumidor escolhe um item pendente por transação. A notificação interna ao autor e a confirmação de entrega são gravadas juntas, com unicidade por empresa e evento. Uma interrupção antes do commit deixa o item disponível para nova execução. Dois consumidores podem trabalhar simultaneamente. Em caso de erro, o item é adiado com intervalo progressivo de um a sessenta segundos. Avisos de falha do consumidor e de indisponibilidade do banco são registrados sem dados da obra. Para investigar uma ocorrência, a identidade administrativa pode conferir os horários e as tentativas em `canteiro_meta.contract_notification_outbox`. O consumidor recomeça pela fila persistida após reiniciar.

A API continua servindo o histórico durante uma interrupção do consumidor. Notificações desta etapa aparecem no próprio histórico, depois de revalidar a consulta ao contrato. Sistemas externos precisarão aplicar a chave do evento no destino para suportar reentregas.

## Migrações

As migrações 0006 a 0008 aplicam o histórico, as reservas, os recibos, a fila e a trilha monetária. A linha inicial de uma referência contratual existente conserva título, versão, autor e instante de criação, além da data em que entrou no histórico. O servidor e o consumidor devem usar o esquema da mesma versão. Em caso de incidente, preserve a fila e os registros confirmados ao corrigir a aplicação. Uma restauração de banco exige reconciliação dos fatos gravados após o ponto restaurado.

Referências de aceite no repositório [REL02](regras/orcamentos-medicoes.md#rel02), [BR09, BR18 e UX04](MATRIZ-RASTREABILIDADE.md) e [issue 9](issues/009.md).
