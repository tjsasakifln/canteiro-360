# Modelo de dados e estados

Modelo lógico obrigatório para orientar as issues. Não é um esquema já implantado. A implementação pode ajustar nomes físicos, preservando relações, significado, integridade e critérios de aceite.

## 1. Identidade e integridade

Entidades de cliente possuem identificador opaco, `tenant_id`, versão, autoria e instantes de criação/alteração. Relações entre entidades incluem a empresa na integridade referencial. Datas de competência, início e fim são datas civis; auditoria usa instante UTC e fuso de apresentação. Identificação brasileira é texto normalizado, preservando CNPJ alfanumérico.

Documento consolidado tem revisão imutável e hash. Registro de negócio guarda `status`, e não depende de o usuário localizar um PDF para saber se está aprovado. Um evento explicita qual revisão e quais valores foram decididos.

## 2. Entidades

| Entidade | Campos essenciais e vínculos | Integridade principal |
|---|---|---|
| Organização | Nome, identificador, plano, políticas, fuso padrão | Fronteira de dados; sem visibilidade cruzada implícita |
| Usuário | Identidade de acesso, autenticação, situação | Separado do profissional e da pessoa contratada |
| Vínculo de organização | Usuário, papel, validade e permissões | Revogação vale na próxima operação autorizativa |
| Concessão de escopo | Obra/contrato/ativo, função, vigência | Não amplia os direitos da organização |
| Política de alçada | Tipo de ato, valores, grupos, segregação, versão | Não compensar acréscimo para escapar de alçada |
| Pessoa/empresa | Cliente, fornecedor, dados de contato e cadastro | Unicidade por identificador dentro da organização |
| Profissional | Pessoa, conselho, registro, UF e função | Não exigir que profissional seja usuário do serviço |
| Designação/ART/RRT | Profissional, escopo, tipo, vigência, documento | Registro por disciplina/ato; upload não comprova validação externa |
| Empreendimento | Título, cliente, centros de custo e localização | Agrupa várias obras e contratos |
| Obra | Empreendimento, ativo, tipo, dimensões e locais | Identidade distinta de contrato ou projeto |
| Ativo/trecho | Endereço, município, coordenadas, propriedade/uso | Localização privada com origem conhecida |
| Projeto | Obra opcional, disciplina, objetivo, gestor e entregas | Pode anteceder obra e contratação |
| Entrega/revisão | Projeto, código, revisão, documento e decisão | Uma revisão emitida vigente por código/escopo |
| Distribuição | Revisão, destinatário, data, confirmação | Versão enviada não muda ao substituir arquivo |
| Tarefa/dependência | Projeto, duração, calendário, recurso e predecessora | Grafo sem ciclos; previsão separada da base aprovada |
| Solicitação técnica | Pergunta, contexto, responsável, prazo e resposta | Encerrar não muda contrato automaticamente |
| Risco/problema/mudança | Causa, impacto, responsável, decisão e ação | Efeito contratual depende do ato correspondente |
| Catálogo de custos | Fonte, competência, localidade, regime, licença | Nova edição não altera contratos anteriores |
| Insumo/composição | Unidade, categorias, coeficientes, fórmula e origem | Sem ciclos; entradas decimais e memória de cálculo |
| Orçamento | Escopo, elaborador, RT, organização e gestores | Vínculo externo limitado ao orçamento autorizado |
| Variação de escopo | Orçamento e subconjunto de obra | Não confundir etapa da obra com nova variação |
| Revisão de orçamento | Variação, sequência, base de preços e situação | Temporal; congelada quando aprovada/vinculada |
| Linha orçamentária | Revisão, grupo, etapa, serviço, unidade, quantidade e preço | Ordenação estável; total derivado pela política |
| Cronograma | Origem, revisão, calendário, grupos e períodos | Percentuais fecham 100%; histórico preservado |
| Cotação/proposta | Revisão de escopo, fornecedor, preços, prazo, validade | Sigilo entre concorrentes e decisão rastreável |
| Contrato | Partes, perspectiva, tipo, regime, início/fim de vigência, início/fim de execução, prazo original/unidade, dias equivalentes, preço e políticas | Execução contida na vigência; base original separada de total atualizado |
| Obra do contrato | Contrato, obra, orçamento/variação e rateio | Relação muitos-para-muitos sem duplicar custo |
| Item/segmento econômico | Escopo, origem, quantidade, preço, data e saldo | Segmento já medido não é reprecificado |
| Etapa global | Grupo, itens, valor-base, percentuais | Sem somar rateio interno ao valor da etapa |
| Ordem de início | Contrato, escopo, data, responsável e documento | Efeito idempotente; identidade de versão |
| Diário/foto/ocorrência | Obra, dia, local, autor, vínculo e anexos | Correção cria revisão; foto mantém original |
| Advertência/qualidade | Requisito, achado, prazo, resposta e aceite | Comunicação formal vinculada à versão enviada |
| Paralisação/retomada | Contrato/frente, intervalo, causa, responsável, percentual físico, ordem e decisão | Intervalos não duplicam dias; execução bloqueada no escopo parado |
| Registro de ajuste de prazo | Pausa/reinício, memória de dias, datas anteriores/novas, documento, aprovação e cronograma | Equivalente privado ao apostilamento; só consolida execução e nunca estende vigência |
| Medição | Contrato, sequência, competência, tipo e versões de referência | Última elegível; períodos e saldos conferidos |
| Item de medição | Segmento, acumulado anterior/atual, delta e valores | Saldo e total calculados, não editáveis como atalho |
| Comissão/avaliação | Medição, obra, profissional, função, período, item e resultado | Avaliação técnica não equivale a aprovação financeira |
| Resultado de cálculo | Entradas, política, versão, detalhamento, hash | Imutável ao consolidar; execução reproduzível |
| Índice/reajuste | Fonte, competência, base, tipo e valor reconhecido | Complemento é diferença; correção mantém anterior |
| Pré-aditivo/variação | Escopo, deltas, justificativa, comissão, situação | Versão proposta distinta de instrumento assinado |
| Aditivo | Proposta liberada, documento, data eficaz e efeitos | Um instrumento não aplica duas vezes a proposta |
| Lote de supressão | Medição, itens, reconhecimentos originais, segmentos, índices originais, razões, decisão e anulação | Aprovação integral do lote e versão exata |
| Origem financeira | Tipo: serviço liberado, compromisso direto, autorização de adiantamento, retenção liberada ou ajuste; alçada, saldo, evidência e contraparte | Uma origem econômica não é reconhecida duas vezes; guardas específicas por tipo |
| Parcela liberada de serviço | Medição/reajuste, origem financeira, perspectiva, valor e saldo | Uma origem de faturamento do serviço pode alimentar várias faturas pelo saldo; não cria caixa |
| Fatura/documento fiscal | Parte, número, emissão, referência, valor e anexos | Identidade fiscal e origem rastreáveis |
| Título/alocação | Origem financeira tipada, fatura opcional conforme natureza, vencimento, valor e situação | Soma alocada não excede origem autorizada; adiantamento não exige medição fictícia |
| Adiantamento | Parte, contrato, pagamento e saldo de crédito | Amortização não cria segunda saída de caixa |
| Retenção/garantia | Base, natureza, taxa, condição de liberação e saldo | Separada de tributo, glosa e caixa realizado |
| Liquidação/conciliação | Título, valor, data, conta, origem, comprovante | Duplicata de evento bancário não repete baixa |
| Estorno/compensação | Evento original, motivo, valor e aprovador | Soma reversível limitada ao fato ainda válido |
| Compromisso direto | Obra, fornecedor, centro de custo e aprovação | Não duplicar item já coberto por contrato |
| Encerramento técnico sem cobrança | Contrato, competência, conclusão, comissão, saldo de escopo, termo e última medição financeira opcional | Não cria parcela faturável; preserva saldo financeiro |
| Referência de encerramento | Discriminante: medição final financeira ou encerramento técnico sem cobrança; data final | Exatamente uma origem válida do mesmo contrato |
| Recebimento | Tipo, contrato, referência de encerramento, termo, comissão e pendências | Data não antecede referência; definitivo exige provisório |
| Garantia de obra | Entrega, obrigação, prazo, responsável e chamada | Registro não promete prazo legal universal |
| Encerramento | Contrato, motivo, data, instrumento e saldos | Não apaga passivo nem força quitação |
| Acordo de manutenção | Contrato, lote, catálogo, BDI/desconto, limite, consumido, reserva remanescente e disponível | Disponível = limite - consumido - reserva; confirmação técnica transfere reserva em consumo uma vez |
| Pedido/OS de manutenção | Ativo, necessidade, escopo, alçada, quantidade e decisão | Reserva, execução e medição são efeitos separados |
| Assinatura do SaaS | Organização, plano, vigência, provedor e permissões | Independente dos pagamentos de obras |
| Cobrança do SaaS | Assinatura, evento externo, valor, documento e conciliação | Evento repetido/fora de ordem não concede plano indevido |
| Chamado | Solicitante, assunto, escopo, prioridade, histórico e anexos | Suporte só vê conteúdo autorizado |
| Evento de auditoria | Ator, organização, entidade, versão, ato, motivo e antes/depois | Sem atualização pelo usuário operacional |
| Trabalho em fila | Organização, tipo, operação, estado, tentativas e resultado | Reexecução idempotente e dados isolados |

## 3. Estados e transições

### Orçamento

| Origem | Ação | Destino | Guarda |
|---|---|---|---|
| Em conferência | Conferir e finalizar | Finalizado | Estrutura, cálculos, cronograma, elaborador e responsabilidade válidos; ART/RRT aplicável pode estar pendente |
| Finalizado | Devolver para ajuste | Em conferência | Motivo e preservação da versão revisada |
| Finalizado | Aprovar | Aprovado | Alçada, versão, segregação e ART/RRT aplicável válidos, conforme SIC-048 p.4 |
| Aprovado | Desaprovar | Em conferência | Nenhum contrato vinculado; motivo e auditoria |
| Aprovado vinculado | Alterar escopo ou preço | Nova revisão | Original permanece íntegro |

### Contrato

Situação de execução (`A iniciar`, `Em andamento`, `Paralisado`, `Concluído`, `Pendência administrativa`, `Encerrado`) é distinta do estado do instrumento (`Rascunho`, `Formalizado`, `Substituído`) e do saldo financeiro. Paralisação e encerramento decorrem de registros próprios. Pendência administrativa mantém motivo, autor e histórico de eventos repetidos; não altera quantidades, libera medição ou quita títulos por si só. Conclusão manual permitida pela referência tem caminho autorizado com justificativa, evidência e reconhecimento dos saldos; não contorna recebimento ou libera pagamento final. Medição final calculada pode concluir execução sem concluir a liberação financeira.

### Medição

Usar dimensões explícitas para evitar a ambiguidade de nomes históricos:

| Dimensão | Estados | Efeito |
|---|---|---|
| Elaboração | Em conferência, calculada definitiva, anulada | Define editabilidade e snapshot |
| Validação técnica | Pendente, com divergência, conferida | Registra evidências e comissão |
| Aprovação | Pendente, aprovada, rejeitada | Decisão da versão e valores exatos |
| Liberação | Bloqueada, aguardando recebimento provisório, liberada | Define elegibilidade de faturamento/pagamento |

Para parcial: conferência válida, cálculo definitivo e aprovação competente precedem liberação. Para final: acrescentar termo provisório assinado e ação explícita de liberação. A interface apresenta um resumo coerente e permite ver cada dimensão; não usa múltiplos rótulos contraditórios sem explicação.

Só a última medição, sem sucessora consolidada, parcela liberada, fatura, baixa ou correção dependente, pode voltar à conferência mediante autorização. Bloquear a cadeia na transação para impedir sucessora concorrente. Invalidar seus cálculos, reajustes, aprovações e relatórios da revisão. Havendo sucessora ou efeito financeiro, corrigir na última competência por operação vinculada e compensação, sem reabrir o fato anterior. A anulação registra os efeitos dependentes e não os apaga silenciosamente.

### Pré-aditivo e aditivo

Pré-aditivo: `Em conferência` → `Aguardando aprovação` quando a alçada exigir → `Liberado` → `Incorporado`. Rejeição mantém a versão e permite nova variação. O termo `Aprovado` do pré-aditivo legado após incorporação corresponde ao vínculo efetivo ao aditivo, separado do aceite da proposta.

Aditivo: `Em preparação` → `Formalizado` → `Aplicado`. Formalização identifica instrumento e data; aplicação ocorre uma vez na data eficaz válida. Cancelamento anterior à aplicação não altera saldos. Desfazer efeito aplicado é outro ato controlado e não exclusão.

### Paralisação e registro de ajuste de prazo

Ordem de pausa formalizada suspende autorização de atos de execução no escopo; atos administrativos permanecem disponíveis. Ocorrência factual de execução irregular é registrável, sem autorizar execução ou liberação financeira automática. Ordem de reinício válida é condição para retomar. O registro equivalente ao apostilamento percorre `Em preparação` → `Aprovado` → `Aplicado`, sempre vinculado às ordens, memória e cronograma. Sua aplicação é única e não muda a vigência. Se o novo término ultrapassa a vigência, aguardar aditivo de vigência e cronograma compatível antes de consolidar/retomar. Pausa parcial não muda automaticamente o estado de execução das outras frentes.

### Supressão medida

`Em conferência` → `Aguardando aprovação` → `Aprovada` ou `Reprovada`; anulação de aprovada passa por solicitação e nova decisão. A decisão sempre cobre o lote inteiro. Mudança de item ou valor cria nova versão e exige novo aceite.

### Manutenção

`Rascunho` → `Em análise da demanda` → `Aprovado para elaborar OS` → `OS em elaboração` → `OS em análise` → `OS autorizada/em execução` → `Execução informada` → `OS concluída`. Análise pode cancelar com motivo ou devolver para adequação. Etapas de medição, faturamento e caixa seguem seus fluxos próprios. Não saltar da aprovação da necessidade para execução sem escopo e saldo aprovados.

A reserva de manutenção é transferida para consumo na confirmação técnica da execução. Divergência conserva reserva; medição/fatura não consomem o limite novamente. Cancelamento parcial libera apenas o remanescente, conforme cálculos §8.1.

### Títulos e caixa

Título pode estar `Em preparação`, `Confirmado`, `Parcialmente liquidado`, `Liquidado`, `Cancelado` ou `Compensado`, conforme eventos. `Vencido` é condição calculada por saldo aberto e vencimento. Retenções não liberadas e adiantamentos não amortizados são saldos distintos. Estorno reabre o valor pertinente sem apagar a liquidação original. A moeda inicial é BRL; não misturar moedas em totalizadores.

## 4. Auditoria e retenção

Registros em rascunho podem ser descartados conforme política, preservando o evento necessário. Registros definitivos, assinados, faturados ou transmitidos mantêm histórico. Eliminação por privacidade segue classificação, prazo, fundamento e bloqueio por obrigação aplicável, sem prometer conservação eterna de todo dado pessoal. O vínculo entre integridade técnica e minimização deve ser tratado na issue 66.

## 5. Dados de teste e importação

Usar duas organizações fictícias com IDs distintos, usuários de múltiplos perfis e contratos que compartilham nomes parecidos para testar isolamento. Importação mantém ID de origem, lote, data, hash e totais de controle. Uma migração histórica não ativa notificações externas nem cobra valores automaticamente. O aceite do lote precede a liberação para novas operações.
