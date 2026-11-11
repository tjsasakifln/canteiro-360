# Contrato de cálculo do Canteiro 360

## 1. Natureza destas regras

Este documento fixa decisões próprias para tornar a implementação determinística. As fontes SICOP documentam quantidades acumuladas, totalizações, reajustes e truncagem, mas não descrevem todos os pontos de cálculo e escala. As decisões abaixo não devem ser anunciadas como reprodução comprovada do algoritmo interno do SICOP. Ver [orçamentos e medições](orcamentos-medicoes.md), [contratos](contratos.md) e [decisões](../decisoes-e-lacunas.md).

Política inicial: `C360-DECIMAL-1`. Cada orçamento aprovado, contrato e cálculo definitivo conserva a versão usada. Mudança futura exige nova versão e migração explícita dos fatos ainda editáveis.

## 2. Representação

| Grandeza | Escala de entrada máxima | Representação persistida | Regra |
|---|---|---|---|
| Quantidade | 6 casas | `NUMERIC(20,6)` | Exigir unidade; positivo ou zero, salvo correção autorizada |
| Preço unitário | 6 casas | `NUMERIC(20,6)` | Preservar precisão contratual; não exibir só duas casas em editor |
| Valor exigível/título | 2 casas | `NUMERIC(20,2)` | Centavos; histórico imutável após consolidação |
| Percentual | 6 casas | `NUMERIC(12,6)` | Faixa depende da função; execução de 0 a 100 |
| Índice/fator | 10 casas | `NUMERIC(20,10)` | Competência, fonte e política vinculadas |
| Intermediário | Até 28 dígitos significativos | Decimal em Rust | Operação verificada, sem saturação ou conversão para float |

As escalas são limites de entrada, não autorização para multiplicar números arbitrariamente grandes. Validar limites de magnitude antes de cada operação e recusar estouro, divisão por zero ou perda de precisão não prevista. Para evitar estouro na aplicação de percentuais, dividir fatores de forma controlada e guardar escala intermediária de 10 casas com arredondamento ao mais próximo, empate afastando de zero, somente onde esta política autoriza. Os valores financeiros exigíveis usam a regra de truncagem descrita a seguir.

Na API, números decimais são textos canônicos com ponto, sem separador de milhar. A tela aceita `1.234,56` conforme `pt-BR` e mostra o valor interpretado. Importação exige selecionar a convenção; `1,234` não deve ser reinterpretado silenciosamente entre três casas decimais e milhar. Dados inválidos são rejeitados antes de gravar.

## 3. Preço, composição, BDI e desconto

Um serviço tem composição calculada ou custo informado, com origem, unidade e vigência. A composição soma coeficiente vezes custo do componente e separa material, mão de obra, equipamento e demais categorias. Fórmulas são expressões restritas, tipadas e versionadas; não executar código ou fórmulas de planilha recebidas. Proibir ciclos entre serviços e incidências.

Decisão inicial para preço negociado por percentual:

`PU = arredondar6(custo_direto × (1 + BDI/100) × (1 - desconto/100))`

`arredondar6` usa empate afastando de zero. BDI e desconto são campos separados, identificados como aplicáveis ou já incorporados. A mesma parcela não pode ser aplicada duas vezes. Se o preço unitário foi negociado diretamente, ele prevalece e sua composição é informação de suporte; não reaplicar BDI sobre esse preço. Quando a base já contém preço de venda, bloquear nova incidência automática até classificação explícita.

Faixas da política inicial: custos, coeficientes e preço negociado ordinários são não negativos; desconto entre 0% e 100%; BDI entre 0% e 1.000%, limite de produto explícito, sem alegar limite legal. BDI negativo exige outra política aprovada, não este campo. Crédito usa registro de ajuste próprio. Desconto de 100% gera item gratuito identificado, sem ser confundido com preço ausente. Aplicar também os limites de magnitude e precisão antes de multiplicar. Desconto de 150%, coeficiente negativo e PU negativo são rejeitados antes de aprovar.

Exemplo: custo R$ 100, BDI 20% e desconto 10% resultam em PU R$ 108,000000. Isso é uma política do produto, não uma afirmação de ordem de cálculo publicada pelo SICOP. Encargos sociais, produtividade, transporte e equipamento possuem memória e parâmetros da fonte utilizada. Não há taxas legais universais embutidas.

## 4. Medição por preço unitário

Para cada item e segmento econômico com preço fixado:

`A_k = truncar2(Q_acumulada_k × PU_segmento)`

`valor_periodo_k = A_k - A_anterior_definitivo`

`quantidade_periodo_k = Q_acumulada_k - Q_anterior_definitiva`

`truncar2` elimina casas além dos centavos em direção a zero. A diferença entre valores acumulados preserva centavos ao longo dos períodos. Não truncar cada quantidade incremental independentemente. O total da medição soma os deltas por item, com reajuste e demais componentes exibidos separadamente.

| Caso | Entrada | Resultado esperado |
|---|---|---|
| MED-A | 100 un contratadas, PU 10; acumulado anterior 20, atual 35 | Período 15 un e R$ 150,00 |
| MED-B | PU 0,335; acumulados 1, 2, 3 | Parcelas 0,33; 0,34; 0,33; acumulado 1,00 |
| MED-C | Acumulado 101 para contrato de 100 | Bloqueio até alteração formal válida |
| MED-D | Acumulado anterior 20, atual 15 sem lote de supressão aprovado | Bloqueio; não salvar correção financeira implícita |
| MED-E | Repetição do mesmo comando definitivo | Mesmo resultado, nenhum segundo consumo de saldo |

Somente a última medição da cadeia, sem sucessora consolidada, parcela liberada, fatura, pagamento ou correção dependente, pode voltar à conferência mediante autorização. Revalidar essas condições na mesma transação que bloqueia a cadeia. Reabertura invalida seu cálculo, reajuste, aprovação e relatórios da revisão. Havendo sucessora, corrigir na última competência por operação vinculada; nunca recalcular retrospectivamente a cadeia. Exemplo: acumulados 20 e 35 a PU 10 reconhecem 200 e 150; tentar reabrir a primeira é bloqueado e a posição de 350 permanece até correção aprovada.

Alteração de preço ou criação de serviço não reprecifica o que já foi consolidado. Um aditivo cria segmento econômico com quantidade inicial, preço, data e saldo próprios. A soma dos segmentos representa o serviço; a tela conserva rastreabilidade por período.

## 5. Medição por preço global

O contrato identifica o regime. Grupo contém etapas e cada etapa contém itens. A etapa guarda valor-base e percentual acumulado. Para valor de etapa `E`:

`G_k = truncar2(E × percentual_acumulado_k / 100)`

`valor_periodo_k = G_k - G_anterior_definitivo`

Etapa de R$ 100.000,00 com avanço de 20% para 45% gera R$ 25.000,00 no período. O valor final a 100% deve igualar o valor-base em centavos. Rateios internos aos itens são de informação e reconciliação, nunca parcelas adicionais a somar ao valor da etapa.

Após a primeira medição, congelar valor-base e composição econômica da etapa. Acréscimo cria etapa/segmento suplementar; redução de escopo não executado cancela saldo mediante aditivo. Não mudar o denominador de percentuais passados. Se o contratado foi medido em excesso, corrigir primeiro pelo fluxo de supressão medida e seus efeitos financeiros, sem disfarçar a correção como mudança de valor global.

### 5.1 Serviços profissionais por incidência percentual

A incidência possui taxa e lista versionada de títulos-base. Na contratação, a base usa os valores contratados elegíveis; na medição, usa somente os valores acumulados efetivamente reconhecidos desses títulos. Definir `B_k` como essa base acumulada, `H_k = truncar2(B_k × taxa / 100)` e `devido_periodo = H_k - H_anterior_reconhecido`.

Os títulos usam seus preços contratuais, que podem já incluir BDI; não reaplicar BDI. Excluir reajustes, tributos, retenções, adiantamentos, itens não reconhecidos/glosados e a própria incidência. Outros itens percentuais são excluídos por padrão; sua inclusão exige seleção explícita e grafo acíclico avaliado em ordem topológica. Mostrar os títulos e componentes incluídos na memória. Alteração de taxa ou base cria segmento econômico e não recalcula o histórico.

Título contratado de R$ 10.000,00 e taxa de 20% dão valor contratual de R$ 2.000,00 para a incidência. Se a base acumulada executada passa de R$ 2.000,00 para R$ 4.000,00, H passa de R$ 400,00 para R$ 800,00; o período deve R$ 400,00, sem cobrar novamente o valor contratual completo. Base com item rejeitado não o inclui. Dois títulos não podem incidir circularmente um sobre o outro.

## 6. Supressão medida e competência sem avanço

Redução de acumulado exige lote de supressão proposto, justificado e aprovado integralmente. O lote guarda os itens e a versão usados na decisão. Rejeição mantém os acumulados anteriores; anulação é nova decisão e exige conciliar efeitos que já geraram fatura ou pagamento.

Cada item suprimido referencia o reconhecimento original e seus segmentos de preço, competência e reajuste. Reverter proporcionalmente a memória original, inclusive seus centavos por diferença de acumulados; a última reversão fecha o saldo do reconhecimento. Não usar o índice atual sobre a base líquida de serviços de ciclos diferentes. Anular a supressão reverte a mesma correção, sem recalcular por novo índice.

Exemplo de ciclos: suprimir R$ 100 de serviço que recebeu reajuste de 5% reverte R$ 100 de serviço e R$ 5 de reajuste. Nova execução de R$ 1.000 no ciclo de 10% gera R$ 100 de reajuste. O período registra serviços de R$ 900, reajuste de R$ 95 e bruto de R$ 995. Aplicar 10% sobre o líquido de 900, obtendo reajuste de 90, é incorreto. Retenções, faturas e baixas são conciliadas por ajuste, sem apagar o fato anterior.

Exemplo: item A reduz 5 un a R$ 10 e item B aumenta 10 un a R$ 20. Período líquido R$ 150,00, somente após a aprovação da supressão. Rejeitado o lote, A permanece e a medição deve ser reconferida com R$ 200,00, sem reaproveitar a aprovação de valor anterior.

Como resolução explícita da divergência entre apostila 2010 e pacote 2019, criar dois tipos:

- **Registro de competência sem avanço:** total zero, fotos/justificativa quando cabíveis, cronologia e histórico; não gera crédito ou débito financeiro.
- **Medição financeira:** total de serviços positivo após supressões aprovadas; zero ou negativo não é liberado como cobrança. Acertos exclusivamente negativos usam crédito/compensação vinculado, com aprovação e conciliação, sem falsificar execução positiva.

O fechamento final sem serviços restantes usa registro técnico de encerramento e termo com saldo, sem fabricar medição financeira positiva. Criar registro de encerramento técnico com contrato, competência, data de conclusão, comissão, saldo de escopo, termo e última medição financeira opcional. O recebimento aponta para uma referência discriminada: medição final financeira ou esse registro técnico. Sua data mínima é o fim da referência escolhida. Exemplo: 100% medido em junho, conclusão técnica em julho e R$ 500 financeiros pendentes permitem termo em julho sem nova parcela; o termo não pode anteceder a conclusão e os R$ 500 continuam abertos. Trata-se de decisão privada necessária ao fechamento consistente.

## 7. Reajuste

Modo por obra ou por serviço, com política, periodicidade, data-base do preço e data de referência identificadas. O padrão contratual é periodicidade anual quando aplicável à contratação; cláusula divergente exige validação jurídica e configuração explícita antes da assinatura. Não pressupor autorização legal para periodicidade inferior.

A competência do índice é fixada por ciclo, contado da data de referência de reajuste e da periodicidade do segmento. A data-base seleciona o índice inicial I0 e não substitui a âncora dos aniversários. Selecionar a competência do aniversário aplicável e manter esse fator nas medições até o próximo aniversário, conforme SIC-003, página impressa 36. Publicação posterior do índice definitivo gera complemento da mesma competência, não atualização mensal do fator. Exemplo: índice-base 100, aniversário em janeiro com 105, fevereiro publicado com 106. Execução em fevereiro no mesmo ciclo anual usa 105, produzindo R$ 500 sobre R$ 10.000, não R$ 600. Serviço novo conserva aniversário próprio. Datas diferentes são permitidas e não se confundem: I0 de dezembro/2024 = 100, referência janeiro/2025, aniversário janeiro/2026 = 105, índice dezembro/2025 = 104. Execução de fevereiro/2026 sobre R$ 10.000 usa a competência janeiro/2026 e deve R$ 500, não R$ 400.

Para a política inicial de índice simples:

`fator = arredondar10(I_competencia / I_base)`

`reajuste_devido = truncar2(base_elegivel_do_periodo × (fator - 1))`

`complemento = reajuste_definitivo - reajuste_ja_reconhecido`

Índice-base deve ser positivo e os índices precisam de fonte, competência e revisão. Valor de índice negativo ou zero é inválido; variação negativa entre dois índices positivos pode reduzir o reajuste, conforme cláusula contratual explícita. Ausência de índice não equivale a zero nem a último índice silenciosamente: bloquear definitivo ou usar provisório identificado e autorizado. Entrada manual exige memória anexada, justificativa e aprovador.

O período que atravessa aniversário é dividido por data de execução evidenciada. Se a quantidade por subperíodo não estiver disponível, impedir rateio presumido definitivo e exigir alocação documentada. Não distribuir custo por dias automaticamente sem cláusula e quantidade compatíveis. Novo serviço usa sua própria data do preço para a carência aplicável.

Exemplo: base elegível de R$ 10.000,00, índice-base 100 e índice de competência 105 resultam em reajuste R$ 500,00. Provisório de R$ 800,00 e definitivo de R$ 950,00 geram complemento de R$ 150,00, nunca novo reconhecimento de R$ 950,00. Alterar a medição em conferência invalida suas prévias de reajuste.

## 8. Faturas, retenções, adiantamentos e caixa

Cada contrato guarda a perspectiva da organização: receita do contratado ou despesa do contratante. Uma relação pode aparecer em duas organizações independentes, sem compartilhar automaticamente seus registros. Um fluxo dentro da mesma organização não pode ser contado duas vezes por estar ligado a obra, contrato, pedido e fatura.

`valor_bruto_faturavel = servicos_liberados + reajustes_liberados + ajustes_aprovados`

`liquido_a_liquidar = bruto - retencoes - amortizacao_adiantamento - creditos_aplicados + debitos_aprovados`

As origens financeiras são tipadas: serviço/reajuste liberado, compra ou despesa direta aprovada, autorização de adiantamento, liberação de retenção e ajuste/compensação autorizado. Só serviço/reajuste exige medição; não fabricar medição ou documento fiscal de execução para uma origem que não os requer. Cada origem tem alçada, saldo, contraparte, natureza e evidência. Título pode anteceder fatura fiscal quando a natureza e o instrumento permitem, sem apresentar o título como nota fiscal.

Classificar cada parcela e proibir dupla dedução. A retenção contratual cria saldo próprio a liberar; retenção tributária guarda natureza, base, taxa, responsabilidade e comprovante e não é receita adicional. A definição das taxas e obrigações tributárias pertence ao enquadramento do cliente e precisa de validação profissional. O sistema não infere um regime fiscal nacional a partir de um CNPJ.

A política inicial deduz, sobre bases contratuais explícitas, retenções tributárias, retenções contratuais, créditos vinculados e amortização de adiantamento, nessa ordem de alocação. Essa é uma política operacional versionada, não regra tributária universal. Retenções exigíveis que superem a obrigação bloqueiam o título ordinário até ajuste fundamentado; não são truncadas silenciosamente. Créditos e amortização consomem no máximo o líquido ainda elegível e o saldo disponível da origem. Soma das deduções nunca gera título ordinário negativo; excesso continua como crédito independente. Exemplo: bruto 80, retenção 10 e crédito de adiantamento 100 permitem amortizar 70, deixam crédito 30 e líquido zero, sem novo caixa. Reservar crédito e saldo de título na mesma transação para impedir consumo concorrente.

Uma fatura pode consumir várias parcelas liberadas e uma parcela pode ser faturada parcialmente. Manter tabela de alocação com saldo e bloqueio transacional. Adiantamento registra caixa e crédito a amortizar; não é execução ou medição. A alocação posterior liquida parte do título sem criar nova saída de dinheiro.

Exemplo: bruto 1.000, retenção contratual 50 e amortização de adiantamento 100 produzem líquido novo de 850. O crédito de adiantamento diminui 100; a retenção a liberar aumenta 50. Pagamento novo de 600 deixa líquido aberto de 250. Ao quitar os 250, ainda há retenção de 50, cujo desembolso futuro é separado. Cada baixa e cada estorno referenciam um lançamento, com conciliação e motivo.

Nenhuma mudança de estado técnico cria baixa bancária. Contratado, medido, aprovado, faturado e recebido são estágios da mesma cadeia, não receitas a somar. Previsão de caixa usa vencimentos e probabilidade explicitada; realizado usa somente liquidações conciliadas, com data efetiva.

### 8.1 Reserva e consumo de manutenção

`disponivel = limite_vigente - consumido_reconhecido - reserva_remanescente`.

Submeter a OS reserva seu valor. A confirmação técnica da execução é o evento autoritativo que transfere o valor aprovado da reserva remanescente para consumido reconhecido, sem somá-lo duas vezes. Execução em divergência continua protegida pela reserva até decisão. Medição e fatura referenciam o consumo, sem novo débito no limite do acordo. Correção de consumo exige evento vinculado aprovado, com tratamento dos efeitos financeiros.

Cancelar libera somente reserva não consumida; revisão ajusta seu delta em transação única. Limite 100, OS 80 e execução confirmada 50 deixam consumido 50, reservado 30 e disponível 20. Cancelar os 30 restantes deixa consumido 50, reservado zero e disponível 50. Nova OS 60 é rejeitada. Reenviar cancelamento não muda o resultado. Preços de execução seguem a versão autorizada da OS.

## 9. Calendários e prazos

Separar três conceitos: cadência das parcelas do cronograma, prazo de execução material e prazo de vigência do instrumento. A [IN SIE 04/2026](https://www.sie.sc.gov.br/webdocs/sie/legislacao/estadual/in/in-04-2026.pdf), arts. 5 a 9, determina o registro de prazos em dias corridos, sem alterar a data final do documento, e o acompanhamento independente de execução e vigência. Esta especificação adapta esse controle à contratação privada.

O orçamento usa períodos de 30 dias por padrão. Para a cadência do cronograma, o contrato escolhe `DIAS_FIXOS_SICOP` ou `CALENDARIO_CIVIL`, registradas antes da versão inicial. O padrão privado mensal é civil. Essa escolha não converte automaticamente um prazo contratual de meses em múltiplos de 30 dias. Guardar unidade, quantidade e âncora do instrumento original, data final calculada e duração equivalente em dias. Divergência com o documento formal bloqueia a consolidação até resolução autorizada.

- Prazo de execução: início material na data da OS; primeiro dia contado é o seguinte. Término = data da OS + número de dias corridos. Exemplo: 01/03 + 10 dias = 11/03. Início e término precisam estar contidos na vigência válida.
- Cadência fixa, quando escolhida: mensal 30, bimestral 60, trimestral 90, semestral 180 e anual 365 dias. Serve para programação e não substitui o termo final do instrumento.
- Cadência civil: avançar meses mantendo a âncora original; se o dia não existir, usar o último dia do mês, sem perder a âncora. Exemplo: 31/01/2027 gera 28/02 e 31/03; em 2028 gera 29/02. Um mês contratual de 31/01/2027 a 28/02/2027 equivale a 28 dias corridos, não a 30.
- Vigência: datas próprias, fundamentadas no documento formal; não deduzir automaticamente da OS ou do fim da execução. Extensão exige instrumento privado válido e aprovação correspondente. A prorrogação automática do art. 111 da Lei 14.133 não é aplicada a contratos privados.
- Paralisação: intervalo `[inicio, retomada)`, sem sobreposição nem contagem dupla na mesma frente. Parada em 10/03 e retomada em 13/03 apura 3 dias. Esses dias alteram a projeção; só alteram o prazo formal com registro privado de ajuste aprovado, memória e cronograma compatível. A vigência permanece inalterada.
- Contenção após pausa: se o novo término de execução ultrapassar a vigência, apresentar a projeção e bloquear consolidação/retomada com esse prazo até aditivo de vigência. Parada parcial só afeta o escopo e prazo aprovados, sem prorrogar toda a obra automaticamente.
- Paralisação aberta: projeção identificada, sem fabricar término formal. Autorização de atos de execução no escopo parado é bloqueada. São permitidos atos administrativos, fatos anteriores com data/evidência e ocorrência factual de execução irregular; registrar a ocorrência não autoriza o fato nem libera cobrança automaticamente.
- O registro privado equivalente ao apostilamento guarda ordem de pausa e reinício, dias apurados, prazo original/acumulado/acrescido, datas anterior/nova, documento, justificativa, aprovador e cronograma. Aplicação transacional e idempotente, sem alterar preço, escopo ou vigência por essa via. Outras alterações seguem aditivo.
- Data eficaz de aditivo segue AD-05; correção de ato antigo após períodos consolidados usa tratamento excepcional auditado, sem reescrever medições. Uma extensão preserva a memória dos prazos anterior, acrescido e consolidado.

O caput e o parágrafo do art. 13 da IN possuem redações que precisam de leitura conjunta. Para a política privada, a consolidação dos dias de pausa usa o instrumento específico dos arts. 11 e 12; qualquer prorrogação além desse efeito e qualquer extensão de vigência usam aditivo. Não se atribui ao texto uma precisão que ele não oferece.

## 10. Rateios e reconciliação

Trabalhar com percentuais internos de até seis casas. Na exibição com duas casas, distribuir o residual pelo método dos maiores restos, com desempate por posição estável crescente e, em seguida, identificador estável. Mesma técnica para repartir centavos de um valor fechado. Exemplo: R$ 100,00 em três parcelas iguais resulta em 33,34; 33,33; 33,33. Mostrar a parcela que absorveu o residual. A linearização de três períodos usa 33,34%; 33,33%; 33,33%, com o mesmo desempate. A ordem acidental da consulta ao banco não altera o resultado.

O rateio deve conservar o total e não criar receita ou custo. Bases de orçamento, contratos, grupos, etapas, medições e faturas precisam de reconciliação explícita. Tolerância monetária entre totais consolidados: **zero centavo inexplicado**. Se um documento externo trouxer valor diferente, registrar a diferença e sua aprovação, sem ocultá-la em tolerância arbitrária.

## 11. Casos obrigatórios de referência

Implementar testes para os exemplos deste documento e [vetores](../validacao/vetores-calculo.json). Incluir estouro, zero, valores negativos, quantidade fracionária, índice ausente, edição concorrente, alteração de política, último centavo, supressão anulada depois de paga, dias sobrepostos, ano bissexto e repetição de comando.

A mesma entrada e política produzem o mesmo resultado em servidor e WASM. O teste deve comparar o resultado do produto com a referência independente, não executar a mesma função em dois lugares e chamar isso de validação financeira.
