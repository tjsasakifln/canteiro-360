# Plano de implementação

**Ordem obrigatória: issues 01 a 72.** Cada etapa depende da aceitação e integração da imediatamente anterior. Assim, a execução numérica respeita todas as dependências anteriores. Pesquisa e revisão podem ser paralelas; não antecipar a integração de um módulo.

A numeração no título é estável. O número nativo foi reservado pela criação sequencial das 72 issues antes das primeiras pull requests. O [manifesto](backlog.json) registra os vínculos publicados. Os [arquivos individuais](issues) preservam o conteúdo planejado, com entregas, critérios, regras e evidências.

## Fases

| Fase | Issues | Resultado acumulado |
|---|---|---|
| F1 | 01 a 15 | Fundação, experiência e cadastros |
| F2 | 16 a 22 | Custos, orçamento e contratação privada |
| F3 | 23 a 30 | Contratos e execução |
| F4 | 31 a 43 | Medições e alterações contratuais |
| F5 | 44 a 50 | Financeiro e encerramento |
| F6 | 51 a 62 | Manutenção, projetos e integração |
| F7 | 63 a 67 | Qualidade e operação |
| F8 | 68 a 72 | Venda, pilotos e liberação comercial |

## Fila numerada

| Ordem | Issue | Resultado desta etapa | Dependência direta |
|---|---|---|---|
| 01 | [Fundação Rust, arquitetura e integração contínua](https://github.com/tjsasakifln/canteiro-360/issues/1) · [especificação](issues/001.md) | Criar a base compilável do produto com Rust no servidor e na interface. | Nenhuma |
| 02 | [Persistência, migrações e transações de domínio](https://github.com/tjsasakifln/canteiro-360/issues/2) · [especificação](issues/002.md) | Estabelecer persistência transacional e contratos de erro que suportem os módulos seguintes. | #1 |
| 03 | [Sistema visual, navegação e componentes acessíveis](https://github.com/tjsasakifln/canteiro-360/issues/3) · [especificação](issues/003.md) | Definir uma interface própria, consistente e orientada à rotina da obra. | #2 |
| 04 | [Formulários e tabelas de alto desempenho](https://github.com/tjsasakifln/canteiro-360/issues/4) · [especificação](issues/004.md) | Entregar formulários e tabelas reutilizáveis que evitem erros e mantenham fluidez. | #3 |
| 05 | [Precisão decimal, unidades e calendários](https://github.com/tjsasakifln/canteiro-360/issues/5) · [especificação](issues/005.md) | Fixar e verificar o motor decimal e as políticas de calendário antes de usá-los em valores reais. | #4 |
| 06 | [Autenticação, sessões e recuperação de acesso](https://github.com/tjsasakifln/canteiro-360/issues/6) · [especificação](issues/006.md) | Permitir acesso seguro e recuperação de conta sem fragilidade herdada do sistema antigo. | #5 |
| 07 | [Organizações, convites e isolamento de dados](https://github.com/tjsasakifln/canteiro-360/issues/7) · [especificação](issues/007.md) | Separar completamente os dados das organizações e permitir convites controlados. | #6 |
| 08 | [Papéis, vínculos, alçadas e substituições](https://github.com/tjsasakifln/canteiro-360/issues/8) · [especificação](issues/008.md) | Aplicar permissões por tarefa, obra e contrato com alçadas rastreáveis. | #7 |
| 09 | [Auditoria, concorrência e execução idempotente](https://github.com/tjsasakifln/canteiro-360/issues/9) · [especificação](issues/009.md) | Preservar história e impedir efeitos duplicados ou perdidos por concorrência. | #8 |
| 10 | [Documentos, anexos e versões imutáveis](https://github.com/tjsasakifln/canteiro-360/issues/10) · [especificação](issues/010.md) | Guardar documentos confiáveis e vinculados ao fato correto. | #9 |
| 11 | [Clientes, fornecedores e profissionais](https://github.com/tjsasakifln/canteiro-360/issues/11) · [especificação](issues/011.md) | Cadastrar as partes e os profissionais usados em todos os fluxos. | #10 |
| 12 | [Carteira de empreendimentos, obras e ativos](https://github.com/tjsasakifln/canteiro-360/issues/12) · [especificação](issues/012.md) | Organizar carteira, obras, contratos futuros e ativos sem ambiguidade. | #11 |
| 13 | [Escopo de projetos, entregas e responsáveis](https://github.com/tjsasakifln/canteiro-360/issues/13) · [especificação](issues/013.md) | Permitir controlar um projeto antes e durante a execução da obra. | #12 |
| 14 | [ART, RRT, licenças e obrigações técnicas](https://github.com/tjsasakifln/canteiro-360/issues/14) · [especificação](issues/014.md) | Controlar responsabilidades e documentos técnicos segundo sua natureza. | #13 |
| 15 | [Notificações, pendências e prazos](https://github.com/tjsasakifln/canteiro-360/issues/15) · [especificação](issues/015.md) | Tornar pendências acionáveis sem excesso de avisos. | #14 |
| 16 | [Catálogos de custos e preços de referência](https://github.com/tjsasakifln/canteiro-360/issues/16) · [especificação](issues/016.md) | Criar uma base de custos com origem e vigência confiáveis. | #15 |
| 17 | [Composições, insumos, BDI e incidências](https://github.com/tjsasakifln/canteiro-360/issues/17) · [especificação](issues/017.md) | Calcular serviços e composições com parâmetros rastreáveis. | #16 |
| 18 | [Orçamentos, variações e estrutura de serviços](https://github.com/tjsasakifln/canteiro-360/issues/18) · [especificação](issues/018.md) | Montar orçamentos completos e seu planejamento inicial. | #17 |
| 19 | [Importação e exportação de planilhas orçamentárias](https://github.com/tjsasakifln/canteiro-360/issues/19) · [especificação](issues/019.md) | Trazer e devolver planilhas sem corrupção de valores ou aplicação parcial. | #18 |
| 20 | [Revisões, cópia, duplicação e atualização de preços](https://github.com/tjsasakifln/canteiro-360/issues/20) · [especificação](issues/020.md) | Revisar orçamento preservando a diferença entre copiar, duplicar e atualizar. | #19 |
| 21 | [Conferência e aprovação de orçamentos](https://github.com/tjsasakifln/canteiro-360/issues/21) · [especificação](issues/021.md) | Aprovar uma versão de orçamento somente após conferência integral. | #20 |
| 22 | [Cotações privadas e seleção de fornecedores](https://github.com/tjsasakifln/canteiro-360/issues/22) · [especificação](issues/022.md) | Conduzir seleção privada de fornecedor com base de comparação comum. | #21 |
| 23 | [Contratos, partes, regime e vigência](https://github.com/tjsasakifln/canteiro-360/issues/23) · [especificação](issues/023.md) | Cadastrar o instrumento privado e seus limites de operação. | #22 |
| 24 | [Serviços contratados e orçamento de referência](https://github.com/tjsasakifln/canteiro-360/issues/24) · [especificação](issues/024.md) | Congelar o escopo econômico contratado com origem verificável. | #23 |
| 25 | [Empreitada global por etapas](https://github.com/tjsasakifln/canteiro-360/issues/25) · [especificação](issues/025.md) | Modelar contratos globais por etapas conforme a estrutura funcional documentada. | #24 |
| 26 | [Cronograma inicial e versões de planejamento](https://github.com/tjsasakifln/canteiro-360/issues/26) · [especificação](issues/026.md) | Fixar o cronograma contratado e conservar as reprogramações. | #25 |
| 27 | [Autorização de início e ordem de serviço](https://github.com/tjsasakifln/canteiro-360/issues/27) · [especificação](issues/027.md) | Autorizar o início apenas com escopo e responsabilidades definidos. | #26 |
| 28 | [Diário de obra, fotos e registros de campo](https://github.com/tjsasakifln/canteiro-360/issues/28) · [especificação](issues/028.md) | Registrar execução e evidências diariamente com revisão confiável. | #27 |
| 29 | [Qualidade, advertências e ações corretivas](https://github.com/tjsasakifln/canteiro-360/issues/29) · [especificação](issues/029.md) | Controlar qualidade e comunicações formais sem perder o histórico. | #28 |
| 30 | [Paralisação, retomada e cálculo de prazo](https://github.com/tjsasakifln/canteiro-360/issues/30) · [especificação](issues/030.md) | Representar paralisação e retomada com prazo corretamente calculado. | #29 |
| 31 | [Medições, competências e sequência cronológica](https://github.com/tjsasakifln/canteiro-360/issues/31) · [especificação](issues/031.md) | Criar a capa e a cronologia das medições sem confundir zero com cobrança. | #30 |
| 32 | [Medição unitária por acumulados](https://github.com/tjsasakifln/canteiro-360/issues/32) · [especificação](issues/032.md) | Calcular a execução e o valor do período a partir dos acumulados. | #31 |
| 33 | [Medição global e serviços percentuais](https://github.com/tjsasakifln/canteiro-360/issues/33) · [especificação](issues/033.md) | Calcular progresso global e incidências sem duplicar execução. | #32 |
| 34 | [Comissões, evidências e conferência técnica](https://github.com/tjsasakifln/canteiro-360/issues/34) · [especificação](issues/034.md) | Conferir medição com equipe válida e evidência suficiente. | #33 |
| 35 | [Reajustes por obra e por serviço](https://github.com/tjsasakifln/canteiro-360/issues/35) · [especificação](issues/035.md) | Calcular reajuste respeitando fonte, competência e data do preço. | #34 |
| 36 | [Cálculo definitivo e aprovação de medições](https://github.com/tjsasakifln/canteiro-360/issues/36) · [especificação](issues/036.md) | Consolidar e aprovar valores somente após todas as validações. | #35 |
| 37 | [Caderno de medição e memória de cálculo](https://github.com/tjsasakifln/canteiro-360/issues/37) · [especificação](issues/037.md) | Produzir documentação de medição completa e reconciliável. | #36 |
| 38 | [Pré-aditivos e propostas de alteração](https://github.com/tjsasakifln/canteiro-360/issues/38) · [especificação](issues/038.md) | Preparar alterações de escopo, valor e prazo com justificativa completa. | #37 |
| 39 | [Alçadas e liberação de pré-aditivos](https://github.com/tjsasakifln/canteiro-360/issues/39) · [especificação](issues/039.md) | Liberar proposta de alteração pela autoridade correta. | #38 |
| 40 | [Formalização e aplicação de aditivos](https://github.com/tjsasakifln/canteiro-360/issues/40) · [especificação](issues/040.md) | Aplicar o instrumento de alteração uma única vez e na data permitida. | #39 |
| 41 | [Supressão de serviços já medidos](https://github.com/tjsasakifln/canteiro-360/issues/41) · [especificação](issues/041.md) | Corrigir quantidade já medida por decisão integral e rastreável. | #40 |
| 42 | [Reprogramação com execução e alterações](https://github.com/tjsasakifln/canteiro-360/issues/42) · [especificação](issues/042.md) | Reprogramar a obra com execução reconhecida e alterações válidas. | #41 |
| 43 | [Reajustes complementares e correções controladas](https://github.com/tjsasakifln/canteiro-360/issues/43) · [especificação](issues/043.md) | Concluir reajustes provisórios e tratar correções sem duplicar valores. | #42 |
| 44 | [Faturas e títulos a pagar e a receber](https://github.com/tjsasakifln/canteiro-360/issues/44) · [especificação](issues/044.md) | Controlar faturamento e obrigações sem confundi-los com execução ou caixa. | #43 |
| 45 | [Adiantamentos, retenções e garantias financeiras](https://github.com/tjsasakifln/canteiro-360/issues/45) · [especificação](issues/045.md) | Controlar retenções e adiantamentos com saldos próprios. | #44 |
| 46 | [Pagamentos, recebimentos e conciliação](https://github.com/tjsasakifln/canteiro-360/issues/46) · [especificação](issues/046.md) | Registrar e conciliar dinheiro efetivamente pago ou recebido. | #45 |
| 47 | [Previsão de caixa e custo por obra](https://github.com/tjsasakifln/canteiro-360/issues/47) · [especificação](issues/047.md) | Dar visão econômica da obra sem somar etapas da mesma despesa. | #46 |
| 48 | [Medição final e recebimento provisório](https://github.com/tjsasakifln/canteiro-360/issues/48) · [especificação](issues/048.md) | Encerrar execução e liberar a medição final com termo apropriado. | #47 |
| 49 | [Recebimento definitivo e garantias da obra](https://github.com/tjsasakifln/canteiro-360/issues/49) · [especificação](issues/049.md) | Concluir recebimento definitivo e organizar garantias e documentos da entrega. | #48 |
| 50 | [Rescisão, cancelamento e encerramento](https://github.com/tjsasakifln/canteiro-360/issues/50) · [especificação](issues/050.md) | Registrar encerramento antecipado sem apagar direitos e obrigações. | #49 |
| 51 | [Acordos de manutenção, lotes e catálogos](https://github.com/tjsasakifln/canteiro-360/issues/51) · [especificação](issues/051.md) | Modelar acordos privados de manutenção com catálogo e limite financeiro. | #50 |
| 52 | [Pedidos de manutenção e aprovação da demanda](https://github.com/tjsasakifln/canteiro-360/issues/52) · [especificação](issues/052.md) | Receber e analisar solicitações de manutenção do ativo correto. | #51 |
| 53 | [Ordens de manutenção e reserva de saldo](https://github.com/tjsasakifln/canteiro-360/issues/53) · [especificação](issues/053.md) | Autorizar ordem de manutenção com preço, quantidade e saldo protegidos. | #52 |
| 54 | [Fiscalização, aceite e medição da manutenção](https://github.com/tjsasakifln/canteiro-360/issues/54) · [especificação](issues/054.md) | Fiscalizar e medir manutenção sem duplicar a execução da ordem. | #53 |
| 55 | [Revisões de projeto, solicitações e distribuição](https://github.com/tjsasakifln/canteiro-360/issues/55) · [especificação](issues/055.md) | Controlar revisão e distribuição de projetos usados na obra. | #54 |
| 56 | [Dependências, cronograma e recursos de projetos](https://github.com/tjsasakifln/canteiro-360/issues/56) · [especificação](issues/056.md) | Planejar projetos com dependências e previsão verificável. | #55 |
| 57 | [Riscos, problemas e mudanças de projeto](https://github.com/tjsasakifln/canteiro-360/issues/57) · [especificação](issues/057.md) | Tratar riscos, problemas e mudanças de projeto com efeito rastreável. | #56 |
| 58 | [Portal do cliente e de fornecedores](https://github.com/tjsasakifln/canteiro-360/issues/58) · [especificação](issues/058.md) | Oferecer colaboração privada ao cliente e ao fornecedor. | #57 |
| 59 | [Busca, favoritos, mapas e relatórios gerenciais](https://github.com/tjsasakifln/canteiro-360/issues/59) · [especificação](issues/059.md) | Consolidar busca e relatórios que correspondam aos fatos da obra. | #58 |
| 60 | [Migração assistida de obras e contratos existentes](https://github.com/tjsasakifln/canteiro-360/issues/60) · [especificação](issues/060.md) | Implantar clientes com histórico existente sem recalculá-lo indevidamente. | #59 |
| 61 | [API e integrações externas controladas](https://github.com/tjsasakifln/canteiro-360/issues/61) · [especificação](issues/061.md) | Integrar serviços externos com autorização e contratos de dados claros. | #60 |
| 62 | [Uso em campo e sincronização de rascunhos](https://github.com/tjsasakifln/canteiro-360/issues/62) · [especificação](issues/062.md) | Permitir trabalho de campo com conexão instável e sincronização segura. | #61 |
| 63 | [Validação integral de interface e acessibilidade](https://github.com/tjsasakifln/canteiro-360/issues/63) · [especificação](issues/063.md) | Demonstrar que o produto é intuitivo, acessível e consistente em suas jornadas completas. | #62 |
| 64 | [Desempenho do servidor e do front-end Rust](https://github.com/tjsasakifln/canteiro-360/issues/64) · [especificação](issues/064.md) | Cumprir metas de desempenho com a aplicação Rust e volume profissional. | #63 |
| 65 | [Revisão de segurança e testes adversariais](https://github.com/tjsasakifln/canteiro-360/issues/65) · [especificação](issues/065.md) | Procurar e corrigir falhas de segurança na versão integrada. | #64 |
| 66 | [Privacidade, retenção e direitos dos titulares](https://github.com/tjsasakifln/canteiro-360/issues/66) · [especificação](issues/066.md) | Operar os dados pessoais com finalidade, direitos e retenção definidos. | #65 |
| 67 | [Operação, monitoramento e restauração de backup](https://github.com/tjsasakifln/canteiro-360/issues/67) · [especificação](issues/067.md) | Manter o serviço recuperável e observável em produção. | #66 |
| 68 | [Planos, assinatura e cobrança do serviço](https://github.com/tjsasakifln/canteiro-360/issues/68) · [especificação](issues/068.md) | Cobrar pelo serviço com planos claros e estados conciliados. | #67 |
| 69 | [Contratação, atendimento e implantação do cliente](https://github.com/tjsasakifln/canteiro-360/issues/69) · [especificação](issues/069.md) | Entregar contratação e implantação utilizáveis por um cliente real. | #68 |
| 70 | [Homologação integrada e reconciliação financeira](https://github.com/tjsasakifln/canteiro-360/issues/70) · [especificação](issues/070.md) | Validar os módulos construídos até a etapa 70 e seus cenários integrados, antes dos pilotos e da liberação. | #69 |
| 71 | [Pilotos profissionais e correção de falhas](https://github.com/tjsasakifln/canteiro-360/issues/71) · [especificação](issues/071.md) | Comprovar utilidade e confiabilidade com duas organizações independentes. | #70 |
| 72 | [Liberação comercial da versão 1.0](https://github.com/tjsasakifln/canteiro-360/issues/72) · [especificação](issues/072.md) | Liberar a versão 1.0 apenas com comprovação profissional e comercial da versão publicada. | #71 |

## Aceite de cada etapa

Entregar domínio Rust, interface Leptos, dados/migração, autorização, auditoria e testes aplicáveis. A funcionalidade precisa ser utilizável na extensão da etapa. Não fechar por protótipo, endpoint isolado ou lista marcada sem evidência. O revisor identifica a versão examinada e registra falhas/correções.

## Testes de regras antes dos módulos consumidores

Fundação de autorização, precisão e auditoria pode ser testada com agregados e dados fictícios antes de existir cada consumidor. Isso verifica o contrato técnico; não é prova antecipada do módulo futuro. Toda issue que introduz novo consumidor deve aplicar e testar as proteções com sua integração real. A issue 70 reúne os cenários completos, incluindo anulação após faturamento/baixa e isolamento de todos os canais.

Orçamento já contém seu cronograma na 18, antes de aprovação na 21. A 26 trata o cronograma contratual. A 20 trata índices da revisão orçamentária; a 35 trata reajuste de medição. Esse recorte evita dependências futuras escondidas.

## Condição da última issue

A 72 exige as 71 anteriores aceitas, versão publicada, pilotos independentes, cobrança e suporte operantes, conciliação monetária, acessibilidade, desempenho, segurança e restauração. A tabela de [prontidão comercial](prontidao-comercial.md) é vinculante. Se um requisito obrigatório falhar, corrigir e repetir os casos afetados antes do lançamento.

## Alterações do plano

Nova necessidade obrigatória encontrada durante implementação exige atualização rastreável do PRD, da matriz e das issues atingidas. Manter o identificador original e não renumerar silenciosamente issues já criadas. Defeitos novos são ligados à etapa de origem e impedem seu aceite ou a liberação quando forem impeditivos.
