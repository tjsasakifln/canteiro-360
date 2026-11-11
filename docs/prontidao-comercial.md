# Prontidão comercial

Especificação de liberação de 11/11/2026. O repositório define o produto e sua implementação em 72 etapas. Este documento não declara que o software já está implementado, homologado, certificado ou disponível para venda.

## 1. Condição para concluir a issue 72

A versão 1.0 somente pode ser liberada quando as issues 01 a 71 estiverem implementadas e verificadas, e todos os critérios deste documento forem comprovados na versão candidata. A issue 72 depende diretamente da 71 e reúne as evidências anteriores, a publicação, a verificação em produção e a decisão de liberação.

Fechar uma issue, compilar o projeto ou publicar uma tela de entrada não demonstra prontidão comercial. O resultado exigido é um serviço utilizável por empresas brasileiras, com regras consistentes, cobrança funcional, atendimento, preservação de dados, recuperação testada e um ciclo real de gestão completo.

Regras extraídas de manuais do SICOP têm sua origem registrada. Adaptações ao setor privado e requisitos adicionais de produto são identificados separadamente. Nenhuma informação ausente nos manuais pode ser apresentada como regra comprovada do SICOP.

## 2. Matriz obrigatória de liberação

| Condição | Issues de origem | Evidência exigida | Impedimento à liberação |
|---|---|---|---|
| Regras e rastreabilidade | 01 a 62, 70 | Regra, origem, teste, resultado e vínculo à implementação; diferenças privadas explicadas | Regra crítica sem implementação, fonte indevidamente atribuída ou comportamento contraditório |
| Fluxos financeiros | 31 a 50, 70 | Memórias independentes, valores exatos, saldo por etapa, estorno e conciliação | Diferença monetária inexplicada, duplicação ou alteração silenciosa de fato aprovado |
| Experiência e acessibilidade | 03, 04, 62, 63, 70, 71, 72 | Relatório das seis tarefas, teclado, leitor de tela, reajuste a 320 CSS px/400% e todos os critérios A/AA aplicáveis | Tarefa crítica inviável, perda de dados, erro financeiro induzido ou critério A/AA aplicável não atendido |
| Desempenho | 01, 04, 64 | Ensaios reproduzíveis, pacotes, tempos, carga, falhas e custo da infraestrutura | Limites acordados não atingidos ou ensaio sem dados/cenário identificados |
| Isolamento e segurança | 06 a 10, 58, 61, 65 | Revisão independente, casos de acesso indevido e correções verificadas | Falha alta ou crítica em aberto; acesso entre organizações; segredo exposto |
| Privacidade | 10, 66, 69 | Inventário de tratamento, termos, retenção, canal de direitos e exercício de incidente | Responsabilidades indefinidas, direito sem atendimento, uso indevido ou descarte sem controle |
| Continuidade | 67 | Restauração completa, tempos medidos, anexos e auditoria conciliados | Backup não restaurado, perda superior ao limite ou recuperação que expõe dados excluídos |
| Contratação do serviço | 68, 69 | Plano e preço publicados, limites, cancelamento, termos versionados e aceite | Oferta impossível de cumprir, preço/limite ambíguo ou aceite não demonstrável |
| Cobrança do serviço | 68, 70, 71 | Contratação, renovação, falha, cancelamento, retorno de pagamento e conciliação | Cobrança duplicada, pagamento sem conciliação ou estado da assinatura incorreto |
| Atendimento | 67, 69 | Responsáveis, canais, horários, procedimento de incidente e ensaio de atendimento | Canal sem responsável ou compromisso sem capacidade demonstrada |
| Pilotos profissionais | 71 | Duas empresas independentes, ciclos completos, aceites e falhas corrigidas | Piloto incompleto, uso fictício apresentado como real ou falha relevante não resolvida |
| Produção | 72 | Domínio, certificado, configuração, migração, observação e teste depois da publicação | Falha funcional, configuração insegura, recuperação indisponível ou versão diferente da homologada |

Os registros da matriz permanecem **a comprovar durante a implementação** até que a evidência da versão correspondente exista. A entrega da documentação não antecipa esses resultados.

## 3. Cobertura operacional mínima

A homologação integrada precisa atravessar os seguintes processos, mantendo as relações entre os registros:

- Cadastro de empresas, pessoas, profissionais, obras, projetos e documentos técnicos.
- Catálogos, composições, orçamento, revisão, conferência e seleção privada de fornecedor.
- Contratos por preços unitários e empreitada global, cronograma, autorização de início e execução.
- Diário, fotos, qualidade, paralisação, retomada e controle de prazo.
- Medições parciais, acumulados, glosas, reajustes, aprovação e caderno de medição.
- Pré-aditivo, alçada, formalização, supressão e reprogramação, preservando o histórico.
- Faturas, títulos, adiantamentos, retenções, pagamentos, recebimentos e conciliação.
- Medição final, recebimentos, ressalvas, garantias, rescisão e encerramento.
- Manutenção por solicitação, reserva, ordem, fiscalização, medição e aceite.
- Gestão de projetos por entrega, revisão, dependência, risco, mudança e distribuição controlada.
- Portal externo, importação, migração, exportação, API e rascunhos de campo.

Uma modalidade não pode passar apenas porque outra usa o mesmo componente. Os testes exercitam as diferenças de regime, saldo, prazo, papel e sequência.

## 4. Prova financeira

### 4.1 Conceitos separados

| Conceito | O que deve representar |
|---|---|
| Contratado | Compromisso vigente, com alterações formalizadas |
| Executado | Fato de execução registrado e suas evidências |
| Medido e aprovado | Serviço e valor conferidos segundo o contrato |
| Faturado | Documento de cobrança vinculado à obrigação |
| A pagar ou receber | Título exigível, vencimento e saldo |
| Pago ou recebido | Movimento efetivo confirmado e conciliado |
| Retido | Valor separado segundo regra contratual, com condição de liberação |
| Previsto | Estimativa futura, sem ser confundida com obrigação ou caixa realizado |

O mesmo valor pode atravessar esses estágios. O painel não os soma como ingressos adicionais. O lado contratante e o lado contratado precisam ser identificados: pagamento de um não é recebimento de caixa da própria empresa do outro sem o respectivo registro e conciliação.

### 4.2 Caso de referência obrigatório

O exemplo abaixo é uma decisão de teste do produto privado, não uma fórmula atribuída ao SICOP nem uma regra tributária. Todos os tributos deste caso são zero e devem estar identificados assim.

| Evento | Valor |
|---|---:|
| Valor apresentado na medição | R$ 10.000,00 |
| Glosa conferida | R$ 500,00 |
| Valor aprovado | R$ 9.500,00 |
| Retenção contratual de 5% sobre o aprovado | R$ 475,00 |
| Adiantamento anteriormente pago e amortizado nesta fatura | R$ 1.000,00 |
| Líquido exigível da fatura | R$ 8.025,00 |
| Pagamento parcial desta fatura | R$ 3.000,00 |
| Saldo exigível desta fatura | R$ 5.025,00 |
| Retenção ainda não liberada | R$ 475,00 |

O caixa já realizado é R$ 4.000,00: adiantamento de R$ 1.000,00 mais pagamento parcial de R$ 3.000,00. A conciliação é `9.500,00 = 4.000,00 + 5.025,00 + 475,00`. Liberar a retenção cria a exigibilidade correspondente, sem criar nova medição nem inflar o valor aprovado. O adiantamento não é contado outra vez como novo pagamento ao ser amortizado.

Também são obrigatórios casos de arredondamento, preço unitário fracionário, concorrência sobre saldo, reenvio do mesmo pagamento, estorno, devolução, glosa revertida, alteração após medição, supressão de serviço medido e recebimento parcial. Cada caso precisa de cálculo esperado independente da implementação.

### 4.3 Limite fiscal e bancário

O produto registra e relaciona documentos fiscais, faturas, valores e comprovantes, e oferece importação/exportação controlada. Não promete apuração tributária nacional completa, geração de todas as obrigações acessórias ou substituição do sistema contábil. Tributos e retenções configurados têm fonte, vigência e responsável pela conferência.

A assinatura do Canteiro 360 é cobrada separadamente das obras geridas pelos clientes. A versão 1.0 não recebe nem mantém sob sua custódia o dinheiro das empreitadas. Conciliação operacional não é afirmação de execução de transferência bancária pelo sistema.

## 5. Requisitos brasileiros

### 5.1 Cadastros e documentos

CNPJ deve aceitar o formato alfanumérico e os números existentes, com 14 posições e os dígitos verificadores previstos pela Receita. Formulários, importação, API, relatórios e cobrança devem usar os mesmos casos oficiais de validação. [Receita Federal, projeto](https://www.gov.br/receitafederal/pt-br/acesso-a-informacao/acoes-e-programas/programas-e-atividades/cnpj-alfanumerico/cnpj-alfa), [documentação de cálculo](https://www.gov.br/receitafederal/pt-br/centrais-de-conteudo/publicacoes/documentos-tecnicos/cnpj).

ART e RRT têm regras próprias. A ART de obra ou serviço deve ser registrada antes do início da atividade técnica nas condições aplicáveis. Para RRT, a tempestividade depende do grupo e dos eventos previstos na norma consolidada. O sistema guarda número, atividade, responsável, conselho, documento, vínculos e estado de conferência. Não declara emissão ou regularidade apenas porque o usuário preencheu um campo. [CONFEA, Resolução 1.137/2023, art. 27](https://normativos.confea.org.br/Ementas/Visualizar?id=76099), [CAU/BR, Resolução 91, art. 2º consolidado](https://transparencia.caubr.gov.br/resolucao91/).

CNO, licenças, alvarás e demais documentos são controlados conforme a necessidade da obra e a regra aplicável. O número do CNO e sua situação são dados de apoio, sem transformar o cadastro local em inscrição na Receita. [Receita Federal, CNO](https://www.gov.br/receitafederal/pt-br/assuntos/construcao-civil/cno/cno).

### 5.2 Assinaturas e evidência

A aprovação de um fluxo e a assinatura de um documento são eventos distintos. O sistema registra quem decidiu, o papel, a versão, o instante e a evidência. Para documentos assinados externamente, preserva o arquivo original, sua identificação de integridade e os elementos de validação disponíveis; a indicação de conferência não substitui uma validação efetiva.

Quando houver assinatura integrada, a issue correspondente deve demonstrar identidade, manifestação de vontade, autoria, integridade e o vínculo com a versão exata, além de cancelamento e reenvio seguros. Outros meios além da ICP-Brasil podem ter validade nas condições do art. 10, § 2º, da MP 2.200-2. O produto não anunciará validade universal ou certificação que não oferece. [Governo Digital, alcance das assinaturas eletrônicas](https://www.gov.br/governodigital/pt-br/identidade/assinatura-eletronica/saiba-mais-sobre-a-assinatura-eletronica).

### 5.3 Garantias e encerramento

Recebimento definitivo, encerramento do contrato e encerramento da conta do software não são sinônimos de extinção de todas as responsabilidades. Garantias e ocorrências pós-obra têm fundamento, prazo, responsável, condições e documentos. O art. 618 do Código Civil trata de uma hipótese específica de solidez e segurança; o produto não deve transformá-lo em prazo único para todos os componentes e direitos. [Código Civil, art. 618](https://normas.leg.br/?urn=urn%3Alex%3Abr%3Afederal%3Alei%3A2002-01-10%3B10406%402021-08-26%21art618).

## 6. Privacidade, segurança e continuidade

### 6.1 Proteção e responsabilidades

Cada operação de tratamento tem finalidade, dados, base aplicável, acesso, prazo de retenção e destino. O fornecedor pode atuar como operador dos dados da obra e controlador de seus próprios dados de conta e cobrança; isso deve ser definido por operação e contrato. A LGPD prevê direitos e medidas de segurança desde a concepção do serviço. [LGPD, arts. 5º, 18, 37 e 46](https://www2.camara.leg.br/legin/fed/lei/2018/lei-13709-14-agosto-2018-787077-normaatualizada-pl.html).

O aceite de privacidade inclui exportar dados e anexos, atender correção e eliminação quando cabíveis, preservar registros com fundamento de retenção e impedir restauração indevida de dados já excluídos. Preservação por disputa ou obrigação deve ser limitada, justificada e revisável. Não existe retenção universal indefinida nem exclusão automática de toda a história ao cancelar uma conta.

Hospedagem, suboperadores e transferências internacionais são inventariados. Se houver transferência internacional, o mecanismo e as informações ao titular precisam atender às regras aplicáveis; hospedar no Brasil não elimina a necessidade de conferir os demais fornecedores. [ANPD, Resolução 19/2024](https://www.gov.br/anpd/pt-br/acesso-a-informacao/institucional/atos-normativos/regulamentacoes_anpd/resolucao-cd-anpd-no-19-de-23-de-agosto-de-2024).

### 6.2 Revisão de segurança

A revisão independente deve testar autenticação, recuperação, sessão, papéis, alçadas, isolamento, busca, arquivos, exportações, integração, repetição de eventos, concorrência, injeção, dependências e segredos. O usuário de uma organização não pode ler, inferir ou modificar o objeto de outra pela troca de identificador ou pela repetição de uma URL antiga.

Se a aplicação mantiver senhas, usar Argon2id por biblioteca apropriada, salt único e parâmetros versionados. A configuração inicial é 64 MiB de memória, três iterações e paralelismo um; a issue 06 mede custo, capacidade e limites de tentativas. Qualquer ajuste precisa de justificativa e não pode ficar abaixo das recomendações aplicáveis da OWASP. Um serviço de identidade aprovado é alternativa, com responsabilidades, configuração e recuperação demonstradas. Senhas não são armazenadas em claro nem protegidas apenas por hash rápido. [OWASP, armazenamento de senhas](https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html).

Recuperação usa token criptograficamente aleatório de 256 bits, validade de 15 minutos, uso único e hash persistido, sem token em claro nos registros. URL de recuperação usa origem aprovada. A recuperação de senha não elimina MFA nem cria sessão privilegiada automaticamente; o segundo fator tem procedimento próprio verificável. [OWASP, recuperação de senha](https://cheatsheetseries.owasp.org/cheatsheets/Forgot_Password_Cheat_Sheet.html).

A identidade única de uma operação é formada por organização, tipo de ato e chave. Guardar e comparar o hash canônico da entrada: repetição idêntica retorna o resultado autorizado; conteúdo diferente retorna conflito. O hash não pode compor uma chave alternativa que permita repetir o efeito com o mesmo identificador e outro valor. O acesso é revalidado antes de devolver resultado anterior. A proteção contra repetição de efeito financeiro permanece vinculada ao evento durável, mesmo quando expira um cache de resposta.

Contas de suporte não recebem acesso permanente a todos os clientes. Um acesso excepcional tem motivo, limite, prazo, responsável e auditoria. A revisão final exige zero falha alta ou crítica em aberto. Um falso positivo somente é descartado com evidência reproduzível e revisão independente.

### 6.3 Incidentes

O procedimento define detecção, contenção, avaliação, responsável, comunicação ao cliente, registro e recuperação. A comunicação legal depende do papel e do risco do incidente. A regra geral da Resolução ANPD 15 é de 3 dias úteis para comunicação pelo controlador nos casos previstos; há condições específicas de prazo em dobro para agentes de pequeno porte elegíveis. O operador informa o controlador sem demora injustificada. Não se presume enquadramento ou dispensa. [ANPD, orientação](https://www.gov.br/anpd/pt-br/canais_atendimento/agente-de-tratamento/comunicado-de-incidente-de-seguranca-cis), [Resolução 2/2022, arts. 3º e 14](https://www.gov.br/anpd/pt-br/acesso-a-informacao/institucional/atos-normativos/regulamentacoes_anpd/resolucao-cd-anpd-no-2-de-27-de-janeiro-de-2022).

O registro de incidentes, inclusive os não comunicados, observa o prazo mínimo específico de cinco anos e eventuais obrigações adicionais. Esse prazo não é usado como regra genérica de retenção de todos os dados das obras. [Resolução ANPD 15/2024, art. 10](https://bibliotecadigital.mj.gov.br/bitstream/1/12879/2/RES_ANPD_2024_15.html).

### 6.4 Metas operacionais

As seguintes metas são decisões de produto e devem ser sustentadas pelo custo, pela arquitetura e pela equipe:

| Indicador | Meta e forma de comprovação |
|---|---|
| Disponibilidade | Pelo menos 99,5% em cada mês; medir jornadas essenciais e incluir manutenção no tempo de indisponibilidade |
| Perda máxima de dados em recuperação | Até 1 hora entre o último estado recuperável e o evento, incluindo registros e anexos relacionados |
| Tempo de recuperação | Até 4 horas entre declaração do incidente e validação do serviço restaurado |
| Retenção operacional de backups | 35 dias, com política de expiração e preservação legal separada |
| Verificação de restauração | Antes da abertura, mensalmente e após mudança relevante de armazenamento ou recuperação |
| Monitoramento | Disponibilidade, erros, latência, filas, cobrança, capacidade, falhas de backup e acesso anormal |
| Atendimento a incidentes | Responsável nominal e substituto; escala, cobertura, forma de acionamento e compromisso publicados antes da venda |

Restauração precisa ocorrer em ambiente isolado e conferir banco, arquivos, relações, auditoria, chaves e permissões. O procedimento reaplica exclusões e limitações registradas depois do ponto restaurado quando necessário. Backup criptografado sem chave recuperável não atende ao critério. A equipe deve demonstrar o acionamento real dos alertas e o acesso aos meios de recuperação.

O compromisso comercial de atendimento não pode superar a cobertura efetivamente financiada. Horários e prazos são definidos nas issues 67 e 69 e publicados nos termos. A disponibilidade operacional continua sendo medida integralmente, sem exclusão arbitrária de manutenção.

## 7. Oferta, assinatura e atendimento

A issue 68 define planos, preço, limites, teste, renovação, cancelamento, inadimplência e exportação, com política comercial mínima versionada. O mecanismo é testado em sandbox e, quando autorizado, por transação técnica controlada. A contratação de cliente sob termos finais e o respectivo recebimento ocorrem depois da issue 69, com comprovação nos pilotos e na liberação. O preço precisa considerar infraestrutura, armazenamento, tráfego, processamento, pagamento, assinatura eletrônica quando usada, suporte e implantação. Registrar a margem e o cenário de maior consumo permitido por cada plano. Não oferecer uso ilimitado sem capacidade e custo demonstrados.

A cobrança usa um provedor habilitado, com segredos protegidos, confirmação autenticada, repetição segura, reprocessamento e conciliação. O estado da assinatura deriva de eventos verificados e do contrato do serviço, não da simples abertura de uma página de sucesso. Testar cobrança aprovada, pendente, recusada, vencida, duplicada, estornada e contestada conforme o meio adotado.

Um ambiente de testes não comprova recebimento real. Antes da liberação, uma contratação real autorizada deve comprovar a cobrança e sua conciliação, com comprovante e dados protegidos. Não se cria receita fictícia para satisfazer o critério.

A issue 69 entrega:

- Termos de uso e contratação, política de privacidade e responsabilidades de tratamento, revisados para a oferta efetiva.
- Identificação do fornecedor, preços, limites, cobrança, cancelamento, retenção, exportação, suporte e condições de suspensão.
- Aceite com versão e data, conservando a evidência sem expor dados desnecessários.
- Guias por papel, obra demonstrativa isolada, importação inicial e convite de colaboradores.
- Canal de suporte operante, horários, responsáveis e procedimento de escalonamento.
- Tratamento de solicitações, incidentes, direitos de titulares e encerramento de conta.

Inadimplência não pode produzir exclusão surpresa dos dados. As condições de aviso, restrição, leitura, exportação, retenção e eliminação devem estar claras e ser verificadas. A recuperação de acesso não envia a senha existente nem depende de conhecer um dado público da empresa.

## 8. Homologação e pilotos

### Issue 70: ciclo integrado controlado

Executar os requisitos cuja issue primária é de 01 a 69, os cenários compostos desses módulos e os critérios próprios da issue 70. BR36 e os demais critérios de pilotos da 71, assim como BR38 e os critérios de liberação da 72, permanecem identificados como posteriores, sem aprovação antecipada. A matriz integral somente é concluída na issue 72.

Executar os fluxos com dados de teste calculados independentemente, papéis distintos e pelo menos duas organizações. Validar cumulativos, totais, documentos, efeitos de mudanças e conciliação. Casos negativos devem incluir saldo insuficiente, acesso indevido, documento desatualizado, versão concorrente, mesma chave com entrada diferente, evento duplicado, interrupção de rede e falha de serviço externo.

Todas as diferenças precisam ser explicadas e corrigidas. O relatório identifica versão do aplicativo, migração, massa de dados, configuração, resultados e defeitos. A evidência pertence à versão candidata; uma alteração posterior relevante repete os testes afetados.

### Issue 71: duas empresas independentes

Participam pelo menos duas organizações distintas, com responsáveis identificados e dados cujo uso foi autorizado. Uma valida empreitada unitária e outra empreitada global. Ambas completam o ciclo central de orçamento, contratação, ordem, execução, medição, faturamento, pagamento ou recebimento, aceite e exportação.

O conjunto também valida gestão de projeto, manutenção, portal externo e uso móvel. Dados históricos podem acelerar o ensaio, desde que a origem seja identificada e o resultado não seja apresentado como uma transação ocorrida naquele momento. Movimentos reais e simulados são sempre distinguíveis.

O piloto deve verificar a assinatura e a cobrança do próprio serviço, a implantação, a recuperação de acesso, o atendimento e a saída de dados. A assinatura documental da obra pode ser externa, desde que o arquivo e a evidência estejam preservados e conferidos segundo o fluxo definido.

Aplicar as seis tarefas e os critérios de [Experiência e desempenho](experiencia.md): pelo menos cinco participantes, 30 execuções, 90% de conclusão sem assistência, mínimo de quatro sucessos por tarefa e nenhum erro financeiro irreversível. Registrar objeções e falhas. Aceite do cliente não dispensa correção de falha alta, crítica ou de cálculo.

## 9. Publicação e decisão de liberação

A issue 72 deve executar, nesta ordem:

1. Confirmar a conclusão verificável das issues 01 a 71 e a matriz deste documento.
2. Identificar a versão candidata, alterações de banco, dependências, configuração e plano de recuperação.
3. Confirmar domínio, certificado, serviços externos, segredos, faturamento, suporte e responsáveis.
4. Fazer o backup exigido e comprovar que a restauração aplicável continua executável.
5. Publicar a versão por procedimento reproduzível, sem compartilhar credenciais ou dados privados nos registros.
6. Executar verificação em produção com duas organizações de teste controladas: autenticação, autorização, obra, documento, medição, financeiro, exportação e cobrança segundo autorização própria.
7. Conferir alertas, filas, erros, isolamento e conciliação após a publicação.
8. Registrar a decisão de liberação comercial, a versão exata, os responsáveis e os compromissos assumidos.

Se qualquer impedimento da matriz persistir, a issue permanece aberta e a versão não recebe a condição de pronta para venda. A documentação deve registrar precisamente o impedimento, sem marcar o requisito como atendido por intenção ou por ausência de teste.

## 10. Conjunto de evidências da versão 1.0

O repositório mantém um índice de evidências da liberação, com referências seguras aos arquivos e relatórios. Dados pessoais, comprovantes financeiros completos e segredos não devem ser publicados no repositório.

O índice deve conter:

- Commit e versão publicados, identificadores das migrações e versões das dependências.
- Matriz de regras, critérios e testes aprovados.
- Relatórios financeiro, de acessibilidade, experiência, desempenho e segurança.
- Resultado de restauração com horário, perda medida e tempo de recuperação.
- Termos e políticas efetivamente aceitos, com identificação de versão.
- Planos, preços, custo por cenário e decisão comercial.
- Evidências de cobrança, conciliação, atendimento e pilotos, com acesso restrito quando necessário.
- Registro de publicação, verificação em produção e decisão final.

Esse conjunto demonstra o que foi construído e verificado. Não corresponde a certificação automática, aprovação de conselho profissional ou garantia de adequação a qualquer contrato, município ou situação tributária fora do escopo declarado.
