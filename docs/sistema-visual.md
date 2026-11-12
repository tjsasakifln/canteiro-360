# Sistema visual do Canteiro 360

## Escopo e estado

Base visual e percursos demonstrativos da [issue 3](https://github.com/tjsasakifln/canteiro-360/issues/3). Os componentes são escritos em Rust/Leptos; a apresentação compartilhada está em `style/prototype.css`, importada pela folha principal. Orçamento, medição, aprovação, documentos e painel usam dados fictícios para avaliar organização, leitura, interação e recuperação de erros.

Esta base não representa autenticação, concessão de acesso, assinatura, aprovação financeira ou gravação comercial. Uma ação demonstrativa deve ser identificada como simulação e informar seu efeito local. Confirmação de persistência real pertence à implementação transacional do respectivo módulo, após autorização e confirmação no servidor.

A aprovação definitiva dos componentes depende do teste inicial com pelo menos três profissionais exigido pela issue 3. O [roteiro](validacao/issue-003/roteiro-profissionais.md) está preparado; um roteiro vazio não constitui evidência de execução. Os registros de compilação, navegador e revisão da versão candidata devem acompanhar a issue, distinguindo o que foi medido do que permanece pendente.

## Regras de uso

1. A organização permanece identificável. Obra, contrato e versão aparecem no contexto da tarefa e na revisão de um ato relevante.
2. A navegação permite consultar seções sem impor uma ordem de menus. A dependência real bloqueia a conclusão do ato, com explicação, sem bloquear consultas independentes.
3. A tela mantém uma ação principal clara e um pequeno conjunto de ações secundárias. Detalhes aparecem por links, revisão ou expansão acessível.
4. Cores acompanham texto ou símbolo compreensível. Valores, pendências e diferenças precisam continuar legíveis sem distinguir cores.
5. Erros preservam entradas válidas, explicam a consequência e orientam a recuperação. Um protótipo não apresenta uma falha simulada como incidente do servidor.
6. Trocar de organização não transporta o rascunho para outra empresa. Saída de edição não descarta conteúdo sem decisão explícita ou mecanismo de preservação claramente informado.
7. O contexto demonstrativo nunca é usado como autorização. Os módulos posteriores devem revalidar identidade, vínculo, empresa, versão e estado no servidor.

## Tokens

As variáveis `--c360-*` centralizam decisões visuais. Não introduzir cores ou escalas divergentes dentro de um módulo sem atualizar este contrato e conferir as combinações utilizadas.

| Função | Token ou valor | Aplicação |
|---|---|---|
| Fundo | `--c360-canvas: #f7f9fc` | Área externa dos cartões |
| Superfície | `--c360-surface: #ffffff` | Cartões, campos e diálogos |
| Superfície discreta | `--c360-surface-subtle: #f1f5f9` | Cabeçalhos e estados neutros |
| Texto | `--c360-text: #172033` | Títulos, conteúdo e valores |
| Texto secundário | `--c360-muted: #475569` | Orientação e metadados |
| Ação principal | `--c360-primary: #1d4ed8` | Botão principal, foco e seleção |
| Ação sob ponteiro | `--c360-primary-hover: #1e40af` | Realce da ação disponível |
| Sucesso | `--c360-success: #166534` | Estado acompanhado de texto |
| Atenção | `--c360-warning: #92400e` | Pendência e orientação |
| Erro | `--c360-danger: #b91c1c` | Campo inválido e falha acionável |
| Campo | `--c360-input-border: #64748b` | Contorno identificável e controles secundários |
| Separação | `--c360-border: #cbd5e1` | Divisão visual não interativa |
| Tipografia | Fontes do sistema; corpo de 16 px; entrelinha 1,5 | Sem fonte remota necessária |
| Valores e quantidades | `font-variant-numeric: tabular-nums` | Alinhamento e comparação de números |
| Espaço | 4, 8, 12, 16, 20, 24, 32 e 40 px | Escala compartilhada |
| Foco | Contorno de 3 px, afastado 4 px | Não depende apenas de sombra |
| Toque | Mínimo de 44 × 44 CSS px nas ações principais | Preservado em densidade compacta |
| Cantos | 12 px em grupos; 6 px em controles | Hierarquia consistente |

As combinações de texto e fundo precisam ser verificadas nas telas renderizadas. A presença de um token de cor, de um atributo ARIA ou de uma regra CSS não comprova conformidade de acessibilidade por si só.

## Densidade e adaptação

`data-density="comfortable"` usa cartões com 24 px e linhas com 16 px de espaçamento. `data-density="compact"` reduz os espaços para 20 px e 12 px. Ambos conservam o corpo de texto, a ordem dos elementos e o tamanho mínimo das ações. A opção não remove conteúdo ou transforma um controle em alvo pequeno.

O contexto e os formulários passam a uma coluna quando necessário. A navegação quebra em linhas; não depende de uma faixa horizontal invisível. O conteúdo usa tamanhos flexíveis e permite quebra de identificadores longos. A interface deve ser ensaiada a 320, 360 e 1280 CSS px, além de ampliação equivalente a 400% a partir de 1280 px.

Os controles de densidade, cenário e ajuda ficam em uma expansão nativa fechada inicialmente. O contexto da obra permanece fora dessa expansão. Assim, as opções de demonstração não ocupam o percurso principal antes da ação da tarefa, mas continuam disponíveis por teclado e antes da hidratação para consulta da ajuda.

Tabelas técnicas mantêm a relação bidimensional entre serviço, unidade, quantidade e valor em uma área de rolagem própria, identificada e acessível por teclado. A exceção não se estende à página, ao contexto, ao formulário ou ao resumo. Não esconder transbordamento horizontal da página para mascarar conteúdo cortado.

## Componentes compartilhados

| Grupo | Classes principais | Contrato de comportamento |
|---|---|---|
| Contexto | `prototype-header`, `context-grid`, `context-item` | Organização, obra, contrato e versão legíveis; troca não herda rascunho |
| Navegação | `task-nav`, `task-link`, `step-list`, `step-link` | Links reais; estado atual identificado; etapas pendentes consultáveis quando permitido |
| Estrutura | `prototype-main`, `page-heading`, `section-card`, `two-column` | Título único da tarefa; ordem de leitura igual à ordem do conteúdo |
| Painel | `metric-grid`, `metric-card`, `metric-value` | Não somar contratado, medido e pago como custos independentes |
| Ações | `primary-button`, `secondary-button`, `text-link`, `page-actions` | Verbo explícito; consequência clara; bloqueio acompanhado de motivo |
| Formulário | `field`, `field-help`, `field-error`, `form-grid` | Rótulo associado, unidade, instrução e erro vinculado; entradas preservadas |
| Data | Campo nativo com rótulo e instrução | Leitura em convenção brasileira; valor inválido nunca vira data diferente silenciosamente |
| Tabela | `table-scroll`, `data-table`, `numeric`, `totals-panel` | Título/legenda, cabeçalhos associados, unidade e total inequívocos |
| Estados | `status-badge`, `notice`, `empty-state`, `prototype-readiness` | Texto informa estado, motivo e recuperação; carregamento não finge sucesso |
| Documento | `document-card` | Versão e situação explícitas; somente exemplos fictícios nesta etapa |
| Histórico | `history-list` | Autor, instante e efeito compreensíveis; não substituir auditoria do servidor |
| Comparação | `version-comparison` | Revisões identificadas; valor anterior, diferença e valor proposto distinguíveis |
| Aprovação | `prototype-dialog`, `dialog-actions` | Revisão antes do ato; foco contido; cancelamento e Escape sem confirmação |

Calendários, seletores e entradas de arquivo utilizam controles nativos onde atendem ao comportamento exigido. Customização futura não deve perder navegação por teclado, leitura assistida ou operação móvel. A aparência de uma ação indisponível não implementa sua regra: o componente Rust e o servidor do módulo correspondente devem aplicar a condição.

## Teclado, foco e movimento

- O link de salto leva ao conteúdo principal. Títulos e marcos permitem compreender a página antes de usar controles.
- O foco segue a ordem do documento. Navegação por Tab, Shift+Tab, Enter e Espaço não depende do ponteiro.
- Diálogos devem usar a API modal nativa, receber foco ao abrir, impedir interação com o fundo e devolver foco ao acionador ao sair. Escape cancela; o fechamento não confirma o ato.
- Atalhos específicos da aplicação devem estar anunciados junto à ajuda, evitar conflitos e respeitar campos em edição. Um botão alcançável por teclado continua disponível como alternativa.
- Erros associados ao campo usam texto explícito. A recuperação conduz ao campo ou resumo pertinente, sem reiniciar o formulário.
- `prefers-reduced-motion` elimina animações e deslocamentos desnecessários. Informações de recuperação do carregamento permanecem disponíveis.
- Modos de cores forçadas conservam bordas e foco. Impressão conserva texto e contexto, sem transformar a impressão do protótipo em documento comercial.

## Estados e fidelidade da demonstração

O seletor de apresentação permite examinar estado normal, vazio, carregando, erro e ausência de permissão. Esses cenários pertencem à demonstração. Ausência de permissão deve ocultar o conteúdo da tarefa e apresentar um caminho de retorno; não constitui autorização aplicada ao banco.

O conteúdo essencial e os links devem existir antes da hidratação. Controles que dependem da interação carregada informam essa condição e oferecem recuperação. A interface não mostra confirmação de gravação por simples clique, mudança de cor ou temporizador.

Os exemplos usam Escola Aurora, contrato CT-014, versão 2, e Galpão Horizonte, contrato CT-027, versão 1, em contextos fictícios distintos. Períodos, valores e responsáveis da demonstração não representam contratos da CONFENGE ou de clientes.

## Verificação e aceite

O registro da versão candidata deve apresentar:

1. Capturas do painel, orçamento, medição, aprovação e documentos em computador e celular, com versão testada identificada.
2. Percursos por teclado, cancelamento de diálogo, retorno de foco e verificação dos estados de erro, carregamento e ausência de permissão.
3. Provas de que troca de contexto e navegação não misturam rascunhos, e de que a dependência de comissão impede somente o ato pertinente.
4. Medições das combinações de contraste utilizadas, reflow e rolagem horizontal restrita às tabelas técnicas.
5. Compilação SSR/WASM e orçamento de transferência repetidos após a alteração. Limites: 1 MiB para WASM inicial, 80 KiB para interoperabilidade JS e 60 KiB para CSS, com Brotli.
6. Revisão adversarial independente e reteste dos defeitos corrigidos.
7. Teste inicial com pelo menos três profissionais, com identificação de ação, valor e estado, correção de problemas graves e registro do reteste.

As verificações técnicas não substituem participantes reais nem avaliação com leitor de tela. Não registrar como atendido um critério cujo teste não ocorreu. A aceitação completa da issue 3 permanece dependente das evidências previstas; a prontidão comercial continua na issue 72.

## Referências

- [Experiência e desempenho](experiencia.md), especialmente seções 3 a 8 e 11.
- [PRD](../PRD.md), jornada, responsabilidades e experiência.
- [Arquitetura Rust](arquitetura-rust.md), apresentação, contexto e limites do cliente.
- [OP06](regras/operacao.md#op06--adaptada--issue-03), navegação livre e dependência real da medição.
- [Matriz de rastreabilidade](MATRIZ-RASTREABILIDADE.md), OP06, UX01 e UX15.
