# PRD | Canteiro 360

**Versão da especificação:** 1.0, 11/11/2026.  
**Responsável pelo produto:** Tiago Sasaki / CONFENGE.  
**Mercado inicial:** gestão profissional de obras e projetos privados no Brasil.  
**Estado:** fundação da issue 1 implementada; módulos de negócio ainda em implementação futura, sem liberação comercial.  
**Diretriz técnica:** Rust no servidor e no front-end, com experiência rápida, acessível e intuitiva.

## 1. Problema e resultado esperado

A gestão da obra perde confiabilidade quando orçamento, contrato, cronograma, execução, medição, documentos e caixa são mantidos em registros independentes, sem uma versão reconhecida e sem indicação de quem aprovou cada alteração. O produto deve manter essas informações conectadas e explicar suas diferenças, permitindo ao gestor agir sobre saldo, prazo, qualidade e recebimento com evidência.

O resultado esperado é um serviço pelo qual uma empresa possa contratar, operar e pagar, com implantação, atendimento, segurança e exportação dos próprios dados. A versão 1.0 atende construtoras, gerenciadoras, escritórios de engenharia e proprietários com gestão profissional. Não presume que todas essas organizações tenham o mesmo papel: um contrato pode ser receita para uma e despesa para outra.

## 2. Fidelidade funcional e fontes

Base funcional: [SICOP de Santa Catarina](https://www.sicop.sc.gov.br/sicop/oquee.html?height=240&width=500), seus [manuais públicos](https://www.sicop.sc.gov.br/download/manuaisSicop.jsp), pacotes de alteração e orientações. A matriz normativa do produto é a [rastreabilidade](docs/MATRIZ-RASTREABILIDADE.md), com fontes, localização, adaptação, issue e teste.

Preservar as regras documentadas de gestão e operação aplicáveis. Identificar explicitamente a substituição de órgãos, documentos e integrações públicas por funções privadas equivalentes. A interface é própria; não há requisito de reproduzir telas antigas. Funcionalidades comerciais, segurança moderna, usabilidade e detalhes de algoritmos ausentes dos manuais são decisões adicionais.

Não atribuir à fonte informação que ela não publica. A cobertura pretendida é de todas as regras da matriz aprovada. Igualdade total com o sistema real, incluindo comportamentos ocultos ou contraditórios, não é demonstrável apenas pelos manuais. Os [conflitos conhecidos](docs/decisoes-e-lacunas.md) têm tratamento definido e teste, sem equivalência fictícia.

## 3. Objetivos e indicadores

| Objetivo | Indicador de aceite |
|---|---|
| Confiabilidade financeira | Zero centavo inexplicado nos cenários de referência e na reconciliação integrada |
| Controle de responsabilidades | Toda decisão definitiva identifica autor, organização, papel, versão, data e efeito |
| Autonomia de uso | Pelo menos 90% de sucesso sem assistência nas jornadas de usabilidade definidas, com cinco participantes representativos |
| Velocidade percebida | Metas de LCP, INP, CLS, navegação, tabelas e servidor de `docs/experiencia.md` atendidas |
| Isolamento | Nenhum acesso indevido entre organizações, inclusive documentos, busca, filas e exportações |
| Continuidade | Restauração comprovada com RPO até 1 hora e RTO até 4 horas na configuração de lançamento |
| Utilidade profissional | Duas organizações independentes completam os cenários de piloto e aprovam seus resultados |
| Viabilidade comercial | Contratação, cobrança, atendimento e custo por organização documentados e operantes |

São metas de produto, não resultados já alcançados. Medição de uso real exige volume de dados suficiente; antes do lançamento usar protocolo de laboratório identificado, sem apresentar amostra inexistente como estatística de produção.

## 4. Usuários, escopos e alçadas

| Perfil | Trabalho principal | Limite essencial |
|---|---|---|
| Administrador da organização | Configurar empresa, plano, pessoas e política | Não se torna responsável técnico nem aprovador financeiro automaticamente |
| Gestor de carteira/obra | Acompanhar prazo, custo, contratos e decisões | Atua somente nos empreendimentos concedidos |
| Orçamentista | Montar e revisar custos, serviços e cronogramas | Não aprova a própria proposta quando houver segregação exigida |
| Fiscal/gerenciador técnico | Registrar execução, conferir medição e propor alteração | Não cria baixa financeira por ato técnico |
| Responsável técnico | Assumir função técnica e apresentar registros | Usuário digitador e responsável técnico são entidades distintas |
| Aprovador do contratante | Decidir conforme alçada sobre orçamento, OS e alterações | Aprova a versão exata; acréscimo misto não é ocultado por compensação |
| Financeiro | Faturar, conciliar e registrar caixa | Não altera quantidade executada para fazer a conta fechar |
| Fornecedor/contratado | Apresentar documentos e execução de seu escopo | Não vê propostas concorrentes nem outras obras |
| Cliente/proprietário | Consultar entregas e realizar os aceites concedidos | Visão e ações limitadas ao vínculo |
| Suporte do serviço | Atender e investigar falhas | Acesso temporário autorizado; sem poder de aprovar atos do cliente |

Uma pessoa pode exercer funções compatíveis, mas a política deve impedir conflito de interesse nos atos definidos como segregados. Configurar delegação, substituição, início/fim de atuação e limites de valor; a delegação nunca amplia a autoridade de quem delega. O último administrador necessário não pode ser removido sem sucessor.

## 5. Jornada principal

1. A organização configura seus responsáveis e importa ou cadastra empreendimento, obra e projeto.
2. O orçamentista monta custos, serviços e cronograma, confere inconsistências e submete a revisão.
3. O contratante compara cotações e formaliza escopo, preço, regime, datas, responsabilidades e política de cálculo.
4. A equipe recebe a autorização de início com a versão contratada, os projetos liberados e os registros técnicos exigidos.
5. A execução alimenta diário, fotos, qualidade, avanço, revisões de projeto e ocorrências.
6. O fiscal prepara medição por acumulados; conferência, cálculos e aprovação preservam as evidências e os saldos.
7. Alterações passam por proposta, decisão e instrumento formal antes de afetar preço, quantidade e prazo aplicáveis.
8. O financeiro usa parcelas liberadas para faturar, registrar retenções e adiantamentos e conciliar pagamentos ou recebimentos.
9. A obra passa por recebimentos, pendências, garantias e encerramento, mantendo separado o saldo financeiro.

A navegação pode acontecer fora dessa ordem quando não houver dependência real. A ordem de implementação das issues não obriga o usuário a percorrer menus desnecessários. Cada tela mostra o próximo ato útil e por que está ou não disponível.

## 6. Requisitos por domínio

### 6.1 Organizações e cadastros

Empresas independentes, associação de usuários, unidades, obras, centros de custo, clientes, fornecedores, profissionais e ativos. Guardar CPF e CNPJ como texto; aceitar CNPJ numérico e alfanumérico conforme Receita, com testes nos formatos de entrada, busca, API e exportação. Endereço brasileiro, município, UF e fuso da obra são explícitos. Datas civis não se deslocam por conversão UTC; eventos de auditoria guardam instante e fuso de apresentação.

Duplicação de identidade dentro de uma organização é tratada; a existência de registro em outra organização permanece sigilosa. Papéis, vínculos e alçadas são conferidos em todas as consultas e mutações. [Requisitos brasileiros](docs/regras/produto.json) e [modelo de dados](docs/modelo-dados.md).

### 6.2 Carteira, obras e projetos

Empreendimento agrupa obras e contratos. Um contrato pode atender várias obras; uma obra pode ter vários contratos e projetos. Ativos podem ser próprios, locados ou de terceiros; intervenções lineares admitem trechos, municípios e coordenadas. Cada obra mantém orçamento de referência, custo previsto, execução e saldo, sem perder o vínculo com o contrato que originou o compromisso.

Projeto representa um escopo técnico com disciplinas, entregas, revisões, responsáveis, marcos e documentos. A versão inicial inclui tarefas, dependências, calendário, risco, problema, solicitação de esclarecimento e solicitação de mudança. Não se confunde com editor CAD/BIM. O detalhamento desse domínio é uma capacidade própria, pois não foi localizado manual público suficiente do módulo de Gestão de Projetos do SICOP para atribuir-lhe todas essas regras.

### 6.3 Custos e orçamentos

Tabelas de referência identificam fonte, competência, localidade, regime e autorização de uso. Preços históricos não são apresentados como atuais. Suportar insumos, mão de obra, equipamentos, transporte, serviços, coeficientes, incidências, custo informado, encargos e BDI com memória rastreável. Não aceitar fórmulas arbitrárias executáveis nem ciclos entre composições.

Orçamento tem variação de escopo e revisão temporal separadas. Estrutura grupo, etapa, subetapa e serviço, com quantidade, unidade e preço. Guardar autor de elaboração e responsável técnico. Planilha em consulta passa a edição por ação explícita e alocação com prazo; concorrência continua protegida por versão e transação.

Cronograma orçamentário integra a versão antes da aprovação. Copiar para atualizar preços e duplicar preservando valores são operações distintas, com prévia. Importação de CSV/XLSX oferece mapeamento, validação de convenção numérica, simulação e confirmação atômica. A planilha exportada conserva IDs, origem e valores; não promete importar qualquer arquivo SICOP sem contrato de formato.

Conferência apresenta os erros e leva ao campo de correção. Finalizar e aprovar são atos distintos. Orçamento aprovado e vinculado a contrato não pode ser desaprovado ou recalculado silenciosamente. A edição de uma nova revisão não modifica a versão contratada. [Regras ORC e EPG](docs/regras/orcamentos-medicoes.md).

### 6.4 Cotações e contratação privada

Solicitação de cotação referencia uma revisão de escopo. Propostas têm fornecedor, validade, exclusões, prazo, preço e documentos. A comparação equaliza itens e explicita ausências; o preço menor não implica escolha automática. A decisão aprovada gera origem contratual rastreável, sem publicar preços concorrentes.

Contrato registra partes, posição da organização, objeto, tipo, regime unitário/global, escopo, preço, datas, periodicidade, reajuste, retenções, adiantamentos, garantias, documentos e alçadas. A aplicação admite construção, reforma, projeto, supervisão, consultoria e manutenção. Regras de consultoria com incidências percentuais são expressas e não podem criar ciclos de base.

Vigência jurídica e execução possuem datas e prazos independentes. A execução fica integralmente contida na vigência; início pela OS e contagem de dias a partir do dia seguinte. Preservar o prazo e a data final do instrumento, inclusive quando convertido de meses para dias. Assinatura, autorização técnica e cadastro são atos próprios. O sistema identifica o documento formal e sua integridade, sem declarar que um clique interno é assinatura qualificada. O início exige ordem/autorização, responsáveis, documentos aplicáveis e base aprovada. Campos originais deixam de ser livremente editáveis após atos que geram efeitos. [Regras CT, RT e AD](docs/regras/contratos.md).

### 6.5 Cronogramas e execução

Cronograma contratual zero conserva a proposta inicial. Reprogramações têm número, motivo, data e bases econômicas identificadas. Grupos totalizam 100%; rateios conservam o total. O executado anterior permanece reconhecido, e uma mudança de prazo não reescreve o plano original. Políticas de calendário aparecem no contrato e nos relatórios.

Diário registra data, local, responsáveis, equipe, clima quando relevante, serviços, impedimentos e documentos. Fotos preservam original, origem e vínculo temporal. Qualidade registra requisito, não conformidade, responsável, prazo, correção e aceite. Advertência formal mantém versões e evidências de envio; não substituir ocorrência técnica por acusação automática.

Paralisação guarda termo, causa, escopo afetado, data, responsável, percentual físico e evidências. Bloquear autorização de novos atos de execução durante a pausa no escopo afetado, preservando atos administrativos e o registro comprovado de execução anterior. Ocorrência factual de execução irregular pode ser registrada, com alerta e evidência, sem autorizá-la nem liberar valor financeiro automaticamente. Retomada exige ordem formal e apura os dias pela política fixada. O efeito sobre o prazo de execução é consolidado por registro privado de ajuste, equivalente funcional ao apostilamento, com memória e cronograma atualizado. Isso não estende a vigência: quando necessário, exigir aditivo específico antes de autorizar prazo fora dela. Suspensão parcial não prorroga automaticamente toda a obra. Distinguir cronograma previsto, compromisso contratual e previsão atual. Essa precedência incorpora os arts. 4 a 15 da IN SIE 04/2026, sem impor publicação em diário oficial, SGPe ou prorrogação legal automática a contratos privados. [Cálculos](docs/regras/calculos.md) e [operação](docs/regras/operacao.md).

### 6.6 Medições

Capa identifica contrato, obra, período, sequência, tipo e circunstâncias. Entradas são acumuladas. A tabela mostra anterior, atual, diferença do período, valor e saldo. Para preço global, usar estrutura e avanço de etapas conforme a orientação de 2025. O servidor recalcula os valores e confere os limites no momento de consolidar. Percentual físico relativo ao contrato pode diminuir por aumento formal do denominador, preservando execução absoluta e histórico; isso não autoriza reduzir acumulados sem o fluxo próprio de correção.

Comissões e profissionais são válidos para o período; fotos e documentos pertencem ao escopo e à data pertinente. Reprovação de item exige motivo. Registro incompleto sincronizado do campo continua rascunho; nunca se transforma em aprovação financeira. Medição definitiva deixa de ser editável e conserva os valores, a versão contratual e a política de cálculo.

Competência sem execução pode ser registrada sem criar título. Medição financeira exige total positivo conforme decisão que resolve o conflito documental. Correção de serviço já reconhecido reverte o preço e reajuste originais de cada segmento, sem aplicar o índice atual à base líquida de ciclos diferentes. Ajustes negativos usam fluxo aprovado de crédito/compensação. Concluir tecnicamente sem nova cobrança não exige inventar valor positivo. [MED, SUP e decisões de cálculo](docs/regras/orcamentos-medicoes.md).

### 6.7 Alterações e reajustes

Pré-aditivo é proposta; aditivo é incorporação formal. Manter justificativa por alteração, comissão, quantidades, valores, dias e impactos. Alteração mista com acréscimo e redução segue alçada de acréscimo mesmo que o saldo líquido seja zero. Data eficaz não pode reescrever medição já consolidada. Reaplicar instrumento não duplica saldo nem prazo.

Supressão de serviço já medido é correção de execução, distinta de redução de escopo futuro. O lote inteiro é aprovado ou rejeitado; não aplicar parcialmente uma lista decidida como unidade. Rejeição e anulação conservam a história e exigem tratar as consequências financeiras que já ocorreram.

Reajuste pode ser por obra ou serviço, provisório, definitivo e complementar. Índice, competência, data-base e data do preço são referências próprias. Parâmetros ficam congelados após o início de medições, com caminho controlado de correção. Não aplicar ao serviço novo aniversário de preço que não lhe pertence. [RJ, AD e MF](docs/regras/contratos.md).

### 6.8 Financeiro operacional

Contratado, executado, medido, aprovado, faturado, vencido, pago e recebido devem aparecer separadamente. O produto trata contas a pagar e a receber na perspectiva da organização. Cada título referencia origem e alocação; uma parcela não pode ser faturada ou baixada acima do saldo. Existem pagamento parcial, estorno, crédito, adiantamento, amortização e retenção.

Registrar documentos fiscais e seus metadados, vencimentos, comprovantes e conciliação. A emissão fiscal completa de qualquer município não faz parte da versão 1.0; integração ou registro externo rastreável atende o fluxo. A tributação é parametrizada conforme política revisada do cliente, sem inferência universal.

Compras diretas podem gerar compromissos e títulos vinculados à obra, com centro de custo e aprovação, sem depender de um contrato de execução fictício. Materiais e subcontratos já incluídos em outro compromisso não são novamente somados como custo. O custo previsto final identifica realizado, saldo comprometido e previsão ainda não comprometida. Caixa realizado vem de baixas conciliadas, não de medição aprovada.

### 6.9 Recebimentos e encerramento

Medição final calculada pode colocar contrato em Concluído, mantendo a liberação financeira pendente do termo provisório assinado. O termo distingue saldo remanescente e escopo medido integralmente. O pacote 2017 prevalece sobre o bloqueio absoluto de 100% encontrado em versões anteriores.

Recebimento provisório, definitivo, encerramento contratual e quitação financeira têm datas e estados independentes. O definitivo exige provisório e documentação aplicável, como projeto conforme executado, licenças, orientações de manutenção e garantias. Ausência justificada de um documento não deve ser escondida como arquivo vazio; a política de aplicabilidade identifica quem a aprovou.

Rescisão ou cancelamento preserva execução, obrigações, documentos e saldos. Encerrar tecnicamente não apaga títulos nem permite retroagir livremente. Garantias e correções posteriores continuam vinculadas à obra e à entrega recebida.

### 6.10 Manutenção

Acordos quadro privados substituem atas públicas, com lote, catálogo, BDI/desconto, vigência e limite financeiro. Pedido segue solicitação, análise, elaboração de OS, decisão, execução, fiscalização, medição e financeiro. Aprovar a necessidade não equivale a aprovar a OS.

Saldo é reservado e consumido por operações distintas: disponível = limite vigente - consumido reconhecido - reserva remanescente. Confirmação técnica transfere reserva em consumo; medição e fatura não o debitam novamente. Cancelamento só devolve o remanescente não executado, mantendo valores em divergência protegidos. Atualizar o catálogo não adiciona serviço automaticamente ao pedido. Finalizar OS não duplica medição. O fluxo e os papéis são descritos nas regras OP07 a OP14 e ATA. Ajustes solicitados voltam para a etapa pertinente com motivo e versão.

### 6.11 Projetos, colaboração e portal

Revisões documentais têm estados claros: elaboração, revisão, devolvido, aprovado, emitido e substituído. Distribuição informa quem recebeu qual versão. Solicitação técnica tem pergunta, prazo, responsável, resposta e ligação com decisão que alterou custo ou execução. Riscos e problemas têm responsáveis e medidas concretas; uma mudança de projeto que afeta contrato não modifica esse contrato sem seu fluxo próprio.

Uso de campo na versão 1.0 significa conexão intermitente em aba previamente carregada. Rascunhos persistem no dispositivo; abrir ou reabrir a aplicação exige conexão, depois da qual os rascunhos são recuperados e reconciliados. Não prometer inicialização sem rede. Atos definitivos continuam exigindo servidor.

Portal do cliente e fornecedor é privado, por convite e escopo. Compartilhamento por link tem finalidade, expiração, revogação e auditoria, sem expor toda a organização. Publicidade de obras ou valores exige decisão própria do cliente; o mapa da carteira não é público por padrão.

### 6.12 Relatórios, migração e integrações

Caderno de medição, resumo por grupo, contratado versus executado, cronogramas, alterações, recebimentos, histórico, diário, qualidade, pagamentos e caixa podem ser consultados e exportados. Relatórios carregam organização, escopo, versão, data de corte e política de cálculo. Exportação obedece ao acesso e preserva totais; PDF não deve cortar tabelas ou omitir itens.

Migração de contratos em andamento usa modo identificado: importar saldos de abertura, medições históricas, preços, evidências e diferenças sem recalcular o passado pelo motor novo. Exigir simulação, totais de controle e aceite. Não anunciar conexão automática a bancos do SICOP.

API documentada possui escopo, versionamento, limites e idempotência. Assinatura, e-mail e cobrança usam provedores habilitados e ambientes separados. Repetição ou atraso de eventos externos não duplica assinatura, pagamento ou notificações. O serviço conserva a capacidade de exportar a organização ao encerrar a relação.

## 7. Adaptações privadas obrigatórias

| Referência pública | Equivalente no Canteiro 360 | Limite da equivalência |
|---|---|---|
| Órgão/unidade gestora | Organização, unidade e centro de custo | Sem acesso público aos dados |
| Bem público/título de obra | Ativo, empreendimento e obra | Cadastro privado com permissões |
| Fiscal e gestor | Representantes técnicos e gestores designados | Habilitação profissional permanece específica |
| Ordenador/GGG/autoridades | Aprovadores e alçadas da empresa | Percentuais e competências definidos em política privada |
| Licitação e proposta | Cotação, comparação e decisão de contratação | Sem ritos legais públicos automáticos |
| Empenho/liquidação/SIGEF | Compromisso, parcela liberada, título e caixa | Não alegar identidade contábil ou integração estatal |
| Ata de manutenção | Acordo quadro e ordens vinculadas | Limite contratual e reserva próprios |
| SGPe/e-Sfinge | Documentos, evidência e API privada | Sem envio ou uso de credenciais públicas |
| Mapa da Transparência | Carteira e mapa com acesso privado | Divulgação somente quando autorizada |

## 8. Experiência e qualidade visual

Interface por tarefa, com contexto de empresa, obra, contrato e revisão sempre reconhecível. Informações frequentes têm prioridade; detalhes técnicos aparecem quando ajudam a decisão. O usuário deve saber o estado, o próximo responsável, o valor e a consequência antes de concluir um ato.

Formulários preservam rascunho, validam por campo, resumem pendências e não apagam trabalho por erro de rede. Tabelas têm busca, filtro, teclado, cabeçalho e totais claros. Operações de aprovação mostram comparação antes/depois e eventual divergência. Uma ação pode ser bloqueada por regra, mas a mensagem informa o motivo, a correção e o responsável.

Design consistente, tipografia legível, espaçamento, contraste, estados de foco, componentes de dados densos e versão móvel para o campo. Alvos de toque e alternativa ao arraste seguem requisitos de acessibilidade. Nenhuma etapa depende exclusivamente de cor, gesto ou memória do usuário. [Especificação completa de experiência](docs/experiencia.md).

## 9. Segurança, privacidade e confiabilidade

Autenticação, MFA nos perfis privilegiados, recuperação segura, isolamento por organização, menor privilégio, trilha imutável e proteção contra duplicidade são fundações. Toda mutação é autorizada no servidor. Documentos têm integridade, armazenamento privado e controle de versões. Suporte não recebe acesso universal silencioso.

Privacidade cobre finalidades, papéis de controlador/operador, direitos, retenção, exportação, suboperadores e incidentes. Prazos legais têm fonte e enquadramento; não presumir exceção por porte. Retenção de auditoria e obrigações documentais deve ser conciliada com eliminação e minimização, por política justificada. [Prontidão comercial](docs/prontidao-comercial.md).

SLO inicial de disponibilidade: 99,5% por mês, com método de medição e manutenção contabilizada. RPO até 1 hora e RTO até 4 horas, demonstrados por restauração de banco, objetos e configurações necessárias. Monitorar erros, fila, armazenamento, consultas, custo e recuperação. Relatórios de saúde não podem vazar dados de clientes.

## 10. Comercialização e limites da versão 1.0

O plano comercial deve definir preço, unidade de cobrança, limites, período de avaliação, política de inadimplência, suporte, armazenamento e saída. O preço não é arbitrado por este PRD: a issue 68 exige tabela aprovada, custo por organização e margem de contribuição demonstrada antes do lançamento. A unidade inicial recomendada é organização com faixas de obras ativas, armazenamento e usuários, validada no piloto.

Suspensão por inadimplência não pode destruir documentos ou impedir o procedimento de exportação previsto no contrato. Cobrança do SaaS é separada do financeiro das obras. Checkout, assinatura, cancelamento, reembolso autorizado, emissão/registro do documento fiscal do serviço e conciliação precisam funcionar. Faturamento comercial não é recebimento efetivo.

Não fazem parte desta versão autoria CAD/BIM, cálculo estrutural, execução automática de transferências bancárias das obras, folha, contabilidade completa, certificação regulatória universal ou publicação de obras públicas. A ausência dessas funções não pode impedir o ciclo operacional privado coberto. Se o piloto demonstrar uma necessidade indispensável ao ciclo, corrigir o plano e implementar antes da issue 72.

## 11. Sequência e critério de conclusão

O [backlog](docs/BACKLOG.md) possui 72 issues com dependência da etapa anterior e critérios específicos. A issue 01 estabelece a aplicação Rust compilável; as etapas seguintes entregam capacidades operáveis e testadas. Cenários que combinam módulos posteriores são identificados como integração na issue 70, sem impor dependência futura oculta à issue primária.

Concluir issue exige código integrado, migrações, interface, autorização, auditoria, testes, documentação e revisão proporcional ao risco. Nenhum dado demonstrativo pode ser apresentado como transação real. Falhas de cálculo, perda de histórico, vazamento, impossibilidade de recuperação, cobrança incorreta ou bloqueio de jornada obrigatória impedem o lançamento.

A issue 72 verifica a versão publicada e suas evidências. Fechar tickets ou compilar código não equivale a estar pronto para uso comercial. A liberação exige os testes da issue 70, os pilotos da 71 e todas as condições de [prontidão](docs/prontidao-comercial.md), incluindo contratação e recebimento do serviço quando autorizados no piloto.
