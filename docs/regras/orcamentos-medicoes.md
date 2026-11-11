# Regras de orçamento, medição e manutenção por catálogo

**Produto:** Canteiro 360. **Referência da especificação:** 10/11/2026. **Situação:** requisitos para implementação; não representa software já implementado ou homologado.

Este documento preserva os comportamentos verificáveis dos manuais públicos do SICOP e define expressamente sua aplicação à contratação privada. O registro estruturado correspondente está em [orcamentos-medicoes.json](orcamentos-medicoes.json). Cada regra tem uma issue primária e um critério verificável de aceite. A issue primária entrega o comportamento; as issues seguintes podem ampliá-lo sem alterar o contrato já estabelecido.

## Como interpretar as regras

- **Documentada:** o comportamento essencial está descrito na fonte e é preservado.
- **Adaptada:** o comportamento de origem está identificado, mas sua implementação privada muda nomenclatura, autorização, integração ou tratamento operacional explicitamente indicado.
- **Decisão:** os manuais são insuficientes, apresentam divergência ou não especificam o detalhe. A regra do Canteiro 360 é uma decisão própria, sem alegação de equivalência não demonstrada.

As fontes mais recentes prevalecem apenas nos pontos que alteram: SIC-042 acrescenta a liberação após recebimento provisório; SIC-027 esclarece edição/alocação e índices; SIC-021 e SIC-020 tratam dos catálogos de manutenção; SIC-016, de novembro de 2025, acrescenta medição por etapas de preço global. Uma data posterior não torna automaticamente inválido todo o documento anterior.

Páginas de SIC-003 e SIC-016 são identificadas como impressas e físicas para evitar erro de localização. As demais referências usam páginas físicas do PDF. Os documentos de referência não substituem avaliação de obrigações profissionais, contratuais ou fiscais aplicáveis à obra.

## Cálculo, datas e preservação do histórico

A política normativa de precisão, escala, limite de entrada, descontos, adiantamentos, retenções e reajustes é definida em [calculos.md](calculos.md). O motor é implementado em Rust e compartilhado com a prévia do navegador, com a validação definitiva no servidor. A apresentação em reais não autoriza o uso de ponto flutuante binário nos valores financeiros.

Para a mesma referência de preço e escopo, a medição do período usa diferenças de valores acumulados truncados a centavos. Exemplo: preço unitário de R$ 0,335 e acumulados de 1, 2 e 3 unidades resultam em R$ 0,33, R$ 0,34 e R$ 0,33 nos períodos. O total é R$ 1,00. A política é uma decisão própria porque a fonte não especifica completamente a ordem da truncagem.

Mudanças de preço e escopo geram segmentos contratuais com vigência identificada. Valores anteriores reconhecidos ficam preservados. A medição seguinte não recalcula quantidades antigas pelo novo preço; uma correção ou reversão referencia o valor originalmente reconhecido. Saldos importados seguem a mesma preservação, com conciliação e decisão registradas.

A divergência entre medição mensal zerada e exigência posterior de total positivo é resolvida por duas operações distintas. O **registro de competência sem avanço** conserva a cronologia sem gerar cobrança. A **medição financeira ordinária** exige base devida positiva. Valores negativos seguem correção ou estorno controlado; não se criam serviços fictícios para torná-los positivos.

O cronograma orçamentário mantém os períodos de 30 dias descritos no manual. O cronograma mensal civil de execução é uma representação privada distinta. Conversão, período residual e data de referência devem ser mostrados, sem tratar mês civil e bloco de 30 dias como iguais. O residual de percentuais e valores usa maiores restos, com desempate por posição estável crescente e depois identificador; o mesmo conjunto de dados produz a mesma distribuição em tela, arquivo e servidor.

## Experiência de uso obrigatória

O usuário vê empresa, obra, contrato e revisão ativos. A tabela de medição diferencia contratado, anterior acumulado, atual acumulado, diferença do período, valor devido e saldo. Quantidade e percentual representam a mesma execução. A prévia deve mostrar o efeito monetário antes de confirmar, e o servidor deve recalcular e validar a mesma revisão.

Mensagens de erro indicam a linha atingida, o valor permitido e como corrigir. Falta de permissão, conflito entre edições e saldo insuficiente nunca produzem gravação parcial silenciosa. O uso de teclado, leitor de tela e telas menores deve manter a mesma clareza da operação.


## Orçamentos e custos

### ORC01

**Origem:** Adaptada. **Implementação:** [issue 08](../issues/008.md). **Fonte:** [SIC-001; p.4](https://www.sicop.sc.gov.br/download/sicop/Guia_Orcamento_Consulta_e_Cadastro.pdf).

**Regra de referência:** Há perfis distintos para cadastrar orçamentos, consultar relatórios e atuar como empresa orçamentista.

**Comportamento do Canteiro 360:** Separar edição, revisão, aprovação e leitura por organização e orçamento; o perfil externo não recebe permissões gerais de funcionário.

**Aceite:** Um usuário de leitura consulta o orçamento autorizado, mas qualquer tentativa de criar, editar, importar ou aprovar é recusada também pela API.

### ORC02

**Origem:** Documentada. **Implementação:** [issue 18](../issues/018.md). **Fonte:** [SIC-001; pp.5-8](https://www.sicop.sc.gov.br/download/sicop/Guia_Orcamento_Consulta_e_Cadastro.pdf).

**Regra de referência:** Natureza, orçamento e versão são selecionados como contexto de trabalho e mantidos entre telas.

**Comportamento do Canteiro 360:** Exibir sempre empresa, obra, orçamento e versão ativos; persistir a seleção por usuário, sem substituir autorização por estado da interface.

**Aceite:** Ao alternar entre dois orçamentos, os cabeçalhos, itens, cálculos e destino de gravação acompanham a seleção; uma aba antiga não grava no orçamento recém-selecionado.

### ORC03

**Origem:** Adaptada. **Implementação:** [issue 07](../issues/007.md). **Fonte:** [SIC-001; p.9](https://www.sicop.sc.gov.br/download/sicop/Guia_Orcamento_Consulta_e_Cadastro.pdf).

**Regra de referência:** A consulta mostra somente os orçamentos de órgãos vinculados ao usuário.

**Comportamento do Canteiro 360:** Restringir busca, listagem, exportação e acesso direto por organização e concessão sobre o orçamento.

**Aceite:** Um usuário da empresa A não consegue obter orçamento, existência, contagem ou arquivo da empresa B alterando o identificador na requisição.

### ORC04

**Origem:** Adaptada. **Implementação:** [issue 12](../issues/012.md). **Fonte:** [SIC-001; pp.11-14](https://www.sicop.sc.gov.br/download/sicop/Guia_Orcamento_Consulta_e_Cadastro.pdf).

**Regra de referência:** Orçamentos de edificações e rodoviários têm campos específicos; o segundo inclui municípios e rodovias.

**Comportamento do Canteiro 360:** Oferecer obras prediais e lineares privadas; localização, trechos, subtrechos e municípios são condicionais à natureza e não exigem um órgão público.

**Aceite:** Uma edificação pode ser cadastrada sem rodovia; uma obra linear registra dois municípios e trechos sem duplicar o orçamento.

### ORC05

**Origem:** Adaptada. **Implementação:** [issue 08](../issues/008.md). **Fonte:** [SIC-001; pp.15-16](https://www.sicop.sc.gov.br/download/sicop/Guia_Orcamento_Consulta_e_Cadastro.pdf).

**Regra de referência:** Gestores de orçamento precisam do perfil apropriado e podem ser associados ou inativados.

**Comportamento do Canteiro 360:** Permitir vincular e inativar gestores autorizados, preservar autoria histórica e revogar imediatamente o poder de editar.

**Aceite:** Após inativação, o antigo gestor mantém seu nome nos registros assinados, mas uma sessão aberta não consegue salvar novas alterações.

### ORC06

**Origem:** Adaptada. **Implementação:** [issue 18](../issues/018.md). **Fonte:** [SIC-012; pp.2-3](https://www.sicop.sc.gov.br/download/sicop/Modulo_de_Orcamento_Orientacao_de_preenchimento.pdf).

**Regra de referência:** O orçamento precisa de ao menos uma variação; variações demandam obras no contrato, enquanto etapas da mesma obra devem ser grupos de serviços.

**Comportamento do Canteiro 360:** Separar variação de escopo, vinculada a uma obra ou intervenção, de revisão temporal da mesma variação; organizar disciplinas e etapas dentro da estrutura de serviços.

**Aceite:** Revisar o orçamento de uma obra não cria uma segunda obra; reforma e ampliação tratadas como intervenções distintas podem ter variações próprias.

### ORC07

**Origem:** Documentada. **Implementação:** [issue 20](../issues/020.md). **Fonte:** [SIC-001; pp.17-22 e 52-53](https://www.sicop.sc.gov.br/download/sicop/Guia_Orcamento_Consulta_e_Cadastro.pdf).

**Regra de referência:** Copiar uma versão atualiza seus preços pela tabela de origem vigente; duplicar preserva os valores.

**Comportamento do Canteiro 360:** Oferecer duas ações com nomes, prévia e resultado distintos. Registrar tabela, competência, revisão de origem e diferenças monetárias antes da confirmação.

**Aceite:** Com preço original 10,00 e nova tabela 11,00, duplicar mantém 10,00 e copiar com atualização usa 11,00; nenhuma ação muda a versão de origem.

### ORC08

**Origem:** Documentada. **Implementação:** [issue 21](../issues/021.md). **Fonte:** [SIC-001; pp.18-19 e 53-55](https://www.sicop.sc.gov.br/download/sicop/Guia_Orcamento_Consulta_e_Cadastro.pdf).

**Regra de referência:** Conferência admite edição; Finalizado corresponde à conferência concluída; Aprovado sucede conferência e finalização.

**Comportamento do Canteiro 360:** Manter transições explícitas; qualquer alteração em versão ainda revisável invalida a conferência anterior e exige novo cálculo antes de aprovar.

**Aceite:** Uma versão editada após a conferência não pode ser aprovada com o resultado antigo; a API informa os itens que exigem nova conferência.

### ORC09

**Origem:** Documentada. **Implementação:** [issue 21](../issues/021.md). **Fonte:** [SIC-001; pp.54-55](https://www.sicop.sc.gov.br/download/sicop/Guia_Orcamento_Consulta_e_Cadastro.pdf).

**Regra de referência:** A aprovação exige conferência e cálculo; somente orçamento não vinculado a contrato pode ser desaprovado.

**Comportamento do Canteiro 360:** Bloquear desaprovação ou edição da revisão contratada; alterações posteriores seguem revisão e aditivo próprios.

**Aceite:** Uma revisão vinculada a contrato recusa desaprovação e gravação direta. Uma revisão aprovada ainda não vinculada pode voltar à conferência mediante justificativa e permissão.

### ORC10

**Origem:** Documentada. **Implementação:** [issue 10](../issues/010.md). **Fonte:** [SIC-001; pp.22-23](https://www.sicop.sc.gov.br/download/sicop/Guia_Orcamento_Consulta_e_Cadastro.pdf).

**Regra de referência:** Documentos com descrição e arquivo ficam vinculados ao orçamento.

**Comportamento do Canteiro 360:** Vincular cada anexo a orçamento e revisão identificados, com autoria, data e versão imutável; permitir substituição por nova versão.

**Aceite:** Trocar um memorial gera nova versão e preserva o arquivo anteriormente associado à aprovação; seu acesso obedece à permissão do orçamento.

### ORC11

**Origem:** Documentada. **Implementação:** [issue 18](../issues/018.md). **Fonte:** [SIC-027; pp.1-3](https://www.sicop.sc.gov.br/download/sicop/PACOTE_DE_ALTERACAO_SICOP_04_2020_ALTERACOES_NA_PLANILHA_DO_ORCAMENTO.pdf).

**Regra de referência:** A planilha abre somente para consulta; a edição exige ação explícita e alocação ao usuário.

**Comportamento do Canteiro 360:** Exigir reserva de edição por versão, com expiração segura e liberação auditada; a perda de reserva impede gravação silenciosa.

**Aceite:** Dois usuários tentam editar a mesma versão: apenas um recebe a reserva; o segundo vê responsável e opção de atualização, sem sobrescrever dados.

### ORC12

**Origem:** Documentada. **Implementação:** [issue 18](../issues/018.md). **Fonte:** [SIC-001; pp.25 e 31](https://www.sicop.sc.gov.br/download/sicop/Guia_Orcamento_Consulta_e_Cadastro.pdf).

**Regra de referência:** A estrutura da planilha inclui grupo, etapa e subetapa; grupo exige grupo de serviço.

**Comportamento do Canteiro 360:** Modelar hierarquia ordenada sem ciclos, com identidade estável por nó e item. Preservar relações ao mover ou renumerar linhas.

**Aceite:** Mover uma etapa entre grupos recalcula os totais uma única vez e preserva seus itens; mover um grupo para dentro de si mesmo é recusado.

### ORC13

**Origem:** Documentada. **Implementação:** [issue 18](../issues/018.md). **Fonte:** [SIC-001; pp.26-29](https://www.sicop.sc.gov.br/download/sicop/Guia_Orcamento_Consulta_e_Cadastro.pdf).

**Regra de referência:** Itens podem ser incluídos por código ou copiados de tabelas e outros orçamentos.

**Comportamento do Canteiro 360:** Permitir inclusão e cópia com origem, unidade, competência e composição identificadas, sem vinculação mutável à tabela externa.

**Aceite:** Copiar um serviço para orçamento autorizado conserva sua procedência; atualização posterior no catálogo não altera o preço copiado sem uma ação de revisão.

### ORC14

**Origem:** Documentada. **Implementação:** [issue 17](../issues/017.md). **Fonte:** [SIC-001; pp.32-33](https://www.sicop.sc.gov.br/download/sicop/Guia_Orcamento_Consulta_e_Cadastro.pdf).

**Regra de referência:** Itens possuem serviço, utilização, documentos, composição e equipe mecânica; fórmula só se aplica quando Custo Informado não está selecionado.

**Comportamento do Canteiro 360:** Separar custo informado de custo calculado; composições têm componentes, coeficientes e unidades, sem referências circulares ou dupla incidência. Incidência percentual deve declarar sua base e não pode incluir a própria linha ou um subtotal que a contenha.

**Aceite:** Um serviço com custo informado não executa também sua fórmula; uma composição que referencia a si mesma, direta ou indiretamente, é rejeitada com o caminho do ciclo. Base direta de 100,00 com serviço percentual de 10% gera 10,00; usar como base um total que inclua esse mesmo serviço é recusado.

### ORC15

**Origem:** Documentada. **Implementação:** [issue 18](../issues/018.md). **Fonte:** [SIC-001; pp.34-37](https://www.sicop.sc.gov.br/download/sicop/Guia_Orcamento_Consulta_e_Cadastro.pdf).

**Regra de referência:** Planilha permite excluir, copiar, recortar, colar e localizar itens enquanto alocada ao usuário.

**Comportamento do Canteiro 360:** Executar operações sobre a seleção visível com prévia de impacto e desfazer em rascunhos; impedir alteração de revisões contratadas.

**Aceite:** Recortar uma etapa e colá-la em outro grupo não duplica quantidades; desfazer restaura hierarquia e valores; exclusão em revisão contratada é bloqueada.

### ORC16

**Origem:** Adaptada. **Implementação:** [issue 17](../issues/017.md). **Fonte:** [SIC-001; pp.38-39](https://www.sicop.sc.gov.br/download/sicop/Guia_Orcamento_Consulta_e_Cadastro.pdf).

**Regra de referência:** Distância de transporte depende de itens cadastrados; há tratamento por fórmula legado e por serviço para NOVO SICRO.

**Comportamento do Canteiro 360:** Suportar transporte por fórmula ou composição de serviço, com distância, unidade, origem e parâmetros explícitos; não atribuir fórmulas privadas à tabela oficial.

**Aceite:** Alterar a distância de um transporte recalcula somente composições dependentes; informar quilômetros em campo de metros exige conversão explícita e conserva o valor físico.

### ORC17

**Origem:** Documentada. **Implementação:** [issue 17](../issues/017.md). **Fonte:** [SIC-001; pp.40-49](https://www.sicop.sc.gov.br/download/sicop/Guia_Orcamento_Consulta_e_Cadastro.pdf).

**Regra de referência:** O orçamento contém combustíveis, materiais, mão de obra, equipamentos, serviços, incidências e transportes, editáveis em conferência.

**Comportamento do Canteiro 360:** Manter categorias e relações de composição, com consulta de utilização e impedimento de excluir componente já referenciado em versão aprovada.

**Aceite:** Excluir um material usado em composição aprovada é recusado e mostra os usos; alterar seu preço em nova revisão não muda o histórico.

### ORC18

**Origem:** Adaptada. **Implementação:** [issue 17](../issues/017.md). **Fonte:** [SIC-001; p.20, formulário de parâmetros](https://www.sicop.sc.gov.br/download/sicop/Guia_Orcamento_Consulta_e_Cadastro.pdf).

**Regra de referência:** O formulário apresenta tabela/data-base, salário, horas por mês, encargos, BDI, juros de equipamento e modo de aplicação do BDI.

**Comportamento do Canteiro 360:** Tratar os parâmetros como dados versionados do orçamento. Nenhum percentual ou salário do exemplo da fonte vira padrão obrigatório ou alíquota fiscal nacional.

**Aceite:** Dois orçamentos podem usar encargos e BDI distintos; alterar um parâmetro produz prévia de diferenças e memória de cálculo, sem atingir o outro.

### ORC19

**Origem:** Adaptada. **Implementação:** [issue 18](../issues/018.md). **Fonte:** [SIC-001; pp.50-51](https://www.sicop.sc.gov.br/download/sicop/Guia_Orcamento_Consulta_e_Cadastro.pdf).

**Regra de referência:** Cronograma orçamentário tem início, prazo em dias, grupos, períodos de 30 dias e percentuais que somam 100%; pode linearizar ou zerar.

**Comportamento do Canteiro 360:** Entregar em #18 o cronograma da versão de orçamento, com períodos de 30 dias como padrão documental. A opção de calendário civil deve ser declarada e registrada como adaptação. O cronograma de execução de #26 mantém sua própria política. Linearização e rateio seguem maiores restos, com desempate por posição estável crescente e depois identificador, conforme calculos.md.

**Aceite:** Para 90 dias e três períodos iguais com posições estáveis 1, 2 e 3, exibir 33,34%, 33,33% e 33,33%, somando exatamente 100%. Mudar a ordem de retorno da consulta não muda o destinatário do residual. Uma distribuição que some 99,99% impede a aprovação.

### ORC20

**Origem:** Documentada. **Implementação:** [issue 21](../issues/021.md). **Fonte:** [SIC-001; p.54](https://www.sicop.sc.gov.br/download/sicop/Guia_Orcamento_Consulta_e_Cadastro.pdf).

**Regra de referência:** Conferência apresenta ações consistentes e pendentes e permite ir à tela para corrigir cada pendência.

**Comportamento do Canteiro 360:** Apresentar cada bloqueio com item, motivo e ação corretiva, distinguindo erro impeditivo de aviso.

**Aceite:** Um orçamento sem quantidade em um item não é aprovado; selecionar o bloqueio leva à linha exata e o resultado da conferência atualiza após a correção.

### ORC21

**Origem:** Documentada. **Implementação:** [issue 20](../issues/020.md). **Fonte:** [SIC-001; p.54](https://www.sicop.sc.gov.br/download/sicop/Guia_Orcamento_Consulta_e_Cadastro.pdf).

**Regra de referência:** Reajuste do orçamento exige versão não aprovada e índices associados aos itens; pode haver fator adotado.

**Comportamento do Canteiro 360:** Aplicar atualização na revisão em preparação, identificando índice ou fator informado, competência e justificativa; separar de reajuste de medição contratual.

**Aceite:** Aplicar fator 1,10 a item de 100,00 produz 110,00 na revisão em preparação; a mesma operação é recusada na revisão aprovada e vinculada.

### ORC22

**Origem:** Documentada. **Implementação:** [issue 20](../issues/020.md). **Fonte:** [SIC-027; pp.3-5](https://www.sicop.sc.gov.br/download/sicop/PACOTE_DE_ALTERACAO_SICOP_04_2020_ALTERACOES_NA_PLANILHA_DO_ORCAMENTO.pdf).

**Regra de referência:** Um índice pode ser aplicado aos serviços selecionados ou a todos por opção específica.

**Comportamento do Canteiro 360:** Exibir quantidade de linhas e alcance da alteração antes de confirmar; todos os serviços exige escolha explícita.

**Aceite:** Selecionar dois entre dez serviços aplica o índice somente aos dois; a ação em todos exige nova confirmação e registra dez linhas atingidas.

### ORC23

**Origem:** Adaptada. **Implementação:** [issue 22](../issues/022.md). **Fonte:** [SIC-001; p.20](https://www.sicop.sc.gov.br/download/sicop/Guia_Orcamento_Consulta_e_Cadastro.pdf).

**Regra de referência:** O relatório para propostas deriva dos serviços da versão do orçamento.

**Comportamento do Canteiro 360:** Gerar solicitação privada de cotação vinculada à revisão, com itens e critérios comparáveis; ocultar preços internos quando o comprador assim definir.

**Aceite:** Duas propostas para a mesma solicitação mantêm os mesmos identificadores de itens; a exportação sem preços internos não os contém em células ocultas ou metadados.

### ORC24

**Origem:** Documentada. **Implementação:** [issue 14](../issues/014.md). **Fonte:** [SIC-012; pp.1-2](https://www.sicop.sc.gov.br/download/sicop/Modulo_de_Orcamento_Orientacao_de_preenchimento.pdf).

**Regra de referência:** Elaborador é o responsável técnico pelo orçamento, não o digitador; o cadastro identifica cargo, ART/RRT e empresa.

**Comportamento do Canteiro 360:** Manter elaborador técnico e autor da digitação como relações separadas e apresentá-las corretamente nos relatórios.

**Aceite:** O assistente que digita o orçamento não aparece como responsável técnico; o relatório identifica o profissional, sua empresa e o registro aplicável.

### ORC25

**Origem:** Adaptada. **Implementação:** [issue 14](../issues/014.md). **Fonte:** [SIC-048; p.4 física, exigência na aprovação; classificação de tipos em SIC-012 pp.1-2 físicas](https://www.sicop.sc.gov.br/download/sicop/Evolucao_01_2016.pdf).

**Regra de referência:** O pacote de 2016 exige ART/RRT aplicável somente ao passar o orçamento a Aprovado, não na finalização anterior. A orientação de preenchimento distingue tipos que exigem o campo e tipos Projetos, Manutenção e Serviços que o dispensam na interface histórica.

**Comportamento do Canteiro 360:** Avaliar responsabilidade técnica pela atividade e pela obrigação aplicável, com matriz de exigibilidade por transição. Finalizar a conferência exige elaborador e responsabilidades identificados; ART/RRT aplicável pode estar pendente nessa etapa, mas impede a aprovação. A dispensa do formulário histórico não constitui dispensa legal.

**Aceite:** Orçamento conferido com elaborador identificado e ART aplicável ainda pendente pode ficar Finalizado, mas não Aprovado. O documento válido habilita aprovação. Selecionar Projetos não elimina automaticamente a obrigação cadastrada para a atividade; não aplicabilidade exige fundamento registrado.

### ORC26

**Origem:** Adaptada. **Implementação:** [issue 08](../issues/008.md). **Fonte:** [SIC-012; pp.3-5](https://www.sicop.sc.gov.br/download/sicop/Modulo_de_Orcamento_Orientacao_de_preenchimento.pdf).

**Regra de referência:** Empresa orçamentista precisa de autorização por contrato e período, orçamento vinculado e usuário entre gestores.

**Comportamento do Canteiro 360:** Acesso externo é concedido por orçamento, contrato, finalidade e prazo, revogável pelo contratante; cada organização mantém seus próprios dados.

**Aceite:** O externo edita somente o orçamento concedido durante a validade; após expiração, nem a API nem um link antigo de exportação permitem novo acesso.

### ORC27

**Origem:** Adaptada. **Implementação:** [issue 19](../issues/019.md). **Fonte:** [SIC-016; p.5, figura da planilha](https://www.sicop.sc.gov.br/download/sicop/Manual%20SICOP%20Empreitadas.pdf).

**Regra de referência:** A figura mostra Importar do Excel, mas não especifica formato, campos e validações.

**Comportamento do Canteiro 360:** Definir importador próprio de XLSX e CSV, com mapa de colunas, prévia, validação completa e confirmação transacional; não prometer compatibilidade universal com arquivos SICOP.

**Aceite:** Um arquivo com 999 linhas válidas e uma unidade inválida importa zero linhas até correção ou exclusão explícita da linha; a prévia identifica número e motivo, sem executar fórmulas do arquivo.

## Medições e encerramento

### MED01

**Origem:** Documentada. **Implementação:** [issue 31](../issues/031.md). **Fonte:** [SIC-003; pp.34-35 impressas, PDF pp.35-36](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloMedicao.2010.0002.pdf).

**Regra de referência:** A capa é cadastrada antes dos quantitativos por serviços ou grupos.

**Comportamento do Canteiro 360:** Criar medição com identidade, contrato, período e regime antes de qualquer detalhe; impedir itens órfãos ou de outro contrato.

**Aceite:** Uma requisição de quantitativos sem capa válida ou com item de contrato diferente é recusada, sem gravação parcial.

### MED02

**Origem:** Documentada. **Implementação:** [issue 31](../issues/031.md). **Fonte:** [SIC-003; pp.35-36 impressas, PDF pp.36-37](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloMedicao.2010.0002.pdf).

**Regra de referência:** A capa guarda identificação, intervalo, percentuais físico/financeiro, tipo parcial/final, cronograma, atraso, ritmo e referência do diário.

**Comportamento do Canteiro 360:** Solicitar apenas os campos relevantes ao estado e tipo de medição, mantendo dados obrigatórios explícitos e vinculados ao histórico.

**Aceite:** Data final anterior à inicial é recusada; indicar cronograma atrasado solicita motivo; dados de medição financeira permanecem distintos do registro mensal sem avanço.

### MED03

**Origem:** Decisão. **Implementação:** [issue 31](../issues/031.md). **Fonte:** [SIC-003; p.35 impressa, PDF p.36; confronto com SIC-039 p.1](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloMedicao.2010.0002.pdf).

**Regra de referência:** A apostila recomenda medição mensal zerada sem serviços; a atualização de 2019 exige total positivo mesmo sem supressões.

**Comportamento do Canteiro 360:** Resolver a divergência com registro de competência sem avanço, valor zero e sem título financeiro. Medição financeira ordinária exige total do período estritamente positivo. Correções financeiras seguem fluxo próprio auditado.

**Aceite:** Registrar competência sem avanço preserva o acumulado e não gera fatura. Tentar concluir medição financeira ordinária de valor zero ou negativo é recusado com indicação do fluxo adequado.

### MED04

**Origem:** Adaptada. **Implementação:** [issue 60](../issues/060.md). **Fonte:** [SIC-003; p.35 impressa, PDF p.36](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloMedicao.2010.0002.pdf).

**Regra de referência:** Implantação permite valores históricos informados, inclusive reajuste, porque o SICOP trunca e pode divergir dos cálculos anteriores.

**Comportamento do Canteiro 360:** Migrar saldos de abertura com origem, data de corte, memória, responsável e conciliação; usar esses saldos como referência sem fabricar medições recalculadas.

**Aceite:** Importar acumulado histórico de 10,01, diferente do cálculo atual, exige aprovação da diferença; a primeira medição nova parte de 10,01 e não reescreve o histórico.

### MED05

**Origem:** Adaptada. **Implementação:** [issue 32](../issues/032.md). **Fonte:** [SIC-003; pp.35-36 impressas, PDF pp.36-37](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloMedicao.2010.0002.pdf).

**Regra de referência:** Medição soma quantidades executadas multiplicadas por preços; data final é comparada à referência e periodicidade do reajuste.

**Comportamento do Canteiro 360:** Calcular o período por diferença dos valores acumulados, conforme OMDEC01 e calculos.md; preservar separação entre valor base, reajuste e deduções.

**Aceite:** Com preço 10,00 e acumulados de 20 e 35 unidades, a segunda medição mostra 15 unidades do período e 150,00; não mostra 350,00 como valor devido no período.

### MED06

**Origem:** Adaptada. **Implementação:** [issue 31](../issues/031.md). **Fonte:** [SIC-048; p.4 física, atualização do percentual físico; antecedente SIC-003 p.36 impressa](https://www.sicop.sc.gov.br/download/sicop/Evolucao_01_2016.pdf).

**Regra de referência:** O pacote de 2016 retira a validação de percentual físico inferior ao anterior quando existe aditivo de valor ou quantidade, corrigindo a regra do guia de 2010.

**Comportamento do Canteiro 360:** Permitir redução do percentual físico quando decorrer de nova base de escopo formalmente aprovada. Preservar quantidades, valores e percentuais históricos com a versão do denominador usado. Redução de execução efetivamente reconhecida segue correção própria; aditivo não autoriza apagar execução. Mostrar a causa da mudança do percentual e não o equiparar automaticamente ao financeiro.

**Aceite:** Com 40 unidades executadas sobre 100, o avanço é 40%; aditivo válido que eleva o escopo a 200 permite exibir 20% sobre a nova base, mantendo as 40 unidades e os valores reconhecidos. Sem alteração formal de base, informar 39% após 40% continua bloqueado no fluxo ordinário.

### MED07

**Origem:** Documentada. **Implementação:** [issue 35](../issues/035.md). **Fonte:** [SIC-003; p.36 impressa, PDF p.37](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloMedicao.2010.0002.pdf).

**Regra de referência:** Reajuste pode ser informado ou calculado, com valores provisório, definitivo e complementar e competências por periodicidade.

**Comportamento do Canteiro 360:** Registrar separadamente modos e valores; fator manual exige origem e justificativa, não substitui valores já aprovados.

**Aceite:** Definitivo de 1.200,00 e provisório já reconhecido de 1.000,00 geram complemento de 200,00, sem cobrar novamente o provisório.

### MED08

**Origem:** Documentada. **Implementação:** [issue 36](../issues/036.md). **Fonte:** [SIC-003; p.37 impressa, PDF p.38](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloMedicao.2010.0002.pdf).

**Regra de referência:** Conferência admite edição; cálculo definitivo bloqueia; análise/aprovação posterior permite encaminhar ao pagamento.

**Comportamento do Canteiro 360:** Separar conferência, cálculo definitivo, aprovação e liberação financeira. Correção posterior preserva versão, autor e decisão; aprovação não é registro de pagamento.

**Aceite:** Após cálculo definitivo, editar um quantitativo é recusado; após aprovação, o relatório ainda mostra valor não pago até registro financeiro efetivo.

### MED09

**Origem:** Adaptada. **Implementação:** [issue 34](../issues/034.md). **Fonte:** [SIC-003; pp.38 e 44 impressas, PDF pp.39 e 45](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloMedicao.2010.0002.pdf).

**Regra de referência:** Há quantidade exigida de profissionais, com padrão de dois, um de cada parte, validada no cálculo definitivo.

**Comportamento do Canteiro 360:** Usar composição de comissão definida no contrato, com representantes de contratante e contratado e regra de substituição; qualquer exceção exige alçada e justificativa.

**Aceite:** Com comissão de dois representantes, apenas um vínculo válido impede a conclusão; adicionar o representante habilitado da outra parte permite nova conferência.

### MED10

**Origem:** Documentada. **Implementação:** [issue 34](../issues/034.md). **Fonte:** [SIC-003; pp.38-39 impressas, PDF pp.39-40](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloMedicao.2010.0002.pdf).

**Regra de referência:** A comissão é vinculada à obra e seus profissionais precisam de atuação e ART válidas no período da medição.

**Comportamento do Canteiro 360:** Validar atuação, vínculo e documento técnico aplicável à atividade e ao intervalo; documentos substitutos preservam a cadeia de responsabilidade.

**Aceite:** Um profissional com atuação encerrada antes da medição não pode assinar sua conferência; substituto válido entra com vigência explícita sem apagar o anterior.

### MED11

**Origem:** Adaptada. **Implementação:** [issue 34](../issues/034.md). **Fonte:** [SIC-003; pp.39-40 impressas, PDF pp.40-41](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloMedicao.2010.0002.pdf).

**Regra de referência:** Fotos associadas pertencem ao período da medição; o guia prevê seleção e associação automática de JPG.

**Comportamento do Canteiro 360:** Aceitar formatos documentados pelo produto, preservando original, data de captura declarada e data de envio; associar automaticamente apenas evidências compatíveis com obra e período.

**Aceite:** Uma foto de outra obra ou fora do período não entra por associação automática; eventual evidência complementar exige vínculo justificado e aparece identificada como tal.

### MED12

**Origem:** Documentada. **Implementação:** [issue 33](../issues/033.md). **Fonte:** [SIC-003; p.41 impressa, PDF p.42](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloMedicao.2010.0002.pdf).

**Regra de referência:** Medição por grupo recebe percentual acumulado aplicado aos serviços do grupo.

**Comportamento do Canteiro 360:** A ação de percentual do grupo exibe previamente os itens atingidos e não soma novamente percentuais já acumulados.

**Aceite:** Aplicar 40% ao grupo com dois itens de 100,00 e 200,00 resulta em acumulados de 40,00 e 80,00; repetir 40% não gera novo valor devido.

### MED13

**Origem:** Documentada. **Implementação:** [issue 32](../issues/032.md). **Fonte:** [SIC-003; pp.42-43 impressas, PDF pp.43-44](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloMedicao.2010.0002.pdf).

**Regra de referência:** Medição por serviço recebe quantidade ou percentual executado acumulado.

**Comportamento do Canteiro 360:** Usar acumulado como entrada principal e mostrar diferença do período. Quantidade e percentual são representações da mesma execução, nunca valores a somar.

**Aceite:** Ao informar 50 unidades em item contratado com 100, o percentual exibe 50%; informar também 50% não duplica a execução. Unidades incompatíveis impedem salvar.

### MED14

**Origem:** Documentada. **Implementação:** [issue 36](../issues/036.md). **Fonte:** [SIC-003; pp.43-44 impressas, PDF pp.44-45](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloMedicao.2010.0002.pdf).

**Regra de referência:** O cálculo atua na última medição ainda não definitiva, consolida por obra/grupo e valida comissão e contratado versus executado.

**Comportamento do Canteiro 360:** Finalizar em transação única sobre uma revisão identificada, impedindo execução acima do contratado vigente e encerramento duplicado.

**Aceite:** Duas tentativas simultâneas de finalizar a mesma revisão produzem uma conclusão e um mesmo resultado consultável; nenhuma ultrapassa quantidade contratada.

### MED15

**Origem:** Documentada. **Implementação:** [issue 48](../issues/048.md). **Fonte:** [SIC-042; pp.1-2 físicas; prevalece sobre SIC-003 pp.36 e 44 impressas](https://www.sicop.sc.gov.br/download/sicop/Pacote_de_alteracoes_02_2017_MedicaoFinalERecebimentoProvisorio.pdf).

**Regra de referência:** O cálculo definitivo final conclui o contrato, mas a medição fica Aguardando Recebimento Provisório.

**Comportamento do Canteiro 360:** Registrar conclusão física/contratual sem confundi-la com liberação financeira ou recebimento definitivo; manter pendências de encerramento visíveis.

**Aceite:** Calcular definitivamente a medição final muda o estado contratual previsto, mas continua bloqueando sua liberação financeira enquanto faltar recebimento provisório.

### MED16

**Origem:** Documentada. **Implementação:** [issue 48](../issues/048.md). **Fonte:** [SIC-042; pp.2-5 físicas](https://www.sicop.sc.gov.br/download/sicop/Pacote_de_alteracoes_02_2017_MedicaoFinalERecebimentoProvisorio.pdf).

**Regra de referência:** Termo provisório assinado entra nos documentos do contrato como Recebimento; o fiscal depois libera a medição final para Concluída.

**Comportamento do Canteiro 360:** Exigir termo assinado, vínculo ao contrato e decisão de liberação por responsável autorizado; guardar evidência e versão exatas.

**Aceite:** Um arquivo sem assinatura ou vinculado a outro contrato não libera a medição; o termo correto permite a ação e gera registro imutável da decisão.

### MED17

**Origem:** Adaptada. **Implementação:** [issue 48](../issues/048.md). **Fonte:** [SIC-042; p.5 física](https://www.sicop.sc.gov.br/download/sicop/Pacote_de_alteracoes_02_2017_MedicaoFinalERecebimentoProvisorio.pdf).

**Regra de referência:** Solicitação de pagamento final depende de medição concluída; somente concluídas seguem para liquidação no SIGEF.

**Comportamento do Canteiro 360:** Liberar cobrança ou título financeiro somente após conclusão da medição. Cada valor liberado mantém origem faturável única e saldo transacional, permitindo várias faturas e títulos parciais vinculados à mesma origem. Substituir a integração pública por contas a pagar/receber privadas, sem confundir liberação, faturamento e pagamento.

**Aceite:** Medição final aguardando termo não gera valor faturável. Após liberação de 1.000,00, faturar 400,00 e depois 600,00 é permitido; faturar mais 0,01 é recusado. Repetir o comando de liberação não cria nova origem ou saldo, e nenhum desses atos registra pagamento automático.

### MED18

**Origem:** Documentada. **Implementação:** [issue 48](../issues/048.md). **Fonte:** [SIC-042; p.1 física](https://www.sicop.sc.gov.br/download/sicop/Pacote_de_alteracoes_02_2017_MedicaoFinalERecebimentoProvisorio.pdf).

**Regra de referência:** O termo provisório distingue obra com saldo a medir da obra com todos quantitativos medidos.

**Comportamento do Canteiro 360:** Permitir final com saldo remanescente identificado, motivo e destinação; não forçar 100% executado para encerrar.

**Aceite:** Contrato de 100 unidades pode encerrar com 95 executadas e saldo de 5 formalmente tratado; relatório não apresenta as 5 como executadas ou pagas.

### MED19

**Origem:** Adaptada. **Implementação:** [issue 49](../issues/049.md). **Fonte:** [SIC-003; pp.32-33 impressas, PDF pp.33-34](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloMedicao.2010.0002.pdf).

**Regra de referência:** Recebimento definitivo tem data não anterior ao provisório e registra pagamento final, habite-se, as built, manutenção e representantes.

**Comportamento do Canteiro 360:** Controlar recebimento definitivo e obrigações aplicáveis com justificativas de não aplicabilidade; preservar situação financeira real independentemente do aceite técnico.

**Aceite:** Recebimento definitivo anterior ao provisório é recusado; aceitar tecnicamente uma obra não muda um pagamento pendente para pago nem inventa habite-se.

## Supressão de serviços já medidos

### SUP01

**Origem:** Adaptada. **Implementação:** [issue 41](../issues/041.md). **Fonte:** [SIC-039; p.1 física](https://www.sicop.sc.gov.br/download/sicop/Pacote_de_alteracoes_2019_1.pdf).

**Regra de referência:** Fiscal propõe supressão de serviço medido durante Conferência, com confirmação ao salvar.

**Comportamento do Canteiro 360:** Representante técnico propõe correção negativa de acumulado, identificando item, medição de origem, motivo e impacto, sem alterar a origem.

**Aceite:** Reduzir acumulado de 20 para 15 unidades cria proposta de supressão de 5, com prévia financeira; a medição anterior permanece com seu valor histórico.

### SUP02

**Origem:** Adaptada. **Implementação:** [issue 41](../issues/041.md). **Fonte:** [SIC-039; pp.1-2 físicas](https://www.sicop.sc.gov.br/download/sicop/Pacote_de_alteracoes_2019_1.pdf).

**Regra de referência:** Itens suprimidos são conferidos e justificados e enviados ao ordenador escolhido, ficando Aguardando Aprovação.

**Comportamento do Canteiro 360:** Submeter conjunto imutável ao aprovador privado competente; qualquer alteração no conteúdo invalida a solicitação anterior.

**Aceite:** Depois do envio, alterar quantidade de um item exige nova revisão e nova aprovação; um aceite da revisão anterior não vale para a revisão alterada.

### SUP03

**Origem:** Documentada. **Implementação:** [issue 41](../issues/041.md). **Fonte:** [SIC-039; pp.2-3 físicas](https://www.sicop.sc.gov.br/download/sicop/Pacote_de_alteracoes_2019_1.pdf).

**Regra de referência:** Aprovação ou rejeição vale para toda a lista; não há decisão por item isolado.

**Comportamento do Canteiro 360:** Manter decisão atômica sobre a lista apresentada com seu resumo e memória de impacto.

**Aceite:** A API recusa aprovar apenas um item de uma lista de três; uma aprovação integral aplica os três efeitos juntos ou nenhum.

### SUP04

**Origem:** Documentada. **Implementação:** [issue 41](../issues/041.md). **Fonte:** [SIC-039; p.3 física](https://www.sicop.sc.gov.br/download/sicop/Pacote_de_alteracoes_2019_1.pdf).

**Regra de referência:** A aprovação permite continuar com a supressão; a rejeição permite continuar sem ela.

**Comportamento do Canteiro 360:** Aplicar deltas negativos somente depois do aceite; a rejeição preserva proposta e motivo, mas exclui seu efeito da medição.

**Aceite:** Rejeitar supressão de 50,00 mantém o valor devido sem essa redução e registra a decisão; nenhum relatório posterior trata a proposta rejeitada como executada.

### SUP05

**Origem:** Adaptada. **Implementação:** [issue 41](../issues/041.md). **Fonte:** [SIC-039; pp.3-4 físicas](https://www.sicop.sc.gov.br/download/sicop/Pacote_de_alteracoes_2019_1.pdf).

**Regra de referência:** Anulação exige justificativa e nova decisão do ordenador; quando aceita, o estado é Anulado; podem existir documentos de suporte.

**Comportamento do Canteiro 360:** Separar anulação de proposta ainda não aplicada de reversão de efeito já consolidado; a segunda usa correção vinculada e não apaga medição ou pagamento histórico.

**Aceite:** Anular uma supressão já incorporada a medição definitiva não edita essa medição; gera correção de efeito oposto, vinculada e sujeita à alçada.

### SUP06

**Origem:** Decisão. **Implementação:** [issue 41](../issues/041.md). **Fonte:** [SIC-039; p.1 física; conflito com SIC-003 p.35 impressa](https://www.sicop.sc.gov.br/download/sicop/Pacote_de_alteracoes_2019_1.pdf).

**Regra de referência:** A atualização exige total positivo independentemente da existência de supressões.

**Comportamento do Canteiro 360:** Exigir saldo base do período positivo na medição financeira ordinária com supressões. Para saldo zero usar registro sem movimento; para saldo negativo usar correção/estorno controlado, nunca forçar um item fictício.

**Aceite:** Supressão de 50,00 e execução positiva de 100,00 deixam base de 50,00; supressão de 100,00 sem nova execução é encaminhada a correção controlada e não vira medição ordinária negativa.

## Empreitada por preço global

### EPG01

**Origem:** Documentada. **Implementação:** [issue 25](../issues/025.md). **Fonte:** [SIC-016; pp.4 e 6 impressas, PDF pp.5 e 7](https://www.sicop.sc.gov.br/download/sicop/Manual%20SICOP%20Empreitadas.pdf).

**Regra de referência:** O regime Empreitada por Preço Global habilita medição por etapas.

**Comportamento do Canteiro 360:** Definir regime na contratação e vedar troca silenciosa após execução; a mudança formal exige análise de impactos e preservação do histórico.

**Aceite:** Contrato global oferece configuração de etapas; trocar para unitário após medição não é uma edição simples de cadastro.

### EPG02

**Origem:** Documentada. **Implementação:** [issue 25](../issues/025.md). **Fonte:** [SIC-016; p.5 impressa, PDF p.6](https://www.sicop.sc.gov.br/download/sicop/Manual%20SICOP%20Empreitadas.pdf).

**Regra de referência:** Cada grupo do orçamento precisa de etapa e cada etapa precisa de itens para permitir a medição.

**Comportamento do Canteiro 360:** Validar a estrutura no orçamento e novamente na formação da referência contratual global.

**Aceite:** Um grupo sem etapa ou uma etapa vazia impedem habilitar a execução global, com indicação da estrutura a corrigir.

### EPG03

**Origem:** Adaptada. **Implementação:** [issue 33](../issues/033.md). **Fonte:** [SIC-016; p.7 impressa, PDF p.8](https://www.sicop.sc.gov.br/download/sicop/Manual%20SICOP%20Empreitadas.pdf).

**Regra de referência:** O percentual executado da etapa alimenta cálculo automático progressivo que considera medições anteriores.

**Comportamento do Canteiro 360:** Medir percentual acumulado da etapa e calcular diferença monetária conforme calculos.md; mudanças em etapas já medidas seguem aditivo e não recalculam retroativamente.

**Aceite:** Para etapa de 100.000,00 com acumulados 20% e 45%, a segunda medição deve 25.000,00. Repetir 45% não gera novo valor e 101% é recusado.

## Manutenção por catálogo e limite financeiro

### ATA01

**Origem:** Adaptada. **Implementação:** [issue 51](../issues/051.md). **Fonte:** [SIC-021; p.1 física](https://www.sicop.sc.gov.br/download/sicop/Pacote_de_alteracao_04_2021_atas_manutencao_sinapi%20.pdf).

**Regra de referência:** Ata sem quantidades usa tabela SINAPI e valida o valor total informado na capa do contrato.

**Comportamento do Canteiro 360:** Representar acordo privado por catálogo e limite financeiro. A reserva é assumida na submissão da OS; a confirmação técnica de execução da issue 54 converte somente o valor aceito em consumido, reduzindo a reserva remanescente pelo mesmo valor. Disponível = limite vigente - consumido - reserva remanescente. Divergência conserva a reserva protegida até decisão. Cancelamento libera somente reserva não consumida; medição e fatura usam o consumo existente, sem consumi-lo novamente.

**Aceite:** Limite 100,00 e OS 80,00 deixam 20,00 disponíveis. Confirmar tecnicamente execução de 50,00 deixa consumido 50,00 e reserva 30,00. Cancelar os 30,00 restantes deixa disponível 50,00; nova OS de 60,00 é recusada. Repetir confirmação, medição ou cancelamento não muda o resultado. Na issue 51 testar o domínio de saldos; integrar as transições reais nas issues 53, 54 e 70.

### ATA02

**Origem:** Adaptada. **Implementação:** [issue 51](../issues/051.md). **Fonte:** [SIC-021; pp.1-2 físicas](https://www.sicop.sc.gov.br/download/sicop/Pacote_de_alteracao_04_2021_atas_manutencao_sinapi%20.pdf).

**Regra de referência:** Manutenção em ata sem quantidades exige BDI e desconto; alterações são permitidas somente em A iniciar.

**Comportamento do Canteiro 360:** Validar percentuais e regime antes de iniciar; mudanças posteriores exigem revisão formal com vigência e não afetam ordens já aprovadas.

**Aceite:** Sem BDI ou desconto explicitamente informado, mesmo que zero, o acordo não inicia; alteração direta após a primeira ordem aprovada é recusada.

### ATA03

**Origem:** Adaptada. **Implementação:** [issue 51](../issues/051.md). **Fonte:** [SIC-021; pp.2-4 físicas](https://www.sicop.sc.gov.br/download/sicop/Pacote_de_alteracao_04_2021_atas_manutencao_sinapi%20.pdf).

**Regra de referência:** Importar SINAPI cria a base dos pedidos; alterar BDI/desconto após importar remove a tabela, apenas em A iniciar.

**Comportamento do Canteiro 360:** Antes do início, invalidar a versão importada e exigir nova geração com prévia ao alterar BDI/desconto, preservando versões anteriores para auditoria.

**Aceite:** Alterar BDI antes do início deixa o catálogo anterior indisponível para novas ordens até recomposição; a tabela antiga continua consultável no histórico.

### ATA04

**Origem:** Decisão. **Implementação:** [issue 51](../issues/051.md). **Fonte:** [SIC-021; p.4 física; fórmula completa não publicada](https://www.sicop.sc.gov.br/download/sicop/Pacote_de_alteracao_04_2021_atas_manutencao_sinapi%20.pdf).

**Regra de referência:** Serviços do contrato refletem o desconto sobre os dados importados; a fonte não detalha ordem de BDI/desconto nem truncagens.

**Comportamento do Canteiro 360:** Aplicar a fórmula e a precisão explicitadas em calculos.md, com custo-base, BDI, desconto e preço final discriminados; congelar a política na versão do acordo.

**Aceite:** O mesmo item e os mesmos parâmetros produzem valor idêntico na prévia e no servidor; a memória permite reconstruir o preço e identificar a versão da política.

### ATA05

**Origem:** Documentada. **Implementação:** [issue 53](../issues/053.md). **Fonte:** [SIC-020; p.1 física](https://www.sicop.sc.gov.br/download/sicop/Pacote_de_alteracao_05_2021_Novos_servicos_em_Atas.pdf).

**Regra de referência:** Atualizar Ata adiciona serviços à ata já vinculada, apenas em Elaboração O.S.; os serviços não entram automaticamente no pedido.

**Comportamento do Canteiro 360:** Atualizar catálogo do acordo de forma explícita; a inclusão em ordem em preparação exige seleção separada e novo cálculo de reserva.

**Aceite:** Após atualizar o catálogo com um serviço novo, o valor e a seleção da ordem permanecem iguais; só a inclusão confirmada do serviço modifica sua reserva.

## Consultas e relatórios

### REL01

**Origem:** Documentada. **Implementação:** [issue 59](../issues/059.md). **Fonte:** [SIC-003; pp.45-46 impressas, PDF pp.46-47](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloMedicao.2010.0002.pdf).

**Regra de referência:** A lista de consistência distingue registro correto, ausência opcional, inconsistência e falta obrigatória, com acesso à origem.

**Comportamento do Canteiro 360:** Oferecer painel de conferência de contrato com estado, fundamento, entidade atingida e ação corretiva.

**Aceite:** Um anexo opcional ausente não bloqueia encerramento; uma assinatura obrigatória ausente aparece como bloqueio e leva ao documento correspondente.

### REL02

**Origem:** Adaptada. **Implementação:** [issue 09](../issues/009.md). **Fonte:** [SIC-003; p.47 impressa, PDF p.48](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloMedicao.2010.0002.pdf).

**Regra de referência:** O histórico apresenta eventos do contrato em ordem cronológica e gera relatório.

**Comportamento do Canteiro 360:** Manter trilha cronológica com instante do evento, instante do registro, autor, versão e origem; eventos retroativos não ocultam a ordem real de registro.

**Aceite:** Cadastrar hoje evento ocorrido na semana anterior conserva as duas datas e não altera registros existentes; o relatório permite identificar a inclusão tardia.

### REL03

**Origem:** Adaptada. **Implementação:** [issue 59](../issues/059.md). **Fonte:** [SIC-003; pp.48-49 impressas, PDF pp.49-50](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloMedicao.2010.0002.pdf).

**Regra de referência:** Relatórios gerenciais mostram contratos e valores distribuídos por município, em PDF, tela e gráficos.

**Comportamento do Canteiro 360:** Oferecer carteira privada por empresa, empreendimento, localização e responsável, com filtros e total reconciliado ao conjunto autorizado.

**Aceite:** Filtrar duas obras autorizadas produz o mesmo total em tabela, gráfico e PDF; valores de uma obra não autorizada não entram nem no total agregado.

### REL04

**Origem:** Adaptada. **Implementação:** [issue 59](../issues/059.md). **Fonte:** [SIC-003; pp.49-53 impressas, PDF pp.50-54](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloMedicao.2010.0002.pdf).

**Regra de referência:** Relatórios cobrem advertências, conclusão, cronograma, diário, ficha, orçamento, paralisação, pré-aditivo, serviços e recebimentos.

**Comportamento do Canteiro 360:** Gerar documentos privados completos com filtros, referência, revisão, período, assinaturas aplicáveis e identificação do estado do documento.

**Aceite:** O relatório de paralisação mostra a ordem e as fotos escolhidas; orçamento e cronograma exibem a revisão usada; rascunhos não parecem documentos aprovados.

### REL05

**Origem:** Documentada. **Implementação:** [issue 37](../issues/037.md). **Fonte:** [SIC-003; pp.54-55 impressas, PDF pp.55-56](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloMedicao.2010.0002.pdf).

**Regra de referência:** Caderno da medição inclui solicitação de pagamento, capa/fotos, resumo por grupo, quantidades e comparativo contratado versus executado.

**Comportamento do Canteiro 360:** Emitir o caderno a partir da mesma revisão imutável usada no cálculo, com anteriores, período, acumulados, reajustes e saldos identificados.

**Aceite:** Somar itens, grupos e obras no caderno reconcilia com a capa e o cálculo aprovado; uma reimpressão conserva os valores históricos mesmo após aditivo posterior.

### REL06

**Origem:** Documentada. **Implementação:** [issue 37](../issues/037.md). **Fonte:** [SIC-003; pp.52-55 impressas, PDF pp.53-56](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloMedicao.2010.0002.pdf).

**Regra de referência:** Orçamento básico, serviços contratados e quantidades executadas têm saída Excel; relatórios citados têm PDF.

**Comportamento do Canteiro 360:** Exportar XLSX e PDF consistentes com a consulta autorizada, usando números reais, unidades e fórmulas próprias seguras quando aplicável.

**Aceite:** O XLSX exportado abre com quantidades numéricas e totais iguais ao PDF; descrições iniciadas por sinal de fórmula permanecem texto e não executam conteúdo.

## Decisões de implementação

### OMDEC01

**Origem:** Decisão. **Implementação:** [issue 05](../issues/005.md). **Fonte:** [SIC-003; p.35 impressa, PDF p.36; truncagem citada sem escala ou ordem completas](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloMedicao.2010.0002.pdf).

**Regra de referência:** A fonte informa truncagem, mas não define o algoritmo monetário completo.

**Comportamento do Canteiro 360:** Adotar valor do período por item igual a trunc 2 do valor acumulado atual menos o valor acumulado anterior reconhecido, conforme calculos.md. Os dados de entrada e resultados são decimais exatos compartilhados em Rust; o servidor decide o resultado definitivo.

**Aceite:** Para preço 0,335 e quantidades acumuladas 1, 2 e 3, os valores de período são 0,33, 0,34 e 0,33, totalizando 1,00; cliente e servidor concordam em todos os vetores.

### OMDEC02

**Origem:** Decisão. **Implementação:** [issue 40](../issues/040.md). **Fonte:** [SIC-016; p.7 impressa, PDF p.8; distribuição de alteração de etapa não especificada](https://www.sicop.sc.gov.br/download/sicop/Manual%20SICOP%20Empreitadas.pdf).

**Regra de referência:** A fonte descreve progressividade, mas não resolve integralmente mudanças de preço ou escopo depois de medições anteriores.

**Comportamento do Canteiro 360:** Congelar segmentos de preço e escopo na referência contratual. Novo aditivo define vigência e saldo afetado; não recalcular o acumulado anterior pelo preço novo. Estornos referenciam o valor originalmente reconhecido.

**Aceite:** Após 10 unidades reconhecidas a 10,00, um aditivo que fixe 12,00 para cinco unidades futuras gera 60,00 nessas cinco, sem reavaliar os 100,00 anteriores.

### OMDEC03

**Origem:** Decisão. **Implementação:** [issue 04](../issues/004.md). **Fonte:** [SIC-003; pp.41-44 impressas, PDF pp.42-45; interface moderna definida pelo produto](https://www.sicop.sc.gov.br/download/sicop/AT-SICOP-ModuloMedicao.2010.0002.pdf).

**Regra de referência:** As fontes usam entradas acumuladas e cálculo definitivo, sem especificar a experiência de interface do Canteiro 360.

**Comportamento do Canteiro 360:** Na tabela de medição, exibir Contratado, Anterior acumulado, Atual acumulado, Diferença do período, Valor do período e Saldo. Prévia monetária deve responder à edição; erros indicam campo, limite e ação, sem esconder dados.

**Aceite:** Em teste de uso, o usuário altera o acumulado sem confundi-lo com o período, encontra a diferença antes de confirmar e corrige excesso de quantidade pelo erro que informa o saldo permitido.

## Condições de aprovação da implementação

A entrega de cada issue deve vincular os testes às regras que implementa. Os testes cobrem comportamento normal, limites, duplicidade, concorrência, permissões e efeitos no histórico conforme pertinente. A aprovação documental desta especificação não dispensa a validação do software, da operação financeira e dos fluxos reais antes da liberação comercial da issue 72.

Os seguintes erros impedem a liberação: faturamento duplicado de acumulados, divergência entre valores aprovados e exportados, edição retroativa de medição definitiva, execução acima do saldo sem alteração autorizada, aceite de supressão por pessoa sem alçada, divulgação de dados entre empresas e liberação final sem o recebimento exigido.
