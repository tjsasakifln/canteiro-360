# Permissões, responsabilidades e alçadas

Implementação da issue [#8](https://github.com/tjsasakifln/canteiro-360/issues/8). Página: `/permissoes`. A base é a integração técnica da [PR #79](https://github.com/tjsasakifln/canteiro-360/pull/79).

## Uso

1. Entre na conta e escolha a empresa em **Empresas**. A empresa ativa aparece no início da página.
2. Um administrador com verificação em duas etapas identifica a obra, os contratos ligados a ela e os orçamentos individuais de cada contrato. Essas referências organizam o acesso. Não formalizam contratos nem substituem o cadastro completo das etapas posteriores.
3. Conceda à pessoa um vínculo específico: obra, contrato ou orçamento individual, função, tarefas, limite monetário por ato, início e término. A pessoa deve participar da empresa. Acesso à obra não concede acesso automático a seus contratos; acesso ao contrato não libera seus orçamentos. O orçamento tem referência própria e depende de concessão expressa.
4. Configure a regra para aprovação de orçamento, alteração, autorização de pagamento ou aceite técnico. Defina o valor a partir do qual serão exigidas duas pessoas e se o proponente fica impedido de aprovar.
5. A pessoa autorizada propõe um pedido com acréscimo e redução separados. Os responsáveis conferem a versão, os valores e os impedimentos antes de decidir. O pedido registra alçada; não executa pagamento, mede serviços ou altera um contrato.

Concessão, delegação, encerramento de direitos e decisão exigem confirmação em duas etapas. Os comandos sensíveis apresentam uma conferência antes da gravação. Falhas preservam campos compatíveis da mesma pessoa e empresa. Após perda de resposta, confira os registros antes de repetir uma criação: a chave geral de idempotência pertence à issue #9.

Campos ainda não gravados são protegidos nas trocas de formulário e na navegação interna. Retomar a página exige consultar novamente a sessão, a empresa e os direitos antes de restaurar o rascunho compatível. A conservação na memória da aba não representa gravação no servidor.

## Papéis e tarefas

O papel delimita tarefas possíveis; cada vínculo concede expressamente um subconjunto, incluindo consulta. Um papel sem vínculo não concede direito.

| Papel | Tarefas que podem ser concedidas |
|---|---|
| Leitor e cliente | Consulta |
| Gestor | Consulta e proposta de alteração |
| Orçamentista | Consulta, edição e conferência de orçamento como atos separados, e proposta de alteração |
| Fiscal | Consulta, registro de execução, conferência de medição e proposta de alteração |
| Responsável técnico | Tarefas do fiscal e aceite técnico |
| Aprovador | Consulta, aprovação de orçamento, alteração ou autorização de pagamento |
| Financeiro | Consulta e registro de pagamento |
| Fornecedor | Consulta, registro de execução e proposta de alteração |
| Suporte | Consulta, obrigatoriamente com término |

Administrador configura direitos, mas não recebe função técnica ou aprovação financeira automaticamente. A consulta aos valores dos pedidos exige vínculo no escopo exato da obra, do contrato ou do orçamento. Funções distintas exigem vínculos pertinentes; múltiplos papéis não anulam conflito de interesse.

## Valores, política e conflito

Valores usam `Money`, são transmitidos como texto e rejeitam mais de duas casas decimais. Acréscimo e redução são magnitudes não negativas independentes. A maior delas define a alçada. Acréscimo de R$ 100,00 e redução de R$ 100,00 exigem alçada de R$ 100,00 e continuam contendo acréscimo. O saldo líquido não reduz a exigência.

O limiar é inclusivo: ao atingir o valor configurado, vale a quantidade definida de uma ou duas aprovações. Abaixo dele, é necessária uma aprovação. Cada pessoa precisa de alçada suficiente para todo o valor; limites individuais não são somados.

Conflitos explicitamente declarados sempre impedem a pessoa e o titular de sua delegação. A opção de segregação acrescenta o proponente ao impedimento. Quando a segregação é exigida, delegar a própria alçada não permite aprovar a própria proposta por intermédio de outra pessoa.

Dupla aprovação exige pessoas com poderes independentes. Duas assinaturas da mesma pessoa, dois delegados do mesmo titular ou titular e delegado não atendem à exigência. Ao completar o quórum, são revalidadas as concessões atuais das assinaturas pendentes, inclusive fonte da delegação e participação na empresa.

## Delegação e responsabilidade contínua

O titular de concessão direta, vigente e confirmado em duas etapas pode delegar para outra pessoa ativa da mesma empresa. A delegação conserva o escopo exato da obra, do contrato ou do orçamento, restringe-se às tarefas concedidas, não aumenta o valor e termina dentro da validade da fonte. O motivo é obrigatório e permanece no vínculo e na auditoria. Vínculos restritos à consulta não podem delegar. Não há redelegação nesta etapa.

Substituição identifica uma delegação durante ausência do titular. Não cria segunda autoridade independente. O prazo efetivo considera também a participação na empresa de quem atua e do titular da fonte. Encerrar ou vencer a concessão original ou uma dessas participações interrompe os direitos dependentes na próxima operação, inclusive com a sessão já aberta.

Gestor, responsável técnico ou aprovador podem ser necessários continuamente. O titular e sua participação devem estar ativos, sem vencimento automático. Antes de retirar o último titular, cadastre sucessor direto que cubra cada tarefa e limite obrigatório. Mesmo papel com apenas consulta, valor menor, início futuro ou prazo de término não cumpre a sucessão. A proteção alcança encerramento do membro e operações concorrentes.

## Versões e histórico

Cada comando confere empresa selecionada, versão do contexto e versão da organização. Delegações, revogações e decisões conferem também versões do vínculo e do pedido. A identidade vem da sessão, nunca do corpo enviado pelo navegador.

O pedido guarda proponente, versão, valores, conflitos e fotografia da política. Nos módulos consumidores, autores e conferentes devem ser identificados a partir dos registros do servidor e inseridos nos impedimentos exigidos pela política; uma lista enviada pelo cliente não pode omitir participantes efetivos. Revisar insere nova versão; decisões anteriores continuam no histórico, mas não aprovam a revisão. Política alterada exige revisar pedidos pendentes. Decisões definitivas permanecem históricas e não podem ser reescritas por revisão.

Versões de pedidos e assinaturas são somente de inserção para a identidade de execução. Configuração e decisões geram auditoria na mesma transação. O bloqueio comum da organização serializa concessão, revogação, sucessão, participação e decisão.

## Integração posterior

`canteiro_domain::access` concentra a política pura. O chamador deve fornecer identidade autenticada, hora do banco, escopo do registro real, valores e versão obtidos no servidor e vínculos atuais. `AuthorizedAccess` é transitório; não deve vir do cliente, funcionar como senha ou ser reaproveitado em outra transação.

Os módulos de orçamento, contrato, medição e financeiro deverão ligar seus registros às referências corretas e autorizar a escrita na mesma transação. Pedido de alçada não permite trocar objeto, versão ou valor. Vínculo de orçamentista usa a referência individual do orçamento, seu contrato e o período autorizado. O módulo consumidor deverá conferir a associação do orçamento real e do gestor, sem aceitar o ID de outro orçamento do mesmo contrato. Os cenários compostos também pertencem à issue #70.

Nesta etapa há limites explícitos de 500 referências, 2000 vínculos históricos e 1000 pedidos por empresa. Os conjuntos de alçada não são truncados silenciosamente. Paginação e retenção deverão preceder a operação comercial em escala.

## Rastreabilidade

| Regra | Implementação |
|---|---|
| OP05 | Consulta distinta de alteração, função e atos explícitos por escopo |
| ORC01 | Consulta, edição, conferência e aprovação de orçamento são atos separados e concedidos expressamente |
| ORC05 | Vigência, encerramento e revalidação do vínculo |
| ORC26 | Orçamento individual ligado ao contrato, finalidade e período; o módulo consumidor confirma o registro real e o gestor |
| BR06 | Limites, delegação, substituição e sucessor suficiente |
| BR07 | Proponente, conflitos, poderes independentes na dupla aprovação e motivo preservado da delegação ou substituição |

As fontes e adaptações estão na [matriz](MATRIZ-RASTREABILIDADE.md), no [PRD](../PRD.md) e nas regras de [operação](regras/operacao.md) e [orçamentos](regras/orcamentos-medicoes.md). Maior magnitude, ausência de redelegação e de herança automática são decisões conservadoras do produto, não afirmações sobre o algoritmo interno do SICOP.

O [registro de validação](validacao/ISSUE-008.md) identifica resultados e limites. O teste humano da issue #3 e a demonstração com gerenciador de senhas da issue #6 permanecem pendentes. Integração técnica não equivale à prontidão comercial da issue #72.
