# Experiência e desempenho

Especificação de produto de 11/11/2026. Estes critérios são obrigatórios para a implementação do Canteiro 360. Os valores representam metas de aceitação, ainda não resultados de um sistema em produção.

## 1. Resultado esperado

O usuário deve saber o que precisa fazer, encontrar a ação e concluir a tarefa sem conhecer a estrutura interna do SICOP. A interface pode reorganizar as telas e os passos, mantendo as regras documentadas de cálculo, estado, permissão, dependência e histórico que se aplicam ao produto privado.

O rigor aparece no momento da decisão: saldo disponível, versão, diferença de valor ou prazo, documento utilizado, responsável e consequência da ação. Campos e confirmações sem utilidade para a tarefa não devem ocupar o percurso principal. Um bloqueio legítimo precisa explicar a causa e o caminho autorizado para resolvê-lo.

As primeiras quatro issues criam a base técnica e visual. Cada módulo entrega sua experiência completa e acessível. As issues 63 e 64 verificam a consistência do conjunto; a issue 71 valida o uso profissional, e a issue 72 exige as evidências finais.

## 2. Arquitetura da interface

O frontend será escrito em Rust com Leptos, HTML gerado no servidor e hidratação WebAssembly. O servidor usará Axum/Tokio e SQLx com PostgreSQL. Componentes, validações de apresentação e regras compartilhadas pertencem ao projeto Rust. A autorização e a validação definitiva sempre ocorrem no servidor.

Não haverá uma segunda aplicação nem uma segunda implementação de regras de negócio em JavaScript ou TypeScript. O código de interoperabilidade gerado pelas ferramentas, HTML, CSS, SQL, configuração e os protocolos de serviços externos não mudam essa decisão. Integrações externas devem ser limitadas, registradas e carregadas apenas onde forem necessárias.

Conteúdo essencial, navegação e os formulários compatíveis com envio HTML devem funcionar antes da hidratação. A interface informa quando uma operação mais rica ainda está carregando. Nenhum dado privado pode aparecer em HTML, cache ou resposta de outro cliente.

O tamanho do WebAssembly deve ser medido a cada alteração relevante. A documentação do Leptos explica que a hidratação convencional aumenta o binário conforme cresce a aplicação; portanto, Rust não dispensa controle de carregamento, paginação e memória. Redução da hidratação, separação de partes ou carregamento posterior devem ser comprovados na versão adotada. [Documentação oficial](https://book.leptos.dev/islands.html).

## 3. Navegação por contexto e tarefa

| Contexto | Informação principal | Ações esperadas |
|---|---|---|
| Carteira | Obras, prazos críticos, pendências e exposição financeira | Abrir obra, filtrar, comparar e identificar a próxima decisão |
| Obra | Estado, responsáveis, orçamento, contrato e próximos marcos | Registrar execução, consultar documento, medir e resolver pendência |
| Projetos | Entregas, versões, pedidos de esclarecimento e decisões | Enviar revisão, conferir, aprovar e distribuir a versão vigente |
| Contrato | Objeto, regime, vigência, serviços, alterações e saldo | Preparar ordem, acompanhar medição, propor alteração e consultar histórico |
| Medição | Período, acumulados, evidências, glosas, reajustes e líquido | Preparar, conferir, corrigir, aprovar e emitir caderno |
| Financeiro | Faturado, vencido, pago, recebido, retido e saldo | Registrar título, anexar documento, conciliar e tratar diferença |
| Manutenção | Solicitação, aprovação, reserva, execução e aceite | Autorizar serviço, registrar atendimento, conferir e encerrar |

O seletor de organização permanece visível. A página identifica a obra e, quando aplicável, o contrato e a versão. A troca de contexto limpa dados da organização anterior e não transporta rascunhos para a nova. Voltar de um detalhe conserva filtros, página e posição de leitura, desde que o usuário mantenha a permissão.

A página inicial é ajustada ao papel e aos vínculos efetivos. Um fornecedor não recebe menus que aparentem permitir acesso à carteira inteira. A busca retorna somente objetos autorizados. Favoritos nunca ampliam permissão.

## 4. Sistema visual

O produto deve transmitir clareza e confiança em telas densas. A marca Canteiro 360 identifica todos os pontos do sistema; não serão usados nomes ou elementos visuais de produtos terceiros como identidade da aplicação.

| Elemento | Regra inicial |
|---|---|
| Fundo e superfícies | Fundo `#F7F9FC`, superfície `#FFFFFF`, texto `#172033`, texto secundário `#475569` |
| Ação principal | Azul `#1D4ED8`, com rótulo explícito e contraste conferido no estado real |
| Estados | Verde `#166534`, âmbar `#92400E` e vermelho `#B91C1C`, sempre acompanhados de texto ou símbolo identificável |
| Foco e contornos | Foco visível de 3 px; contorno de campo identificável; nenhum estado depende apenas de sombra |
| Tipografia | Fontes do sistema, corpo padrão de 16 px, entrelinha de 1,5, números tabulares em valores e quantidades |
| Espaçamento | Escala comum baseada em 4 e 8 px; alinhamento consistente entre rótulos, entradas e totais |
| Ações de toque | Área principal de pelo menos 44 por 44 CSS px; o modo compacto preserva as condições de acessibilidade aplicáveis |
| Tabelas | Identificação do serviço, unidade e cabeçalho persistentes; coluna monetária alinhada à direita; total não se confunde com linha editável |

Os valores visuais são decisões de produto. O atendimento de contraste e acessibilidade deve ser medido nas combinações efetivamente utilizadas, inclusive erro, foco, seleção, desabilitado e impressão. Uma paleta aprovada isoladamente não comprova a acessibilidade da tela inteira.

Componentes comuns devem cobrir botão, campo, seletor, calendário, tabela, painel de totais, carregamento, vazio, erro, ausência de permissão, arquivo, histórico, comparação de versões e aprovação. Os módulos não devem inventar versões próprias desses comportamentos.

## 5. Formulários, rascunhos e prevenção de erros

1. Rótulos são permanentes, claros e associados ao campo. Exemplo de preenchimento não substitui rótulo.
2. Campos obrigatórios e unidades são visíveis. A máscara não impede colar CPF/CNPJ, datas ou valores válidos. CNPJ alfanumérico conserva suas letras.
3. A leitura e a edição usam convenções brasileiras. A API recebe valores decimais sem ambiguidade, conforme a regra de precisão do domínio; não usa ponto flutuante binário para dinheiro.
4. A validação local ajuda o usuário. A validação do servidor decide se o ato é permitido, inclusive depois de alteração de papel, saldo, versão ou estado.
5. Um erro preserva entradas válidas, destaca os campos afetados, move o foco de forma previsível e informa como corrigir. Mensagens técnicas ficam fora do fluxo normal.
6. Rascunhos têm estado, autor, versão e horário de salvamento. Uma tentativa sem conexão não aparece como salva no servidor.
7. Alterações concorrentes exibem as diferenças e exigem resolução autorizada. A última gravação não sobrescreve silenciosamente o trabalho anterior.
8. Sair de uma edição preserva o rascunho ou pede decisão sobre o conteúdo não salvo. Não se exige confirmação repetida em ações sem risco relevante.
9. Operações de aprovação, assinatura, exclusão, estorno ou alteração financeira oferecem revisão, correção ou reversibilidade adequada à natureza do ato. [WCAG 2.2, prevenção de erros legais, financeiros e de dados](https://www.w3.org/WAI/WCAG22/Understanding/error-prevention-legal-financial-data).

### Revisão antes de um ato relevante

A confirmação de uma medição ou alteração contratual apresenta, na mesma revisão:

- Organização, obra, contrato, período e versão.
- Autor, responsável pela conferência e autoridade que decide.
- Valor anterior, diferença, novo valor, retenções e saldo quando aplicáveis.
- Efeito sobre o prazo, o planejamento e as etapas seguintes.
- Evidências e documentos que sustentam a decisão.
- Inconsistências que impedem concluir e orientação para corrigi-las.

O botão final descreve o ato, como **Aprovar medição de R$ 18.200,00**. A confirmação de sucesso somente aparece depois da transação confirmada. Repetir um clique ou uma requisição idêntica não duplica o ato. A mesma chave de operação com conteúdo diferente apresenta conflito e preserva as entradas, sem informar o sucesso de uma decisão anterior como se fosse a nova.

## 6. Planilhas, documentos e operações extensas

Orçamentos e medições precisam de uso profissional por teclado, seleção e cópia de dados, totais compreensíveis e informação de unidade. Colar várias linhas passa pela mesma validação de uma importação. Uma célula não pode converter texto inválido em zero sem avisar.

A importação apresenta prévia, campos reconhecidos, linhas válidas, rejeições e efeito esperado. O usuário deve conseguir corrigir e reenviar sem duplicar registros. Valores truncados, linhas ignoradas e arredondamentos precisam aparecer no relatório de resultado.

Tabelas grandes usam busca, filtros e paginação no servidor. Um contrato com 10 mil serviços não exige criar 10 mil componentes ativos de uma vez. Se houver virtualização, a leitura assistida, o foco e a seleção devem funcionar; quando isso não puder ser garantido, disponibilizar a tabela paginada equivalente.

Relatórios, importações e processamento de anexos grandes são tarefas acompanháveis. A resposta inicial identifica o pedido; progresso, falha e resultado continuam disponíveis depois de sair da tela. Retentar não duplica processamento ou lançamento. O relatório exportado informa versão, filtros, período, total e data de emissão.

## 7. Celular e uso em campo

O percurso de consultar documento, anexar foto e registrar ocorrência deve funcionar em 360 CSS px, sem rolagem horizontal fora de conteúdo técnico que efetivamente necessite de duas dimensões. Fotos são reduzidas e enviadas com estado visível, preservando o original quando a política de evidência exigir.

Funções que usam arraste também oferecem uma forma sem arrastar. Isso se aplica a ordenar itens, mover atividades e selecionar intervalos. O teclado não deve ser a única alternativa disponível ao usuário de toque. [W3C, novos critérios da WCAG 2.2](https://www.w3.org/WAI/standards-guidelines/wcag/new-in-22/).

A issue 62 entrega uso com conexão intermitente em uma sessão ou aba previamente carregada. Rascunhos ficam persistidos em IndexedDB; depois de fechar e reabrir a aplicação, a recuperação requer conexão e nova verificação de acesso. A versão 1.0 não oferece partida a frio sem rede. A preparação de campo informa essa condição antes do deslocamento.

Dados locais são limitados ao conjunto autorizado. A autorização local vence em até 8 horas desde a última verificação do servidor, prazo reduzível pela organização. Expirada, exigir rede para nova consulta/sincronização. Rascunhos não enviados permanecem no máximo 7 dias, com aviso e recuperação controlada. Não há remoção remota instantânea de conteúdo em dispositivo sem conexão. Sair da conta ou trocar de organização limpa ou protege o conteúdo conforme a política. A revogação remota é aplicada na próxima conexão ou na expiração da autorização local, e impede a sincronização indevida. Conflitos preservam o autor original e os estados de envio. Aprovações financeiras e atos que dependem de saldo atualizado exigem conexão e validação do servidor.

## 8. Acessibilidade

A meta técnica do produto é WCAG 2.2 nível AA nas páginas públicas e nas jornadas autenticadas, com atendimento de todos os critérios A e AA aplicáveis. A Lei Brasileira de Inclusão exige acessibilidade nos sítios empresariais abrangidos pelo art. 63; o critério técnico aqui adotado precisa ser testado, não presumido a partir da biblioteca visual. [LBI, art. 63](https://www2.camara.leg.br/legin/fed/lei/2015/lei-13146-6-julho-2015-781174-publicacaooriginal-147468-pl.html/), [WCAG 2.2](https://www.w3.org/TR/wcag/).

São obrigatórios:

- Operação por teclado, ordem de foco previsível e foco nunca oculto por elementos fixos.
- Rótulos, relações, estados e mensagens compreensíveis por leitor de tela.
- Contraste, ampliação, reajuste de conteúdo e ausência de dependência exclusiva de cor.
- Reajuste de conteúdo a 320 CSS px, incluindo ensaio equivalente com janela inicial de 1280 CSS px e zoom de 400%. Conteúdo técnico que efetivamente exige duas dimensões pode usar área própria, com exceção justificada; a exceção não se estende ao restante do formulário ou da página.
- Alternativa a gestos complexos e arraste.
- Ajuda consistente, preservação de dados já informados e erros acionáveis.
- Autenticação que permita gerenciador de senhas e colagem, sem desafios cognitivos desnecessários.
- Documentos de saída com texto selecionável, títulos e ordem de leitura adequados ao formato; alternativa acessível quando o conteúdo técnico exigir.
- Indicação e canal de acessibilidade conforme a política publicada, sem usar um selo como substituto da verificação.

Ferramentas automáticas são parte da avaliação. O aceite exige teste manual das tarefas com teclado e leitor de tela, incluindo tabelas, anexos, mensagens, diálogos e etapas financeiras. O relatório relaciona todos os critérios A/AA, página, aplicabilidade, reprodução, correção e nova verificação. Falha A/AA não pode permanecer como atendida por não ter sido classificada como impeditiva. Os percursos de contratação e cobrança construídos após a issue 63 entram na verificação integrada da 70 e na conferência final da 72. [W3C, reajuste de conteúdo](https://www.w3.org/WAI/WCAG22/Understanding/reflow.html), [W3C, condições de conformidade](https://www.w3.org/WAI/WCAG22/Understanding/conformance).

## 9. Metas de desempenho

### 9.1 Interface

| Indicador | Limite de aceitação |
|---|---|
| LCP | Até 2,5 s no percentil 75, móvel e computador avaliados separadamente |
| INP | Até 200 ms no percentil 75, com amostra real suficiente depois da abertura |
| CLS | Até 0,1 no percentil 75 |
| Resposta visual local simples | Até 100 ms em seleção, expansão e indicação de entrada, sem simular confirmação do servidor |
| WebAssembly inicial | Até 1 MiB transferido com Brotli, por entrada principal |
| Interoperabilidade JavaScript inicial | Até 80 KiB transferidos com Brotli; sem lógica própria de aplicação |
| CSS inicial | Até 60 KiB transferidos com Brotli |

LCP, INP e CLS adotam os limiares de boa experiência publicados pelo Google. Os limites de pacotes e resposta local são decisões específicas do Canteiro 360. [Métricas](https://developers.google.com/search/docs/appearance/core-web-vitals), [limiares e percentil](https://web.dev/articles/defining-core-web-vitals-thresholds).

Os limites de transferência incluem o conjunto inicial exigido pela rota, não apenas o maior arquivo. Fontes, imagens e dependências adicionais também são registrados. O código necessário a uma funcionalidade posterior não deve bloquear a tarefa atual. Exceção de orçamento não pode ser escondida por exclusão de arquivo ou troca de ferramenta de medição.

### 9.2 Servidor

| Operação | Limite de aceitação |
|---|---|
| Leitura autenticada simples | Percentil 95 de até 500 ms |
| Gravação simples | Percentil 95 de até 800 ms, com transação e auditoria confirmadas |
| Pedido de relatório extenso | Confirmação e identificador em até 1 s |
| Relatório de contrato com 10 mil serviços | Conclusão em até 60 s no cenário de referência |
| Isolamento sob carga | Nenhum dado ou efeito atravessa a organização autorizada |

Cálculos extensos e processamento em lote possuem métricas próprias e não são usados para maquiar as operações simples. Erros, tempo de espera, cancelamento e tarefas não concluídas entram no relatório. Uma resposta rápida de erro não conta como uma operação bem-sucedida.

### 9.3 Protocolo reproduzível

**Cliente laboratorial:** Chromium estável com versão fixada, janela de 1366 por 768 CSS px para computador e 360 por 800 CSS px para móvel; redução de CPU de quatro vezes; rede de 4 Mbps de descida, 1 Mbps de subida e 150 ms de latência de ida e volta. O executor tem 4 vCPUs e 8 GiB de RAM, sem concorrência externa. Sistema operacional, CPU física, navegador e ferramenta ficam registrados para permitir comparação.

**Cliente físico:** um aparelho Android intermediário com no mínimo 6 GiB de RAM, escolhido e identificado na issue 01, e um computador de referência. Modelo, processador, memória, sistema, navegador e condições de energia são fixados no registro de ensaio. O aparelho físico valida uso real; a simulação laboratorial fornece repetibilidade.

**Servidor:** aplicação com 4 vCPUs e 8 GiB de RAM; PostgreSQL em instância com 4 vCPUs e 8 GiB, armazenamento SSD; objeto de teste no mesmo local de hospedagem. Tipo de CPU, armazenamento, limites, versão do banco, latência e custo mensal do conjunto são registrados. Não é permitido aumentar recursos entre medições sem registrar o novo cenário e seu custo.

**Volume:** 50 usuários concorrentes distribuídos por 50 organizações, cada organização com 10 obras; contrato de teste com 10 mil serviços; 100 mil registros de medição no conjunto. A mistura contém 70% de leitura, 20% de gravações simples e 10% de consultas de tarefas extensas, com pedidos de relatórios limitados e documentados para não simular uma carga irreal. Cada perfil de acesso relevante participa do ensaio.

**Execução:** registrar aquecimento de 5 minutos e janela estável de 30 minutos no servidor. Para interface, executar pelo menos 20 navegações por rota e perfil de dispositivo, separando cache vazio e aquecido. As rotas mínimas são entrada, carteira, obra, orçamento, contrato, medição e financeiro. Publicar conjunto de dados, configuração, resultados, falhas e percentis.

Antes de existir tráfego suficiente, o aceite usa esses ensaios laboratoriais e interações controladas equivalentes. A telemetria real deve estar pronta para conferir LCP, INP e CLS após a abertura. Um resultado de laboratório não será descrito como percentil de visitantes reais; uma nota de ferramenta não substitui a medição de INP.

## 10. Validação com usuários

A issue 63 valida a interface integrada; a issue 71 confirma o uso profissional em duas empresas. O teste de experiência reúne pelo menos cinco pessoas dos papéis relevantes, incluindo duas sem familiaridade com SICOP. A amostra é suficiente para descobrir falhas de uso e aplicar o critério interno, sem representar pesquisa estatística de todo o mercado.

As seis tarefas são:

1. Criar uma obra e importar seu orçamento, identificando uma linha inválida.
2. Localizar o contrato, conferir condições e preparar a autorização de início.
3. Registrar execução e anexar evidência pelo celular.
4. Conferir uma revisão de projeto e identificar a versão que pode ser usada na obra.
5. Preparar e revisar uma medição parcial, com glosa e alteração contratual vigente.
6. Registrar fatura, pagamento parcial e retenção, explicar o saldo e exportar a evidência.

O roteiro entrega objetivos, sem instruir o caminho pela interface. Registrar sucesso, ajuda solicitada, tempo, erros, compreensão de valores e observações. A meta é **pelo menos 90% de conclusão sem assistência nas 30 execuções**, considerando cinco participantes e seis tarefas, e **nenhum erro financeiro irreversível**. Cada tarefa individual deve ser concluída sem assistência por pelo menos quatro participantes; um bom resultado agregado não pode ocultar uma tarefa inutilizável.

A pessoa deve compreender corretamente a diferença entre valor contratado, medido, faturado e recebido. Quando um erro de uso revelar um risco de saldo, documento, aprovação ou acesso, o lançamento fica bloqueado mesmo que a taxa agregada de sucesso seja suficiente.

## 11. Critério de entrega

Cada issue que inclua tela deve entregar seus estados, regra, orientação de correção, comportamento móvel, teclado, prevenção de erro e evidência de teste. A revisão final utiliza a versão candidata realmente publicada em homologação, com dados coerentes e permissões ativas.

Falhas corrigidas precisam ser novamente verificadas nas jornadas afetadas. Uma alteração em dinheiro, autorização, documento aprovado ou sincronização também repete os controles desses elementos. As evidências entram no conjunto de liberação descrito em [Prontidão comercial](prontidao-comercial.md).
