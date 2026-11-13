# Formulários e tabelas da issue 4

## Como experimentar

Com a aplicação de desenvolvimento iniciada conforme [Desenvolvimento](desenvolvimento.md), abra `/prototipo/aurora/planilha` ou `/prototipo/horizonte/planilha`. A navegação da obra também oferece **Planilha de medição**.

A grade apresenta uma página da base fictícia de 10.000 serviços da empresa selecionada. Busca, ordenação e paginação são executadas no servidor. O HTML inicial contém os serviços da página solicitada e os links de navegação, inclusive com scripts desativados. A edição depende da interação já carregada.

1. Busque por código ou descrição, escolha a ordenação e use **Consultar serviços**. O tamanho da página pode ser 25, 50 ou 100.
2. Use **Editar rascunho**, junto à identificação e à unidade do serviço. Preencha o acumulado atual, a unidade, o preço demonstrativo e a data.
3. Confira a interpretação abaixo de cada campo. Os números aceitam vírgula decimal e agrupamento brasileiro, como `1.234,56`. Um ponto isolado, como `1.234`, exige correção; o sistema não decide silenciosamente entre milhar e decimal.
4. Caso simule um preço diferente, explique o motivo nas observações. O percentual é um campo demonstrativo separado e não altera os valores da prévia.
5. Use **Conferir valores e diferenças**. A conferência do servidor apresenta o antes e depois; erros conservam as entradas e direcionam o foco ao campo a corrigir.
6. Use **Guardar rascunho nesta aba**. A revisão local, o autor demonstrativo e o horário são identificados. Voltar à grade conserva os textos. A grade permanece como referência e não incorpora a edição local.

Rascunhos são voláteis e restritos à aba. Navegar entre tarefas da mesma empresa os conserva. Recarregar ou fechar a aba pode perdê-los, com aviso nativo quando há edição. Trocar de empresa exige decisão e descarta os rascunhos do contexto anterior; a regra também cobre o histórico do navegador. Essa etapa não oferece armazenamento de rascunhos no banco, autenticação nem operações comerciais. O armazenamento local para conexão intermitente pertence à issue 62.

## Componentes reutilizáveis

| Componente | Responsabilidade |
|---|---|
| `LabeledTextInput` | Controle público independente do domínio e do estado de medição, com rótulo, obrigatoriedade, valor original, interpretação, erro associado e callbacks. `ValidatedField` adapta esse controle ao rascunho da medição. |
| `ValueComparison` | Painel público de valores identificados. `BeforeAfter` adapta acumulados, valor do período e saldo, com unidades explícitas. |
| `PaginationControls` | Links públicos de página anterior e próxima, independentes do formato dos registros. |
| `WorksheetState` e `RowDraft` | Conservam entradas por serviço, distinguem edição de cópia local e invalidam a conferência quando um campo muda. |
| `MeasurementTable` | Tabela sem virtualização, página limitada, identidade do serviço persistente, seleção explícita e navegação acessível. |
| Contratos em `canteiro-contracts::forms` | Interpretação pt-BR, tipos públicos e cálculo exato limitado às prévias fictícias. |

O controle de entrada não substitui a validação do servidor. Dinheiro, quantidade e percentual trafegam como texto decimal, sem decisão financeira em ponto flutuante. A política completa de precisão e calendário será implementada na issue 5; a prévia desta etapa já respeita o cálculo por diferença de valores acumulados truncados em centavos.

## Consulta e seleção

Os cabeçalhos distinguem contratado, anterior acumulado, atual acumulado, diferença no período, preço unitário, valor do período e saldos de quantidade e dinheiro. De 20 para 35 m² são 15 m² no período. Com preço 10, o valor é R$ 150,00. Com preço 0,335, de 1 para 2 unidades o período vale R$ 0,34, pela diferença de acumulados.

Selecionar a página nunca seleciona outros resultados. **Selecionar os resultados da busca** exige uma ação separada, identifica a quantidade completa e oferece o arquivo TSV. Alterar a consulta limpa a seleção e preserva os rascunhos. A cópia da página oferece um campo de texto selecionável, sem depender de autorização para a área de transferência. Cópia e exportação usam a base fictícia de referência; os rascunhos locais não são apresentados como dados gravados.

Na tela estreita, a grade tem sua própria rolagem horizontal. A identificação, a unidade e a ação de edição permanecem disponíveis. Os demais elementos se reorganizam para a largura da página. Tab, Enter, Espaço, Escape e os controles nativos funcionam sem exigir arraste.

## Contrato HTTP da demonstração

| Método e caminho | Resultado |
|---|---|
| `GET /api/prototipo/{organization}/medicoes` | Página com até 100 serviços. Parâmetros: `page`, `page_size`, `search`, `sort`, `direction`. |
| `GET /api/prototipo/{organization}/medicoes/export` | TSV de todos os resultados da busca, com os valores de referência e até 10.000 serviços. |
| `POST /api/prototipo/{organization}/medicoes/preview` | Prévia validada ou inconsistências por campo, sem gravação. |

`organization` admite somente `aurora` ou `horizonte`, ambas fictícias. A busca admite até 80 caracteres e as observações até 1.000. Consulta inválida não é corrigida silenciosamente para outro filtro ou página. O corpo da prévia é limitado a 16 KiB. Respostas usam `no-store`. Unidade incompatível, acumulado menor que o anterior, excesso do contratado, data inexistente, percentual fora de 0 a 100, entrada ambígua e precisão excedida são recusados.

O cliente preserva entradas em falhas de leitura ou conferência. Respostas atrasadas não podem confirmar um rascunho modificado ou transportar dados de outra empresa. A consulta mostra espera e recuperação; a confirmação local só é habilitada para a versão efetivamente conferida.

## Dependência e aceite

A implementação parte da PR 75, correspondente à issue 3. O aceite dessa etapa exige teste com pelo menos três profissionais, ainda pendente no início desta implementação. A PR da issue 4 deve permanecer dependente dessa base, sem integração fora de ordem e sem encerrar a issue por simples compilação.

Os resultados executados, os limites da verificação e a versão conferida pertencem ao [registro da issue 4](validacao/ISSUE-004.md). A liberação comercial completa continua sujeita à issue 72.
