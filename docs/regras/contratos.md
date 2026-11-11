# Contratos, alterações, reajustes e recebimentos

Esta especificação define o comportamento do canteiro-360 para obras e serviços privados. A implementação é orientada pelas regras públicas do SICOP de Santa Catarina, com a origem de cada comportamento indicada abaixo. A data de referência documental é 10/11/2026.

- **Documentada:** regra operacional expressa na fonte, preservada no produto.
- **Adaptada:** regra documentada com adequação explícita a partes, alçadas, contratos ou controles privados.
- **Decisão:** comportamento fixado para o produto quando os manuais não oferecem definição suficiente ou apresentam conflito.

A descrição da fonte e o comportamento privado são campos distintos. A implementação deve seguir o comportamento privado e seus testes, mantendo rastreabilidade. Regras não publicadas do SICOP não integram uma declaração de paridade comprovada. Não se reproduzem código, interface proprietária ou integrações reservadas do sistema público.

## Ordem e precedência

1. O pacote que altera expressamente uma regra prevalece sobre a descrição histórica dessa mesma regra.
2. A apostila completa SIC-049 tem conteúdo histórico. A data de exportação do PDF não torna todas as suas regras posteriores aos pacotes de alteração.
3. SIC-046 substitui a associação de pré-aditivo em Definitivo pela condição Liberado e remove comparação de data/número entre proposta e aditivo.
4. SIC-042 substitui o bloqueio absoluto de medição final sem 100% de execução descrito em SIC-048 e SIC-046. Medição final pode ter saldo remanescente explicitado no termo correspondente.
5. SIC-039 rege correções de quantidades já medidas. Esse fluxo não substitui o aditivo que reduz escopo ainda não executado.
6. A [IN SIE 04/2026, SIC-050](https://www.sie.sc.gov.br/webdocs/sie/legislacao/estadual/in/in-04-2026.pdf), de 17/09/2026, é referência posterior de prazos, registros e responsabilidades. Seus arts. 4º-14 prevalecem sobre o recálculo genérico das apostilas históricas: documento válido governa o registro; execução e vigência são independentes; execução fica contida na vigência; suspensão afeta execução mediante formalização e não amplia vigência automaticamente.
7. SIC-050 art. 13 tem redação ampla no parágrafo único, enquanto o caput ressalva a suspensão formalizada e os arts. 11-12 tratam especificamente do apostilamento de pausa e do aditivo de vigência. O produto adota a distinção dos dispositivos específicos como política privada explícita; não afirma que a aparente tensão editorial inexiste. A aplicação automática da Lei 14.133/2021 a contratos privados está excluída.
8. O conflito do intervalo de cronograma em SIC-002 é resolvido por CR-02 como decisão privada. Cronograma e conversão do prazo formal são conceitos distintos: converter meses ou anos pactuados em dias preserva a data final do instrumento, conforme CT-05. Os detalhes monetários, fórmulas e datas seguem [calculos.md](calculos.md).

As páginas dos documentos SICOP citadas são as impressas no rodapé quando existentes. A contagem física do PDF pode incluir uma capa adicional. Em pacotes de alteração sem paginação impressa, usa-se a posição da página no arquivo, iniciada em um.

## Estados que permanecem separados

| Objeto | Significado operacional | Não produz automaticamente |
|---|---|---|
| Contrato Concluído | Execução declarada concluída, por ato autorizado ou medição final | Recebimento definitivo, faturamento ou quitação |
| Medição definitiva | Quantidades e valores consolidados | Assinatura do recebimento provisório |
| Medição final liberada | Termo e aprovação permitem prosseguir ao financeiro | Pagamento ou recebimento bancário |
| Recebimento provisório | Aceite inicial documentado | Encerramento das garantias |
| Recebimento definitivo | Aceite técnico final conforme obrigações aplicáveis | Baixa de contas em aberto |
| Contrato Encerrado | Motivo e instrumento de encerramento registrados | Extinção de passivos financeiros existentes |

## Regras implementáveis

Cada regra informa a issue responsável pelo seu aceite integral. A implementação em ordem pode criar antes as entidades e controles necessários. Os critérios se aplicam ao servidor Rust e aos fluxos do frontend Rust; a prévia no navegador não substitui validação do servidor.

### CT-01

**Origem:** Adaptada. **Issue primária:** [23](../issues/023.md).

**Fonte:** [SIC-002](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloContrato.2025.pdf). §§4.1.2 e 4.2.1; páginas impressas 11-15 e 24.

**Fontes complementares:** [SIC-050](https://www.sie.sc.gov.br/webdocs/sie/legislacao/estadual/in/in-04-2026.pdf), art. 4º; página física 4.

**Regra de origem:** Contrato registra partes, objeto, tipo, assinatura, proposta, execução, vigência e referência de reajuste. Pode conter várias obras com serviços, orçamento e comissão próprios. O identificador SICOP combina tipo, sequência anual, ano e complemento.

**Comportamento privado:** Organização é a fronteira de dados. Contrato privado mantém identificador interno único por organização, referência externa e vínculo a uma ou mais obras; ativo privado substitui bem público. Usar como formato inicial CT-00001/2026, mantendo sequência atômica por organização e ano. Partes e obras precisam pertencer ou estar explicitamente vinculadas à mesma organização. Documento contratual válido é a referência do cadastro: divergência fica identificada e exige correção auditada, sem alterar o instrumento para acomodar o sistema.

**Aceite verificável:** Criar dois contratos simultaneamente na mesma organização gera códigos distintos. Duas organizações podem usar CT-00001/2026 sem compartilhar dados. Vincular obra de outra organização pela API é rejeitado. Contrato com duas obras mantém serviços e orçamento separados.

### CT-02

**Origem:** Adaptada. **Issue primária:** [23](../issues/023.md).

**Fonte:** [SIC-002](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloContrato.2025.pdf). §4.1.5; páginas impressas 19-21.

**Fontes complementares:** [SIC-050](https://www.sie.sc.gov.br/webdocs/sie/legislacao/estadual/in/in-04-2026.pdf), art. 15 §§1º-2º; página física 7.

**Regra de origem:** O criador é associado como gestor do novo contrato. Deve permanecer pelo menos um gestor. O vínculo de gestão controla o acesso e a alteração dos dados contratuais.

**Comportamento privado:** Aplicar papéis, vínculo ao contrato e alçadas cumulativamente. Criador autorizado torna-se gestor inicial. Somente gestor com permissão pode editar a capa; fiscais, fornecedores e clientes veem apenas funções concedidas. Remover ou inativar o último gestor exige substituição na mesma transação. Desligamento de usuário preserva atribuições históricas. Gestor responde pelos atos administrativos concedidos; fiscal responde pelos registros técnicos de execução, medição e recebimento provisório. Uma concessão administrativa não equivale a habilitação profissional.

**Aceite verificável:** Tentar remover o último gestor, inclusive por duas requisições simultâneas, falha sem deixar contrato sem responsável. Fiscal sem permissão de gestão não edita valores pela API. Gestor de outro contrato não obtém acesso apenas por conhecer o identificador.

### CT-03

**Origem:** Adaptada. **Issue primária:** [23](../issues/023.md).

**Fonte:** [SIC-002](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloContrato.2025.pdf). §§4.1.2 e 4.1.7; páginas impressas 11-13 e 21-23.

**Regra de origem:** Estado inicial sugerido A Iniciar. Alterações posteriores são registradas no histórico com data, estados anterior e novo, responsável e justificativa. Data do evento deve caber no período contratual; repetir estado é admitido para pendência administrativa.

**Comportamento privado:** Manter A iniciar, Em andamento, Paralisado, Concluído, Pendência administrativa e Encerrado, com motivo de encerramento separado. Eventos são anexados ao histórico, não sobrescritos. Eficácia e instante de registro são campos distintos. Evento fora da vigência somente entra por correção formal ou migração com política explícita, sem alterar silenciosamente a regra normal.

**Aceite verificável:** Uma edição direta do campo estado após criação é rejeitada. Repetir Em andamento falha; registrar nova pendência administrativa com justificativa mantém os dois eventos. Data fora da vigência falha no fluxo normal, e correção autorizada preserva autor, data original e justificativa.

### CT-04

**Origem:** Adaptada. **Issue primária:** [48](../issues/048.md).

**Fonte:** [SIC-002](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloContrato.2025.pdf). §4.1.2; página impressa 13.

**Fontes complementares:** [SIC-042](https://www.sicop.sc.gov.br/download/sicop/Pacote_de_alteracoes_02_2017_MedicaoFinalERecebimentoProvisorio.pdf), páginas 1-5.

**Regra de origem:** Paralisado e Cancelado derivam de registros próprios. Cálculo definitivo da medição final muda o contrato para Concluído. O manual também admite conclusão por registro de situação.

**Comportamento privado:** Paralisação, encerramento e conclusão acionam comandos específicos. Conclusão manual exige gestor autorizado, motivo e evidência de conclusão; jamais cria medição, termo, fatura ou quitação. Conclusão automática da medição final conserva o fluxo de recebimento previsto em MF-02. UI apresenta estado do contrato, estado da medição e posição financeira separadamente.

**Aceite verificável:** Consolidar medição final muda contrato para Concluído e mantém medição Aguardando recebimento provisório. A conclusão manual não cria pagamento nem libera saldo. Enviar Paralisado sem evento de paralisação pela API é rejeitado.

### CT-05

**Origem:** Adaptada. **Issue primária:** [27](../issues/027.md).

**Fonte:** [SIC-050](https://www.sie.sc.gov.br/webdocs/sie/legislacao/estadual/in/in-04-2026.pdf). arts. 5º-9º; páginas físicas 4-5.

**Fontes complementares:** [SIC-002](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloContrato.2025.pdf), §4.1.2; páginas impressas 13-15; descrição histórica, subordinada à IN 04/2026 nos pontos alterados.

**Regra de origem:** Prazos de execução e vigência são independentes. A execução começa pela OS ou documento equivalente, com contagem a partir do dia seguinte, e deve estar integralmente contida na vigência. Converter meses ou anos em dias não altera o termo final formal. A vigência inicia no marco previsto no instrumento.

**Comportamento privado:** Guardar início e fim de vigência, data autorizada de execução e prazo de execução como campos distintos, respaldados pelo contrato privado. Término de execução usa D+n conforme calculos.md. Conversão de meses/anos parte das datas civis pactuadas, não de multiplicação indiscriminada por 30/365. Exigir início de vigência <= início da execução e término de execução <= fim da vigência. Mudança de execução que ultrapasse a vigência exige instrumento privado válido, não prorrogação automática. As opções de cálculo da apostila antiga servem apenas de apoio à migração quando reproduzirem o instrumento válido.

**Aceite verificável:** Contrato vigente de 01/03/2026 a 31/05/2026, OS de 01/03 e execução de 10 dias termina em 11/03. OS antes de 01/03 ou execução após 31/05 é recusada sem regularização formal. Prazo pactuado de 31/01 a 28/02 não vira 02/03 pela multiplicação por 30 dias. A emissão da OS não substitui os marcos de vigência.

### CT-06

**Origem:** Documentada. **Issue primária:** [23](../issues/023.md).

**Fonte:** [SIC-002](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloContrato.2025.pdf). §4.1.2; página impressa 15.

**Regra de origem:** Após aditivo de prazo ou medição, campos originais de período ficam bloqueados. Após aditivo de valor ou quantidade, ou medição, valores originais ficam bloqueados.

**Comportamento privado:** Proteger no servidor as bases originais. Alterações prospectivas usam aditivo; erro material usa correção controlada que preserva o registro anterior, sua origem e os efeitos compensatórios. A exclusão de um rascunho não deve apagar evidência já consolidada nem liberar edição indevida.

**Aceite verificável:** Com primeira medição registrada, editar início, duração ou valor original pela API é rejeitado. Adicionar aditivo válido altera a versão vigente, mantendo a base original. Rollback de transação com erro não deixa parte da base modificada.

### CT-07

**Origem:** Adaptada. **Issue primária:** [24](../issues/024.md).

**Fonte:** [SIC-002](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloContrato.2025.pdf). §§4.1.2 e 4.1.8; páginas impressas 12-15 e 23.

**Fontes complementares:** [SIC-050](https://www.sie.sc.gov.br/webdocs/sie/legislacao/estadual/in/in-04-2026.pdf), art. 2º XIV e art. 4º; páginas físicas 3-4.

**Regra de origem:** Contrato, aditivos e total exibem valor informado, calculado e diferença. O calculado decorre da soma das quantidades multiplicadas pelos preços unitários. A tolerância da pequena diferença mencionada no manual não é quantificada.

**Comportamento privado:** Conservar totais declarados e calculados separados; o valor declarado inicial deve corresponder ao instrumento contratual válido, equivalente privado da decisão homologada. Valores calculados são somente leitura, em decimal exato. Tolerância, arredondamento e bloqueio de aprovação seguem calculos.md como decisão do produto. Diferença não resolvida é visível; não ajustar artificialmente uma quantidade para ocultá-la.

**Aceite verificável:** Dois itens de 2 × R$ 10,00 e 3 × R$ 7,50 produzem R$ 42,50. Declaração de R$ 43,00 mostra diferença R$ 0,50. Alterar o total calculado diretamente é rejeitado; a aprovação segue a tolerância explicitamente configurada.

### CT-08

**Origem:** Documentada. **Issue primária:** [24](../issues/024.md).

**Fonte:** [SIC-002](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloContrato.2025.pdf). §§4.2.2 e 4.2.4; páginas impressas 26 e 30-31.

**Regra de origem:** Rateios entre obras somam 100%. Após cadastrar serviços, participação de custo é calculada. Rateios municipais somam 100% e suas dimensões compõem a dimensão da obra.

**Comportamento privado:** Guardar rateios e dimensões por obra/localidade com unidade. Valor dos serviços passa a determinar participação financeira; ajustes de arredondamento usam a política de calculos.md. Múltiplos municípios só são exigidos quando existirem, sem duplicar dimensões nem misturar unidades incompatíveis.

**Aceite verificável:** Obras de R$ 60.000,00 e R$ 40.000,00 produzem 60% e 40%. Rateio manual de 60% e 30% não pode ser aprovado. Trechos de 2 km e 3 km totalizam 5 km; um terceiro trecho em m² não entra na mesma soma.

### CT-09

**Origem:** Adaptada. **Issue primária:** [24](../issues/024.md).

**Fonte:** [SIC-002](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloContrato.2025.pdf). §4.2.2; páginas impressas 26-27.

**Regra de origem:** Obra contratual vincula orçamento e variação aprovada; os serviços vêm dessa versão. Ata de manutenção sem quantidade utiliza tabela SINAPI em vez de orçamento e variação.

**Comportamento privado:** Contratação copia uma versão aprovada e guarda vínculo com a origem, sem atualizar preços automaticamente quando o catálogo muda. O modo acordo de manutenção sem quantidade será habilitado na issue 51 e terá tabela, edição e regra de desconto identificadas; não será uma dispensa genérica de orçamento nos demais contratos.

**Aceite verificável:** Após contratar orçamento aprovado de R$ 50.000,00, atualizar o catálogo não altera contrato nem serviços. Versão em conferência não é contratável. O modo manutenção sem quantidade só fica disponível após implementação das regras de acordo e saldo da issue 51.

### CT-10

**Origem:** Adaptada. **Issue primária:** [34](../issues/034.md).

**Fonte:** [SIC-002](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloContrato.2025.pdf). §4.1.2; página impressa 12.

**Regra de origem:** Por padrão, cálculo definitivo exige fotografia para cada grupo de serviço medido. O legado permite desabilitar a exigência por suporte.

**Comportamento privado:** Manter exigência padrão por grupo efetivamente medido. Exceção é política explícita aprovada por gestor, justificada por contrato e tipo de serviço, com alternativa documental adequada para entregas sem evidência fotográfica. A exceção não desativa vínculo, data, integridade e autorização dos anexos.

**Aceite verificável:** Dois grupos medidos com foto em apenas um impedem consolidação e exibem o grupo faltante. Exceção autorizada para entrega de projeto aceita documento técnico vinculado; usuário sem alçada não consegue remover a exigência. Foto de outro contrato não satisfaz a validação.

### RT-01

**Origem:** Adaptada. **Issue primária:** [34](../issues/034.md).

**Fonte:** [SIC-002](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloContrato.2025.pdf). §4.2.3; páginas impressas 27-30.

**Regra de origem:** Comissão por obra contém representantes da fiscalização e da contratada, no mínimo dois profissionais. Havendo consultoria, incluir representante próprio. Responsabilidades são específicas à função.

**Comportamento privado:** Representante do contratante ou gerenciadora assume fiscalização; contratado mantém representante distinto. A configuração padrão preserva dois profissionais, funções e vínculos de responsabilidade pertinentes. Obras por administração direta usam modalidade própria e política explícita, sem criar empresa contratada fictícia. Exceções não eliminam responsabilidades legais.

**Aceite verificável:** Obra de empreitada com apenas representante da contratada não passa na conferência. Adicionar fiscal habilitado, função e responsabilidade vinculada permite prosseguir. Quando consultoria estiver no escopo, sua ausência é indicada. Upload de ART não é exibido como autenticação no conselho.

### RT-02

**Origem:** Adaptada. **Issue primária:** [34](../issues/034.md).

**Fonte:** [SIC-002](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloContrato.2025.pdf). §4.2.3.1; página impressa 29.

**Regra de origem:** ART associada ao profissional deve ter início igual ou posterior à assinatura do contrato. Atuação pode ser inativada. O cadastro público usa dados de portaria do fiscal.

**Comportamento privado:** Converter portaria em designação privada. Preservar a validação padrão de datas no vínculo contratual. Responsabilidade que legitimamente anteceda contrato formal exige exceção documentada por responsável autorizado, com motivo e documento, sem alterar a data original. Esse caminho é adaptação privada, não regra comprovada do SICOP.

**Aceite verificável:** Vínculo de ART com início anterior à assinatura é recusado no fluxo normal. Exceção autorizada mantém datas reais e justificativa no relatório. Usuário comum não cria exceção; profissional inativo não é oferecido para novos atos.

### RT-03

**Origem:** Adaptada. **Issue primária:** [39](../issues/039.md).

**Fonte:** [SIC-049](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloContrato.2010.0002-AG.pdf). §§5.1.3 e 5.1.4; páginas impressas 72-73.

**Regra de origem:** Comissão de pré-aditivo é por obra; padrão de dois profissionais, um de cada parte. Elegibilidade depende de vínculo à obra e período da ART que compreenda a data da proposta.

**Comportamento privado:** A alçada define a composição exigida conforme modalidade, preservando separação entre elaboração e aprovação. Só são selecionáveis responsáveis ativos com função e responsabilidade que cubram o ato. Substituições mantêm histórico e invalidam aprovações ainda não concluídas quando mudam a identidade dos signatários.

**Aceite verificável:** Pré-aditivo de 01/07 não aceita profissional cuja atuação terminou em 30/06. Proposta para duas obras valida comissão nas duas. Troca de signatário após envio exige nova confirmação, sem reaproveitar aprovação ligada à versão anterior.

### RT-04

**Origem:** Documentada. **Issue primária:** [14](../issues/014.md).

**Fonte:** [SIC-013](https://www.sicop.sc.gov.br/download/sicop/passo_a_passo_cadastro_de_arts.pdf). páginas 1-3.

**Fontes complementares:** [SIC-048](https://www.sicop.sc.gov.br/download/sicop/Evolucao_01_2016.pdf), página física 4; exigência de ART/RRT no orçamento em Aprovado.

**Regra de origem:** Registro distingue ART e RRT e reproduz dados do documento do conselho correspondente. Na aprovação do orçamento, SIC-048 move a exigência de ART/RRT da situação Definitivo para Aprovado.

**Comportamento privado:** Guardar conselho, tipo, número, profissional, atividade, período, documento, vínculo e estado cadastral. Responsabilidades de orçamento, projeto, execução e fiscalização não se substituem automaticamente. Orçamento pode ser finalizado sem ART/RRT quando essa é a única pendência; registro aplicável é exigido ao aprovar, sem transformar ausência de campo legado em dispensa legal. Documentos necessários ao início/execução seguem a matriz de obrigações específica. Só afirmar verificação externa quando efetivamente realizada por fonte autorizada, com data e evidência.

**Aceite verificável:** A ART de orçamento não satisfaz execução sem atividade compatível. Anexar arquivo não marca validação externa. Nas etapas 18/21, orçamento conferido pode ficar Finalizado com ART/RRT pendente, mas a aprovação é bloqueada quando exigível; anexar registro aplicável permite nova validação. Ausência justificada de obrigação legal fica identificada.

### AD-01

**Origem:** Documentada. **Issue primária:** [38](../issues/038.md).

**Fonte:** [SIC-049](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloContrato.2010.0002-AG.pdf). §§5.1 e 5.2; páginas impressas 69-82.

**Fontes complementares:** [SIC-050](https://www.sie.sc.gov.br/webdocs/sie/legislacao/estadual/in/in-04-2026.pdf), art. 2º IX-XI e art. 4º; páginas físicas 3-4.

**Regra de origem:** Pré-aditivo prepara e justifica mudança. Aditivo formaliza sua incorporação ao contrato. São objetos distintos.

**Comportamento privado:** Modelar proposta de alteração e instrumento formal separadamente. Proposta liberada ainda não aumenta limite de medição, preço ou prazo. Somente aplicação atômica do aditivo formal produz nova versão contratual. Apostilamento ou registro privado equivalente é objeto distinto, reservado a ajustes sem alteração substancial e aos efeitos de suspensão permitidos pela política contratual; não usar esse objeto para esconder acréscimo de escopo ou preço.

**Aceite verificável:** Liberar proposta de acréscimo de 10 unidades não aumenta saldo do contrato. Aplicar instrumento formal aprovado aumenta uma única vez. A tela diferencia Proposta liberada e Aditivo aplicado.

### AD-02

**Origem:** Adaptada. **Issue primária:** [38](../issues/038.md).

**Fonte:** [SIC-049](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloContrato.2010.0002-AG.pdf). §5.1.2; páginas impressas 70-71.

**Fontes complementares:** [SIC-046](https://www.sicop.sc.gov.br/download/sicop/Pacote_de_alteracoes_03_2016_data_de_referencia_do_pre-aditivo_e_aditivo.pdf), página 2.

**Regra de origem:** Pré-aditivo nasce em Conferência e somente nessa situação pode ser editado. Rejeição inativa a proposta; nova variação copia os dados e conserva o histórico. Pacotes posteriores introduzem Liberado como condição para associação.

**Comportamento privado:** Usar Em conferência, Em aprovação, Liberado, Rejeitado, Inativo e Incorporado. A nomenclatura privada torna explícita a diferença entre aprovação da proposta e incorporação. Após envio, editar conteúdo exige nova variação e invalida aprovações da anterior. Rejeição conserva a versão original.

**Aceite verificável:** Editar quantidade de proposta Em aprovação pela API é rejeitado. Criar variação de proposta rejeitada copia conteúdo, mas não assinaturas nem aprovação. Variações têm número e vínculo estáveis. Somente Liberado é elegível para formalização.

### AD-03

**Origem:** Adaptada. **Issue primária:** [38](../issues/038.md).

**Fonte:** [SIC-049](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloContrato.2010.0002-AG.pdf). §§5.1.2-5.1.6; páginas impressas 71-78.

**Fontes complementares:** [SIC-050](https://www.sie.sc.gov.br/webdocs/sie/legislacao/estadual/in/in-04-2026.pdf), arts. 13-14; páginas físicas 6-7.

**Regra de origem:** Proposta guarda dias, valores informados e calculados, percentuais, justificativa, comissão e alterações por obra/grupo/serviço. Delta e nova quantidade são interdependentes. Todo grupo alterado exige localização, motivo, solução e consequências.

**Comportamento privado:** Mostrar comparação antes/depois com acréscimos e supressões separados, mesmo com saldo financeiro zero. Validar comissões, justificativas e alçadas antes do envio. Novo serviço guarda unidade, preço e componentes, índice/data quando aplicáveis. Mudança de escopo não reduz acumulado já medido; eventual correção de medição segue MF-03. Em prorrogação, explicitar prazo original, prazo acumulado, acréscimo, prazo consolidado, novas datas de execução e vigência e justificativa técnica; anexar memória e cronograma atualizado quando execução mudar.

**Aceite verificável:** Quantidade vigente 100, delta -20, resulta em nova quantidade 80. Grupo sem justificativa completa bloqueia envio. Proposta com +R$ 1.000,00 e -R$ 1.000,00 mostra ambos, não apenas total zero. Reduzir limite abaixo do já medido é rejeitado até tratar a medição por procedimento próprio.

### AD-04

**Origem:** Adaptada. **Issue primária:** [39](../issues/039.md).

**Fonte:** [SIC-048](https://www.sicop.sc.gov.br/download/sicop/Evolucao_01_2016.pdf). páginas 1-2.

**Regra de origem:** No fluxo público, prazo e supressão exclusivamente negativa têm liberação simplificada; acréscimo ou mistura de acréscimos e supressões exigem autoridade superior.

**Comportamento privado:** Preservar distinção das rotas, substituindo órgãos públicos por alçadas empresariais versionadas. Configurar critérios por valor, percentual, prazo e modalidade. Acréscimos e supressões não se compensam para decidir alçada. Limites de contratação pública não são impostos automaticamente. Solicitante não aprova sua própria proposta quando a rota exige aprovador distinto.

**Aceite verificável:** Proposta +R$ 10.000,00 e -R$ 10.000,00 segue rota mista, mesmo com valor líquido zero. Mudança de alçada não altera aprovações históricas; proposta ainda pendente usa versão de política explicitada. Link de notificação sem sessão e alçada não aprova.

### AD-05

**Origem:** Adaptada. **Issue primária:** [40](../issues/040.md).

**Fonte:** [SIC-046](https://www.sicop.sc.gov.br/download/sicop/Pacote_de_alteracoes_03_2016_data_de_referencia_do_pre-aditivo_e_aditivo.pdf). páginas 1-2.

**Fontes complementares:** [SIC-050](https://www.sie.sc.gov.br/webdocs/sie/legislacao/estadual/in/in-04-2026.pdf), art. 4º e art. 8º; páginas físicas 4-5.

**Regra de origem:** Data do pré-aditivo/aditivo retrata o ato. Janela permitida começa no dia posterior ao fim da última medição definitiva e termina no menor entre fim contratual e dia do registro. Associação depende de pré-aditivo Liberado, sem comparar sua data ou número com o aditivo.

**Comportamento privado:** Guardar data eficaz e instante de inclusão separadamente. Na ausência de medição definitiva, início contratual é o limite inferior da política privada. Nesta adaptação, o limite superior contratual da janela é o fim da vigência vigente. Não deslocar datas reais para contornar validação. Um ato válido fora da janela segue correção formal ou migração, com justificativa, documento, impacto e revalidação financeira. O registro deve retratar o instrumento válido; não se aplica automaticamente a extensão de vigência prevista para contratos administrativos por escopo.

**Aceite verificável:** Última medição terminou em 30/06; data eficaz 30/06 falha e 01/07 é elegível. Com cadastro em 05/07 e fim contratual 31/07, 06/07 falha. Sem medição, data anterior ao início contratual falha. Proposta Liberada não é rejeitada apenas por comparação de seu número ou data com os do instrumento.

### AD-06

**Origem:** Adaptada. **Issue primária:** [40](../issues/040.md).

**Fonte:** [SIC-050](https://www.sie.sc.gov.br/webdocs/sie/legislacao/estadual/in/in-04-2026.pdf). arts. 11-14; páginas físicas 6-7; complementarmente SIC-049 §5.2.2, p.81 impressa.

**Fontes complementares:** [SIC-049](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloContrato.2010.0002-AG.pdf), §5.2.2; página impressa 81; descrição histórica, subordinada à IN 04/2026 nos pontos alterados.

**Regra de origem:** Aditivo incorpora a alteração formal, com memória dos prazos e datas de execução e vigência. Alteração de execução exige cronograma físico-financeiro atualizado. Efeito de pausa é consolidado por apostilamento, e eventual extensão da vigência exige termo aditivo; não há campo único de prazo que substitua essas distinções.

**Comportamento privado:** Aplicar instrumento uma única vez em transação que grava versão contratual, itens, saldo, cronograma vinculado e histórico. Atualizar execução e vigência separadamente, conforme o documento privado aprovado. Pausas formalizadas integram cálculo de execução mediante o registro de PA-02 e nunca ampliam vigência por simples soma de dias. Prorrogação de execução só é aplicada com memória de cálculo e versão atualizada do cronograma, que a issue 26 já deve permitir registrar; a issue 42 amplia a reprogramação integrada com execução real.

**Aceite verificável:** Duas chamadas concorrentes produzem uma versão e um incremento. Falha durante gravação reverte todos os efeitos. Adicionar cinco dias à execução não acrescenta cinco dias à vigência sem cláusula/instrumento próprio. Nova execução além da vigência é bloqueada até regularização. Prorrogação sem memória ou cronograma atualizado é recusada.

### AD-07

**Origem:** Adaptada. **Issue primária:** [40](../issues/040.md).

**Fonte:** [SIC-041](https://www.sicop.sc.gov.br/download/sicop/Pacote_de_alteracoes_03_2017_Prorrogacao_de_Contratos_de_Servicos_Continuos.pdf). páginas 1-3.

**Regra de origem:** Prorrogação de serviços contínuos é distinta de alteração de valor/quantidade. Quantidades adicionais devem ser proporcionais aos dias prorrogados. O manual contém limites públicos históricos e rota própria de liberação.

**Comportamento privado:** Configurar modalidade serviço contínuo. Prorrogação explicita período, quantidade proporcional, preço, justificativa e aprovação. Quantidade que não seja proporcional é alteração de escopo e segue rota correspondente. Não aplicar automaticamente os limites públicos de 25% ou 60 meses a contratos privados.

**Aceite verificável:** Serviço de 300 unidades para 30 dias, prorrogado por 10 dias à mesma taxa, propõe 100 unidades. Alteração para 180 unidades é identificada como mudança de escopo adicional. Política privada de alçada governa aprovação, sem bloqueio fixo de 25%.

### AD-08

**Origem:** Adaptada. **Issue primária:** [33](../issues/033.md).

**Fonte:** [SIC-040](https://www.sicop.sc.gov.br/download/sicop/Pacote_de_alteracoes_01_2018_Cadastro_Supervisao_e_Consultoria.pdf). páginas 1-3.

**Regra de origem:** Supervisão/consultoria podem ter orçamento direto no contrato, organizado em título, grupo, serviço e item. Título e item são obrigatórios. Itens percentuais incidem sobre títulos selecionados e repercutem em contrato, medição e aditivo.

**Comportamento privado:** Permitir orçamento privado de serviços profissionais com origem e aprovação. Base B é o valor acumulado elegível dos títulos explicitamente selecionados, sem o próprio item, bases cíclicas, reajustes, retenções ou tributos financeiros. Para taxa fixa, H=truncar 2(B×taxa/100); valor do período é H_atual menos H_anterior reconhecido. A seleção e a taxa são versionadas. Não reaplicar taxa sobre título que já contenha o mesmo serviço percentual; mudanças e correções preservam reconhecimento anterior.

**Aceite verificável:** Título contratado de R$ 10.000,00, base executada acumulada anterior R$ 2.000,00 e atual R$ 4.000,00, taxa 20%: devido no período R$ 400,00. Não aplicar taxa aos R$ 10.000,00 integrais em cada medição. Reajuste, retenção e tributos não entram nessa base. Configuração A incide sobre B e B sobre A, ou título que inclua o próprio percentual, é rejeitada.

### PA-01

**Origem:** Adaptada. **Issue primária:** [30](../issues/030.md).

**Fonte:** [SIC-050](https://www.sie.sc.gov.br/webdocs/sie/legislacao/estadual/in/in-04-2026.pdf). arts. 10-12; páginas físicas 5-6; fotos em SIC-049 §§5.3.2-5.3.3, pp.84-87 impressas.

**Fontes complementares:** [SIC-049](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloContrato.2010.0002-AG.pdf), §§5.3.2 e 5.3.3; páginas impressas 84-87; descrição histórica, subordinada à IN 04/2026 nos pontos alterados.

**Regra de origem:** Paralisação exige ordem motivada e autorizada com início, prazo estimado quando cabível, responsável e percentual físico. Retomada exige ordem própria. Durante a paralisação, atos de execução ficam suspensos, permanecendo possíveis atos administrativos de gestão.

**Comportamento privado:** Usar ordem privada de suspensão e de reinício com escopo, justificativa, responsável, prazo estimado quando cabível, percentual físico, documentos, aprovação e evidência de comunicação às partes. Sem reinício formal, impedir autorização de nova execução e medição ordinária de atos praticados no intervalo suspenso. Permitir registrar ocorrência factual de execução irregular, sem autorizá-la ou liberá-la financeiramente, e encaminhar à regularização. Manter gestão, pagamentos e registro de execução anterior devidamente datada. Foto histórica da pausa tem data anterior ou igual ao início; foto posterior pode documentar vistoria de retomada. Não exigir DOE ou SGPe na contratação privada.

**Aceite verificável:** Suspensão sem reinício mantém contrato/frente Paralisado. Autorizar nova execução ou liberar medição ordinária de execução durante a suspensão é rejeitado. Ocorrência de execução irregular pode ser registrada com alerta e bloqueio financeiro. Pagamento de obrigação anterior e registro de execução anterior continuam possíveis. Retomada sem ordem/aprovação é rejeitada; foto posterior não substitui evidência histórica.

### PA-02

**Origem:** Adaptada. **Issue primária:** [30](../issues/030.md).

**Fonte:** [SIC-050](https://www.sie.sc.gov.br/webdocs/sie/legislacao/estadual/in/in-04-2026.pdf). arts. 11 §§1º-2º e 12 §§1º-3º; página física 6.

**Fontes complementares:** [SIC-049](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloContrato.2010.0002-AG.pdf), §5.3.2; página impressa 85; descrição histórica, subordinada à IN 04/2026 nos pontos alterados.

**Regra de origem:** Após reinício, o período de paralisação e o novo prazo de execução são consolidados por apostilamento. Quando a vigência precisar ser estendida, é necessário termo aditivo próprio. A regra posterior distingue esses efeitos do recálculo genérico da apostila histórica.

**Comportamento privado:** Contar suspensão encerrada no intervalo [início, retomada), conforme calculos.md. Rejeitar sobreposição do mesmo escopo; repetição idempotente não cria novo intervalo. Retomada registra intervalo e projeção; consolidar a nova data de execução exige registro privado de ajuste de prazo com instrumento, autorização, memória e cronograma atualizado, distinto do aditivo que altera vigência. Suspensão parcial não estende contrato inteiro. Sem ordem válida ou suporte contratual, manter projeção identificada e pendência de regularização; nunca inventar prolongamento da vigência.

**Aceite verificável:** Parada em 01/07 com retomada em 04/07 apura três dias, mas não altera vigência. Repetir a operação não acrescenta dias. Novo intervalo sobreposto é rejeitado. Sem registro de ajuste, a nova data aparece como projetada; após instrumento válido fica consolidada. Se execução prorrogada ultrapassar vigência, exigir aditivo próprio antes de autorizar esse excedente.

### EN-01

**Origem:** Adaptada. **Issue primária:** [50](../issues/050.md).

**Fonte:** [SIC-049](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloContrato.2010.0002-AG.pdf). §5.4; páginas impressas 88-89.

**Regra de origem:** Cancelamento pode registrar rescisão ou decurso de prazo sem conclusão, com data, termo, forma, motivo e justificativa. Registro próprio altera a situação.

**Comportamento privado:** Encerrado é estado, rescisão, cancelamento ou decurso de prazo são motivos. Vencimento gera pendência e não quitação automática. Bloquear novos compromissos ordinários após encerramento; permitir acerto, cobrança, pagamento e consulta das obrigações existentes mediante permissão. Não excluir medições, faturas ou anexos.

**Aceite verificável:** Encerrar contrato com fatura em aberto preserva a cobrança e o saldo. Alterar data do sistema para depois da vigência não marca contrato como quitado. Tentar nova medição ordinária após rescisão é rejeitado; ajuste formal continua possível com origem explícita.

### RJ-01

**Origem:** Documentada. **Issue primária:** [35](../issues/035.md).

**Fonte:** [SIC-002](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloContrato.2025.pdf). §4.1.2; página impressa 14.

**Fontes complementares:** [SIC-005](https://www.sicop.sc.gov.br/download/sicop/Como_fazer_reajuste_por_Obra_ou_por_Servico_dos_Contratos.pdf), páginas 3-12 e 18-22; [SIC-003](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloMedicao.2010.0002.pdf), página impressa 36, física 37; competência fixa por periodicidade.

**Regra de origem:** Reajuste pode ser por obra ou por serviço. Índice deve ser informado na entidade correspondente. Data-base do índice, data-referência do aniversário e periodicidade são conceitos distintos; padrão publicado é 12 meses. Mês e ano de referência do reajuste ficam fixos durante cada ciclo da periodicidade, avançando somente ao ciclo seguinte.

**Comportamento privado:** Persistir modo, série, data-base, data-referência, periodicidade, competência do ciclo e versão da política. Modo por obra desativa índice por serviço e vice-versa. Na política anual, escolher a competência de cada aniversário e mantê-la nas medições daquele ciclo; não trocar pelo mês corrente a cada medição. Cláusula diferente exige política privada explícita, juridicamente validada e identificada como adaptação. Memória diferencia fator, parcela-base e valor do reajuste.

**Aceite verificável:** Contrato por obra com dois serviços usa o índice da obra e rejeita índice conflitante no serviço. Para ciclo iniciado em janeiro, índice-base 100, janeiro 105 e fevereiro 106, medições de janeiro e fevereiro do mesmo ciclo usam 105, não 106. No próximo ciclo a competência avança. Contrato por serviço aceita referências próprias; alterar base não muda aniversário silenciosamente.

### RJ-02

**Origem:** Documentada. **Issue primária:** [43](../issues/043.md).

**Fonte:** [SIC-005](https://www.sicop.sc.gov.br/download/sicop/Como_fazer_reajuste_por_Obra_ou_por_Servico_dos_Contratos.pdf). páginas 2 e 9-12.

**Regra de origem:** Novo serviço incluído por aditivo pode usar a data do seu preço em lugar da base original. Reajuste considera o decurso anual dessa referência própria.

**Comportamento privado:** Preço novo mantém data, índice e origem. Não conceder reajuste anterior ao aniversário próprio por reutilizar aniversário do contrato. Quando não existir índice aplicável, sinalizar pendência e seguir política provisória explícita de calculos.md, sem inventar índice ou aplicar zero como definitivo.

**Aceite verificável:** Serviço novo cotado em 01/07/2026 não recebe reajuste anual em 01/01/2027 apenas porque o contrato faz aniversário nessa data. A partir de 01/07/2027, usar sua referência própria. Índice ausente mantém cálculo não definitivo.

### RJ-03

**Origem:** Documentada. **Issue primária:** [35](../issues/035.md).

**Fonte:** [SIC-005](https://www.sicop.sc.gov.br/download/sicop/Como_fazer_reajuste_por_Obra_ou_por_Servico_dos_Contratos.pdf). páginas 9 e 14.

**Regra de origem:** Após a primeira medição ser incluída, índices e parâmetros do cálculo de reajuste não podem ser alterados livremente.

**Comportamento privado:** Trava deve ocorrer no servidor e conservar versão original. Correção posterior passa pelo fluxo da issue 43, com motivo, aprovação, reprocessamento dos efeitos e diferenças financeiras explícitas; nunca modifica documentos já emitidos sem gerar versão sucessora.

**Aceite verificável:** Com primeira medição existente, trocar série ou data-base por PATCH é rejeitado. Antes da medição, mudança autorizada atualiza a prévia. Correção posterior conserva séries e fatores usados nos documentos históricos.

### RJ-04

**Origem:** Adaptada. **Issue primária:** [43](../issues/043.md).

**Fonte:** [SIC-049](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloContrato.2010.0002-AG.pdf). §6.1.2; página impressa 91.

**Fontes complementares:** [SIC-005](https://www.sicop.sc.gov.br/download/sicop/Como_fazer_reajuste_por_Obra_ou_por_Servico_dos_Contratos.pdf), páginas 14-16.

**Regra de origem:** Reajuste pode ser calculado ou informado. Há cálculos provisório, definitivo e complementar. Definitivo exige os índices necessários; cálculo feito em medição em Conferência é cancelado quando ela passa a definitiva e deve ser refeito.

**Comportamento privado:** Valor informado exige justificativa, anexo e aprovação compatível. Guardar memória de cada versão. Provisório e complementar são lançamentos relacionados; complementar registra apenas diferença, conforme calculos.md. Consolidar a medição invalida prévia de reajuste e exige recálculo sobre a base congelada. Retificação de índice publicado também exige diferença versionada.

**Aceite verificável:** Provisório de R$ 800,00 seguido de definitivo total R$ 950,00 gera complemento R$ 150,00. Repetir pedido não duplica complemento. Consolidar medição com reajuste de conferência invalida a prévia. Valor informado sem justificativa é rejeitado.

### RJ-05

**Origem:** Decisão. **Issue primária:** [35](../issues/035.md).

**Fonte:** [PRODUTO](calculos.md). Política do produto em docs/regras/calculos.md; lacuna dos manuais em fórmula completa e arredondamentos.

**Fontes complementares:** [SIC-003](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloMedicao.2010.0002.pdf), página impressa 36, física 37; competência fixa do ciclo é documentada.

**Regra de origem:** Os documentos consultados não fixam todos os detalhes de fórmula, prorrata, arredondamento e retificação de séries. A competência fixa por ciclo é documentada e não integra essa lacuna.

**Comportamento privado:** Implementar a política versionada de calculos.md com decimal exato, competência fixa do ciclo, origem, memória e tratamento de índice ausente. A fórmula é decisão do canteiro-360 para contratos privados, sem alegação de reprodução integral do motor SICOP. Correção de quantidade já medida reverte o reajuste efetivamente reconhecido do segmento de origem, não um reajuste recalculado pelo índice corrente. Não confundir atualização da série histórica com novo índice de outro mês.

**Aceite verificável:** Vetores de calculos.md produzem valores idênticos na prévia Rust e no servidor, comparados com referência independente. Janeiro 105 e fevereiro 106 no mesmo ciclo anual mantêm 105. Supressão de cinco unidades originalmente reconhecidas a R$ 10,00 mais 5% reverte R$ 50,00 de base e R$ 2,50 de reajuste, mesmo que o fator corrente seja 8%. Toda correção preserva os documentos e a versão anterior.

### MF-01

**Origem:** Documentada. **Issue primária:** [48](../issues/048.md).

**Fonte:** [SIC-042](https://www.sicop.sc.gov.br/download/sicop/Pacote_de_alteracoes_02_2017_MedicaoFinalERecebimentoProvisorio.pdf). páginas 1-5; precedência sobre SIC-048 e SIC-046.

**Fontes complementares:** [SIC-048](https://www.sicop.sc.gov.br/download/sicop/Evolucao_01_2016.pdf), páginas 3-4, regra anterior e remissão à alteração 02/2017; [SIC-046](https://www.sicop.sc.gov.br/download/sicop/Pacote_de_alteracoes_03_2016_data_de_referencia_do_pre-aditivo_e_aditivo.pdf), página 3, regra anterior.

**Regra de origem:** Fluxo de medição final de 2017 prevê termo tanto para saldo remanescente quanto para ausência de saldo. Substitui a exigência absoluta de 100% dos quantitativos dos pacotes de 2016.

**Comportamento privado:** Permitir final com saldo remanescente claramente discriminado e aceito no instrumento de recebimento adequado. Não fabricar medição de serviço não executado nem reduzir o contrato silenciosamente. Saldo técnico, valor não executado e obrigações financeiras ficam separados até seu tratamento formal.

**Aceite verificável:** Contrato de 100 unidades com 90 executadas permite preparar final de 90, com 10 remanescentes expressas no termo. Não é permitido preencher automaticamente as 10 faltantes nem liberar pagamento sem cumprir MF-02. Final sem saldo usa documento correspondente.

### MF-02

**Origem:** Adaptada. **Issue primária:** [48](../issues/048.md).

**Fonte:** [SIC-042](https://www.sicop.sc.gov.br/download/sicop/Pacote_de_alteracoes_02_2017_MedicaoFinalERecebimentoProvisorio.pdf). páginas 1-5.

**Regra de origem:** Cálculo definitivo da medição final conclui o contrato, mas medição aguarda recebimento provisório. Termo assinado é anexado como Recebimento; liberação específica coloca a medição em Concluída e permite solicitação de pagamento.

**Comportamento privado:** Validar contrato, tipo e versão do documento, signatários exigidos e evidência da assinatura. Ação de liberação fica restrita ao responsável autorizado. Contas a pagar/receber substituem integração pública; o vínculo financeiro só é habilitado após liberação. Upload isolado não equivale a aprovação nem assinatura validada.

**Aceite verificável:** Após cálculo final, tentativa de faturar medição aguardando termo é rejeitada. Documento de outro contrato ou sem assinaturas exigidas não libera. Termo válido mais comando autorizado permite o fluxo financeiro. A tela distingue contrato Concluído e medição aguardando recebimento.

### MF-03

**Origem:** Adaptada. **Issue primária:** [41](../issues/041.md).

**Fonte:** [SIC-039](https://www.sicop.sc.gov.br/download/sicop/Pacote_de_alteracoes_2019_1.pdf). páginas 1-4.

**Regra de origem:** Supressão de serviço já medido ocorre em medição em Conferência, com total positivo. Lote passa por conferência e aprovação integral. Rejeição não aplica supressão. Anulação exige justificativa e aprovação próprias.

**Comportamento privado:** Nomear operação Correção de quantidade já medida para diferenciá-la de redução do escopo por aditivo. Preservar estados Em conferência, Aguardando aprovação, Aprovado, Reprovado e Anulado. Cada lote é atomicamente aprovado ou rejeitado. Reverter base e reajuste efetivamente reconhecidos no segmento e período de origem; não aplicar índice corrente à quantidade antiga. Anulação após efeito financeiro cria compensações e reconciliação, sem apagar documento ou pagamento anterior.

**Aceite verificável:** Lote com dois itens não pode aprovar apenas um. Medição com resultado total zero ou negativo é rejeitada neste fluxo; acerto isolado segue correção financeira controlada. Reprovação mantém valores anteriores. Anulação de correção já faturada exige rastrear e compensar efeitos financeiros, sem deletá-los.

### CR-01

**Origem:** Adaptada. **Issue primária:** [42](../issues/042.md).

**Fonte:** [SIC-002](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloContrato.2025.pdf). §§4.3.2 e 4.3.2.1; páginas impressas 32-36.

**Fontes complementares:** [SIC-050](https://www.sie.sc.gov.br/webdocs/sie/legislacao/estadual/in/in-04-2026.pdf), art. 14; página física 7.

**Regra de origem:** Cronograma zero representa proposta, com prazo e valor originais sem aditivos. Reprogramações têm número superior a zero. Cada grupo totaliza 100% e seu valor contratado; execução acumulada anterior entra no primeiro período da reprogramação.

**Comportamento privado:** Implementar base inicial e registro de versões na issue 26; reprogramação integrada na issue 42. Base original conserva datas, valor e distribuição aprovada. Cada prorrogação de execução já na issue 40 deve vincular nova versão válida, memória de cálculo e instrumento, sem aguardar a interface ampliada da issue 42. Primeiro e último marcos não podem ser excluídos. Reprogramação separa executado imutável de previsão futura, sem reescrever percentuais efetivamente medidos.

**Aceite verificável:** Grupo contratado de R$ 100.000,00, com R$ 30.000,00 já executados antes da nova versão, inicia reprogramação com 30% executados e distribui R$ 70.000,00 restantes. Total de 90% é rejeitado. Editar a reprogramação não altera cronograma zero ou medições anteriores.

### CR-02

**Origem:** Decisão. **Issue primária:** [05](../issues/005.md).

**Fonte:** [SIC-002](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloContrato.2025.pdf). §4.3.2.1; página impressa 35, exemplo mensal e tabela de intervalos contraditórios.

**Regra de origem:** Exemplo do manual usa meses civis e tabela indica mensal de 30 dias; não há regra única comprovada para todos os intervalos.

**Comportamento privado:** Oferecer políticas explícitas DIAS_FIXOS_SICOP, com 30/60/90/180/365 dias para mensal a anual, e CALENDARIO_CIVIL, padrão privado com meses e anos civis. Semanal usa sete dias e quinzenal quinze dias corridos. A política de cadência fica congelada no cronograma zero. Não confundir cadência de parcelas de planejamento com prazo contratual: meses/anos do instrumento são convertidos em dias equivalentes preservando a data final, conforme CT-05 e SIC-050. Âncora e ano bissexto seguem calculos.md. Comparação direta exige mesma política; conversão explícita pode gerar projeção comparável sem editar as bases aprovadas.

**Aceite verificável:** No calendário civil, mensal com âncora 31/01 passa pelo último dia de fevereiro e retorna à âncora de março. Em dias fixos, mensal avança 30 dias. Instrumento com fim 28/02 conserva essa data e recebe período residual, mesmo com cadência de 30 dias. Comparação de políticas diferentes exige projeção convertida identificada; bases aprovadas não mudam.

### RC-01

**Origem:** Adaptada. **Issue primária:** [48](../issues/048.md).

**Fonte:** [SIC-003](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloMedicao.2010.0002.pdf). §5.2.2; páginas impressas 30-31.

**Regra de origem:** Recebimento provisório exige contrato Concluído. Sua data é igual ou posterior ao fim da medição final. Depois de registrado definitivo, provisório não pode ser excluído.

**Comportamento privado:** Usar ReferenciaEncerramento discriminada: medição final financeira ou registro técnico sem cobrança. O segundo contém dataConclusao, comissão, saldo residual e última medição financeira opcional, sem criar título fictício. Data do termo deve ser igual ou posterior ao fim da referência correspondente. Registrar responsáveis, representantes, observações e saldo financeiro pendente. Preservar termo assinado e anexos. Depois de definitivo, bloquear exclusão do provisório; esse cenário integrado é verificado nas issues 49 e 70. Recebimento não reduz faturas nem comprova quitação.

**Aceite verificável:** Medição final com fim 30/06 rejeita provisório 29/06 e aceita 30/06. Registro técnico sem cobrança com conclusão 30/06 usa a mesma guarda e não exige medição final financeira fictícia. Contrato ainda Em andamento não recebe provisório. Na integração 49/70, existência de definitivo impede excluir provisório. Saldo financeiro pendente continua no painel.

### RC-02

**Origem:** Adaptada. **Issue primária:** [49](../issues/049.md).

**Fonte:** [SIC-003](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloMedicao.2010.0002.pdf). §5.2.3; páginas impressas 32-33.

**Regra de origem:** Recebimento definitivo exige provisório e data igual ou posterior. Registra pagamento final, habite-se, projeto como construído, programa de manutenção, observações e representantes.

**Comportamento privado:** Cada obrigação técnica fica como Atendida, Pendente ou Não aplicável justificado. A matriz contratual e local define quais impedem aceite. Não presumir que todo campo precisa ser afirmativo nem que um registro equivale a documento regular. Pagamento final é fato financeiro conciliado, separado de aceite técnico.

**Aceite verificável:** Definitivo sem provisório ou com data anterior é rejeitado. Habite-se obrigatório pendente impede aceite conforme matriz aplicável. Item não aplicável precisa motivo e aprovador. Definitivo sem pagamento integral não fecha automaticamente contas em aberto.

### RC-03

**Origem:** Adaptada. **Issue primária:** [49](../issues/049.md).

**Fonte:** [SIC-011](https://www.sicop.sc.gov.br/download/sicop/Manual_de_Licitacoes_e_Contratos_de_Obras_Publicas.pdf). §§7.14.1-7.14.3; páginas impressas 91-93.

**Regra de origem:** Manual público prevê vistoria, registro e correção de defeitos, verificação do funcionamento e emissão dos termos. Prazos e formalidades públicas não são automaticamente aplicáveis a contrato privado.

**Comportamento privado:** Adotar vistoria privada por escopo, lista de pendências, criticidade, responsável, prazo e evidências de correção. Pendência impeditiva precisa ser encerrada ou ter tratamento contratual válido antes do aceite correspondente. Preservar programa de manutenção, documentos técnicos e garantias da obra no dossiê de entrega. Aprovação de usuário não dispensa obrigação técnica legal aplicável.

**Aceite verificável:** Pendência impeditiva aberta bloqueia aceite definitivo e informa responsável e providência. Anexar foto sem análise não a encerra. Fechamento autorizado mantém antes/depois e data. Termo emitido referencia a versão exata da vistoria e dos anexos.

### RC-04

**Origem:** Decisão. **Issue primária:** [50](../issues/050.md).

**Fonte:** [SIC-049](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloContrato.2010.0002-AG.pdf). §5.4; páginas impressas 88-89, cadastro de encerramento sem fórmula universal de acerto.

**Regra de origem:** O cadastro documentado de rescisão/cancelamento não define fórmula universal de indenização, multa, retenção ou saldo de encerramento.

**Comportamento privado:** Consolidar posição de serviços, medições, aditivos, faturas, pagamentos, recebimentos, adiantamentos, retenções e garantias. Cada ajuste financeiro exige base contratual, memória, evidência, autorização e contrapartida. Não deduzir indenização automática de regra pública. Termo de encerramento e quitação financeira são registros independentes.

**Aceite verificável:** Contrato encerrado com R$ 5.000,00 de retenção e R$ 2.000,00 de adiantamento não recebe saldo arbitrário. Usuário autorizado registra tratamento contratual de cada verba e a conciliação demonstra os efeitos. Reabrir consulta conserva posições e documentos históricos.

## Interface e operação

A tela deve mostrar o estado atual, quem responde pelo próximo ato, o impedimento específico e a ação disponível. Uma ação indisponível explica o que falta e oferece acesso direto ao registro pertinente. Confirmação é reservada a efeitos relevantes, como aprovação, aplicação de aditivo, liberação final e encerramento; não deve aparecer a cada salvamento de rascunho.

Na revisão de mudança, mostrar quantidade, preço, valor, prazo, documentos e responsáveis antes e depois. Acréscimos e supressões aparecem separados. A data eficaz do ato deve ser visualmente distinta da data de registro. Em operações financeiras e de aprovação, servidor Rust revalida permissões, versão do registro, saldo e política antes de gravar qualquer efeito.

A versão estruturada correspondente está em [contratos.json](contratos.json). Alterar regra, evidência ou critério de aceite exige manter os dois arquivos sincronizados.
