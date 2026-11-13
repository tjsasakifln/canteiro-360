# Verificação da issue 5

## 1. Objeto e versão

Este registro acompanha a [PR 77](https://github.com/tjsasakifln/canteiro-360/pull/77) e a [issue 5](../issues/005.md). O objeto é o motor compartilhado de precisão decimal, unidades e calendários, sua aplicação às prévias e a comparação entre execução nativa e WASM real. O [guia de uso](../calculos-calendarios.md) descreve as funções, os contratos HTTP e os limites.

| Referência | Identificação |
|---|---|
| Base | PR 76, `feat/issue-4-formularios-tabelas` |
| Commit da base | `a7f3b1ef1ab4bd654d2ce047b7bc4eb991488670` |
| Fonte de implementação conferida | `9c4c9b92aeed975695220734c2ea5432512ff3c9` |
| Árvore da implementação | `f3b9fb823304f814baebe044f66e950420a02f40` |
| Fonte anterior, com erro nas esperas do ensaio | `8f658c8f01560f1d6d176fc64066d3d51d8f6fa5` |
| Política numérica | `C360-DECIMAL-1` |
| Referência independente | [74 casos versionados](issue-005/reference-vectors.json) |
| SHA-256 da referência | `6c7f315a565d0d459cd5ca44f4e9ccc13d6b9dc5a4f6ddfaa6fbf69f83537bb5` |

**Validação técnica local concluída em 12/11/2026.** A fonte `9c4c9b92` passou em 99 testes nativos, 15 testes PostgreSQL e nos dois testes de navegador próprios desta etapa. Os 74 casos independentes coincidiram com os resultados nativos e com o WASM executado. As capturas finais foram examinadas após a correção dos campos sem carregamento. Os seis testes de navegador da fundação, planilha e protótipo passaram na fonte inicial, antes dos ajustes restritos à nova página e ao seu ensaio; a execução completa da revisão publicada é identificada na PR 77.

A implementação permanece dependente do aceite e da integração das issues 3 e 4. O teste da issue 3 com pelo menos três profissionais reais não é substituído por estes ensaios automatizados. Esta entrega não realiza merge nem fecha a issue 5.

O checkout local foi reconstruído a partir dos blobs da base, sem transportar o histórico Git completo. Seu identificador local não é apresentado como commit remoto. A [comparação com os blobs remotos](issue-005/local/source-correspondence.json) conferiu 82 arquivos de código, ferramentas, estilos, configuração e referência: todos coincidem com a fonte `9c4c9b92`. O identificador Git de cada blob é calculado a partir dos bytes locais e comparado com o retornado pela árvore remota. O registro inicial é preservado separadamente.

O [manifesto](issue-005/manifest.json) associa os arquivos preservados aos respectivos SHA-256, comandos e revisões. O commit de consolidação acrescenta documentação e evidências à fonte acima, sem alterar a aplicação, suas dependências ou seus testes.

## 2. Entrega e limites

O domínio implementa operações decimais exatas, precisão e escala explícitas, limites por grandeza e serialização textual. Produtos intermediários usam inteiros exatos temporários; nenhum valor financeiro é convertido para ponto flutuante. O resultado precisa caber na política, ou a operação falha. Tipos de entrada possuem desserialização validada; resultados derivados do domínio não aceitam desserialização.

A política contém preço com BDI e desconto, medição por diferença de acumulados, medição global, reajuste e rateios por maiores restos. Quantidades associadas à unidade só podem ser somadas ou subtraídas quando as unidades coincidem. A prévia da planilha passou a usar esse motor, removendo sua multiplicação monetária própria.

Calendários civis e de dias fixos usam a âncora original, conservam o término formal e identificam períodos residuais. Pausas são apuradas por frente e não modificam o prazo aprovado. Conversões de fuso recusam horas inexistentes ou ambíguas; offsets históricos com segundos são recusados na apresentação RFC3339 quando não podem ser representados sem alterar o instante.

A rota `/conferencia` oferece rateio em parcelas iguais e cronograma mensal, com interpretação brasileira, política visível, residual identificado e preservação das entradas inválidas. As duas prévias HTTP recalculam no servidor, exigem a versão da política e recebem somente entradas limitadas. São funções sem gravação comercial: não aprovam medições, não reservam saldo, não alteram contratos e não geram pagamentos.

Não há nova migração ou evento de auditoria comercial nesta etapa, pois não foi acrescentada mutação persistente. Autorização, organização, versão, idempotência e bloqueio transacional devem acompanhar os módulos que consolidarem os resultados. O [mapeamento dos 30 vetores originais](../calculos-calendarios.md#10-cobertura-dos-vetores-originais-e-consumidores-futuros) identifica os consumidores e a homologação da issue 70, sem declarar concluídos fluxos futuros.

## 3. Critérios próprios da issue

| Critério | Resultado exigido | Evidência |
|---|---|---|
| Medição por acumulados | PU `0.335`, acumulados 1, 2 e 3, parcelas `0.33`, `0.34`, `0.33`, total `1.00` | Aprovado. Testes de domínio, contratos e casos D025 e P-TEL |
| Conservação no rateio | `100.00` em três pesos iguais produz `33.34`, `33.33`, `33.33`; percentuais fecham `100.00` | Aprovado. D029, D033, P-ALLOC, API e prévia visual |
| Calendários e pausa | 31/01/2027 avança para 28/02 e 31/03; em 2028 usa 29/02; pausa de 10 a 13 soma três dias | Aprovado. D035 a D046, P-CAL, domínio e prévia visual |
| Igualdade nativa/WASM e falha explícita | Mesmos 74 resultados, representação textual idêntica e erros em entradas fora da faixa | Aprovado. [Relatório de paridade](issue-005/local/calculation-parity.json); negativos D012 a D016 e testes de limites |

Os 74 casos são 50 vetores determinados e 24 amostras de propriedades. Seus valores esperados foram calculados fora do produto com `decimal.Decimal` de precisão 120, frações exatas e bibliotecas de calendário e fusos. No ensaio, o WASM recebe somente identificador, operação e entradas. O teste externo compara o retorno com a referência e com o Rust nativo. Uma regressão específica confirma que trocar o campo de resultado esperado não altera a resposta do adaptador.

As propriedades nativas incluem 136 rateios gerados, 500 medições em cinco cadeias de 100 e 512 combinações de cronograma. Conservação, determinismo e monotonicidade são exigidos no alcance matemático correto: não se exige que cada parcela de maiores restos seja individualmente monotônica quando o total distribuído muda.

## 4. Verificações executadas

| Verificação | Resultado observado |
|---|---|
| `cargo test --locked --workspace` | 99 testes aprovados, nenhum reprovado; 23 testes externos ignorados somente neste comando |
| PostgreSQL real, execução explícita dos ignorados | 15 testes aprovados |
| Migrações e identidade de execução restrita | Aprovadas |
| Clippy nativo, todos os alvos | Workspace aprovado na fonte inicial; web e E2E novamente aprovados após as correções, com avisos tratados como erro |
| Clippy do front-end WASM | Aprovado com avisos tratados como erro |
| `cargo deny --locked check` | Dependências, origens, licenças e avisos de segurança aprovados; avisos não bloqueantes permanecem no log |
| Formatação, regras do repositório e verificação de espaços das fontes e documentos | Aprovadas; logs brutos excluídos somente da verificação de espaços |
| Compilação otimizada do servidor e aplicação WASM | Aprovada |
| Compilação do verificador WASM separado | Aprovada |
| Dois testes reais de navegador da issue 5 | Aprovados na fonte final, incluindo os 74 casos e o percurso visual completo |
| Seis testes reais de navegador das etapas anteriores | Aprovados na fonte inicial; resultados preservados em `initial/`; não apresentados como uma execução única com o reteste final |

Os 74 casos independentes são verificações internas do conjunto de testes. Não foram somados novamente à contagem de 99 funções de teste. Os 23 ignorados correspondem a 15 testes PostgreSQL e oito de navegador. Os 15 de banco e os dois novos cenários foram executados explicitamente na fonte final. A CI conserva o comando que executa os oito testes de navegador juntos, sem ignorar uma falha parcial.

Os logs de [testes nativos](issue-005/local/native.log), [PostgreSQL](issue-005/local/postgresql.log), [navegador da conferência](issue-005/local/browser-calculations.log), [migrações](issue-005/local/migrations.log), [Clippy nativo inicial](issue-005/local/clippy-native.log), [Clippy dos componentes corrigidos](issue-005/local/clippy-corrected.log), [Clippy WASM](issue-005/local/clippy-wasm.log), [dependências](issue-005/local/dependencies.log) e [regras](issue-005/local/policy.log) preservam os resultados locais. Migrações e dependências não foram alteradas depois de sua aprovação. A conclusão da CI da revisão publicada é registrada separadamente na PR 77, vinculada ao commit executado.

Os logs mantêm os espaços e as linhas finais emitidos pelas ferramentas, inclusive a diagramação do cargo-deny. Por isso, a verificação de espaços exclui apenas os `.log` das pastas `initial/` e `local/`; o conteúdo dessas evidências é preservado byte a byte e conferido pelo manifesto.

## 5. Interface e recursos servidos

O [relatório da interface](issue-005/local/calculation-interface.json) confirma larguras de 360 e 1280 CSS px sem vazamento horizontal da página, rótulos associados e altura mínima de 44 CSS px nos campos. As entradas `100,01`, `1.234`, quantidade zero e data inexistente exercitam atualização, erro e recuperação. O calendário conserva o término enquanto a data inicial é corrigida. A mudança explícita entre meses civis e trinta dias produz o resultado esperado no ano bissexto.

As imagens de [computador](issue-005/local/calculation-1280.png), [entrada inválida preservada](issue-005/local/calculation-invalid-preserved.png) e [falha de carregamento](issue-005/local/calculation-wasm-unavailable.png) foram conferidas. No celular, as capturas da [tabela de parcelas](issue-005/local/calculation-allocation-visible-360.png), do [calendário](issue-005/local/calculation-calendar-visible-360.png) e da [rolagem horizontal](issue-005/local/calculation-calendar-scrolled-360.png) mostram o conteúdo dentro da janela real, com foco visível. O teste usa Tab e roda nativos para enquadrar as tabelas; a seta do teclado desloca seu conteúdo horizontalmente.

A captura longa de uma página pode não pintar uma superfície rolável fora da janela. Por isso, a aprovação visual das tabelas usa também as capturas de janela após navegação real, sem reconstruir ou editar a imagem. Não foi identificada falha de apresentação do calendário nas capturas em que a tabela estava visível.

O bloqueio proposital do WASM mantém os quatro valores iniciais nos campos, conserva os exemplos calculados e oferece recarga. O erro de rede correspondente fica separado no relatório; o console é vazio no percurso normal e na comparação numérica. Nenhum desses resultados substitui a avaliação humana exigida pela issue 3.

O orçamento foi medido nos artefatos destinados à aplicação, antes da inclusão do módulo de verificação exclusivo de testes. O módulo não integra a aplicação distribuída nem é usado para produzir os resultados esperados. Compilar esses artefatos não significa publicar uma versão em produção.

| Recurso | Medida local | Limite |
|---|---:|---:|
| WASM Brotli | 654.860 B | 1.048.576 B |
| JavaScript externo Brotli mais inline bruto da rota `/` | 8.636 B | 81.920 B |
| CSS Brotli | 7.028 B | 61.440 B |

Esses números se referem ao orçamento definido pelo projeto para a rota `/`. Não representam medida de rede, tempo percebido por usuários ou orçamento individual de todas as rotas.

## 6. Revisão independente e correções

O motor decimal, o calendário e a referência independente foram preparados por especialistas separados. A revisão cruzada examinou o domínio, os contratos, a tela e o caminho do teste. A conferência final associa o resultado à fonte efetivamente publicada.

| Achado examinado | Correção ou decisão | Verificação |
|---|---|---|
| Offsets históricos com segundos poderiam ser formatados com perda de precisão e representar outro instante | Erro `SubminuteOffset` na conversão de saída RFC3339; conversão local para UTC permanece exata | Regressões de São Paulo, Paris e Tóquio, conferidas por revisor independente |
| Resultado intermediário extenso poderia ser confundido com estouro do resultado final | Inteiro temporário exato, normalização e validação dos limites no resultado representável | Caso com coeficiente temporário de 55 dígitos e resultado exato admitido, além dos casos de estouro real |
| Reaplicar um preço diferente ao histórico poderia reescrever o valor reconhecido | Operação com valor anterior reconhecido recusa divergência e exige segmento econômico coerente | Teste de `InconsistentPrevious` |
| Formatação de apresentação recusava valores negativos ou com mais de seis casas | Formatação usa o decimal exato comum e conserva o sinal e a precisão informada | Regressão de apresentação |
| Tela nova não herdava os estilos dos campos | Contêiner comum aplicado e áreas mínimas preservadas | Revisão de CSS e ensaio de dimensões |
| Retorno da conferência apontava para uma empresa específica | Retorno para a página inicial | Inspeção de rota e captura da tela |
| Falha de WASM deixava campos desabilitados sem saída | Mensagem de preparo, ajuda após espera e link de recarga; exemplos SSR continuam legíveis | Ensaio com bloqueio real do arquivo WASM |
| Referência poderia ser incorporada ao cálculo sob teste | Adaptador recebe somente entradas, sem resultados esperados; teste contra uso indevido do campo esperado | Comparação nativa e execução em WASM |
| Quatro esperas do novo ensaio omitiam `return`, fazendo o WebDriver receber `null` mesmo com a condição atendida | As expressões agora devolvem o booleano; prazos e critérios foram preservados | Diagnóstico real confirmou edição pronta, console vazio e ajuda visível; reteste dos dois cenários afetados |
| O aviso de preparo deixava uma barra vazia depois da hidratação | O contêiner inteiro fica oculto quando a página está pronta | Captura anterior preservada e revisão da captura após a correção |
| Sem WASM, os campos iniciais ficavam vazios embora as tabelas SSR mostrassem as prévias | Atributos `value` são enviados pelo servidor além das propriedades reativas | As quatro propriedades foram lidas com WASM bloqueado; captura final conferida por dois revisores |
| Captura longa não bastava para conferir uma tabela fora da janela | Capturas de janela após Tab e roda nativos, com verificação de geometria, conteúdo e rolagem horizontal por teclado | Tabelas visíveis nas duas larguras; a preparação não altera o DOM ou o resultado calculado |

A reprodução principal da revisão numérica é feita pelos testes e pelos 74 casos incluídos no repositório. As conclusões de leitura e os ensaios exploratórios não são somados às contagens de testes versionados.

## 7. Ambiente e reprodutibilidade

A execução local usa Rust/Cargo 1.99.1, wasm-bindgen 0.2.130 e cargo-deny 0.20.2. O banco local é PostgreSQL 17.11; a CI conserva PostgreSQL 17.7 fixado no workflow. Somente dados fictícios foram usados. A aplicação executa com a identidade restrita, distinta da identidade de migração e do administrador de preparação dos testes.

O Chrome Headless Shell e o ChromeDriver locais são 156.0.8078.80. O Chrome comum não conseguiu criar seu socket Unix no ambiente local. A troca para a distribuição headless oficial permitiu executar o mesmo protocolo WebDriver sem alterar as restrições do ambiente ou relaxar os critérios. A CI conserva Chrome e ChromeDriver 156.0.8078.80 nas distribuições já fixadas no projeto.

Uma tentativa inicial de PostgreSQL encontrou o processo de banco encerrado entre comandos do executor; foi repetida com o banco e o teste na mesma sessão. A primeira tentativa de navegador falhou antes da criação da sessão por restrição de socket. Esses resultados não são evidência de aprovação nem foram atribuídos a defeitos do produto. Os registros selecionados nesta consolidação são os das execuções concluídas e identificadas.

Na primeira execução completa em headless, a fonte `8f658c8f` aprovou seis testes de navegador: dois da fundação, três da planilha e um do protótipo. Os dois novos testes falharam nas esperas sem `return`, conforme o [log original](issue-005/initial/browser.log). Um diagnóstico separado observou [página pronta e console vazio](issue-005/initial/diagnostic-normal.json) e [ajuda visível com WASM bloqueado](issue-005/initial/diagnostic-blocked.json). O erro de carregamento nesse último console é consequência do bloqueio proposital. A falha do ensaio foi corrigida sem aumentar os prazos ou retirar as verificações.

A fonte intermediária `983087cc` aprovou os [dois testes depois da correção das esperas](issue-005/initial/browser-return-corrected.log). A revisão visual ainda encontrou [campos sem valores no modo bloqueado](issue-005/initial/wasm-unavailable-before-values.png). Ao acrescentar capturas dentro da janela, uma [tentativa de enquadramento por foco apenas](issue-005/initial/browser-viewport-focus.log) não mostrou a tabela inteira no computador. O teste passou a centralizá-la com roda nativa, mantendo a exigência de que ela caiba integralmente na janela. A fonte final preserva os valores SSR e passou no ensaio ampliado; nenhum resultado anterior reprovado foi reclassificado como aprovado.

O guia contém os comandos de reprodução nativa e de navegador. A compilação WASM isolada não é tratada como prova de execução no navegador. Perfis móveis dos ensaios são emulações; não substituem aparelhos reais, leitores de tela ou o teste profissional exigido pela issue 3.

## 8. Situação de integração

O encerramento da issue depende do aceite próprio e da sequência do projeto. A PR 77 tem a PR 76 como base; a PR 76 depende da PR 75. Enquanto o aceite com três profissionais da issue 3 estiver pendente, a integração desta etapa permanece bloqueada. Os quatro critérios técnicos próprios foram atendidos nos ensaios registrados, mantendo a issue aberta até sua integração regular.

A fundação aqui verificada não declara aprovados faturamento, recebimento, concorrência financeira de módulos futuros ou prontidão comercial da versão publicada.
