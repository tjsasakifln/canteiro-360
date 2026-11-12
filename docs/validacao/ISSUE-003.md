# Verificação da issue 3

## Objeto e limite do aceite

Implementação do sistema visual, navegação por tarefa e componentes demonstrativos da [issue 3](https://github.com/tjsasakifln/canteiro-360/issues/3). A dependência, issue 2, foi integrada pela PR 74 no commit `83c94e0938c09d7d30d9f97aad6de2840cfb50e6`, com CI completa aprovada.

Esta etapa não implementa autenticação, autorização comercial, medições reais, faturas, pagamentos nem aprovação persistida. Os exemplos usam duas empresas fictícias. A mudança do cenário de permissão demonstra a apresentação, sem conceder ou revogar acesso real.

**Aceite integral pendente:** a issue exige teste inicial com pelo menos três profissionais. O [roteiro de aplicação e registro](issue-003/roteiro-profissionais.md) foi preparado, mas não há participação humana registrada. Testes automáticos e revisão independente de código não substituem esse requisito. A issue 3 deve permanecer aberta até a evidência ser registrada e eventuais problemas graves serem corrigidos.

## Implementação verificável

| Critério | Implementação e cenário |
|---|---|
| Contexto persistente | Empresa na rota; obra, contrato e revisão em cada tarefa; recarga direta nas dez rotas das duas empresas |
| Navegação por tarefa | Painel, orçamento, medição, aprovação e documentos; links reais e marcação da tarefa atual |
| Etapas OP06 | Documentos acessíveis com cadastro pendente; conclusão da medição bloqueada por comissão ausente, com motivo explícito |
| Valores | Exemplos com centavos inteiros; orçamento completo separado do subtotal filtrado; contratado, medido, faturado e pago separados |
| Recuperação | Validação do rascunho conserva descrição e período; erro de consulta não apaga entradas |
| Troca de empresa | Decisão antes de descartar rascunho; dados da empresa anterior não aparecem na nova |
| Aprovação | Revisão de contexto, período, versão, bruto, retenção, líquido, responsáveis e consequência; resultado explicitamente simulado |
| Estados | Vazio, carregando com saída operável, erro recuperável e ausência de permissão sem dados de negócio no conteúdo da tarefa |
| Teclado | Navegação, filtro, retorno ao serviço, formulários, diálogos nativos, Escape, retorno de foco e atalho anunciado |
| Apresentação | Tokens CSS, densidade, tabelas em área própria, foco de 3 px, controles de toque, movimento reduzido e impressão |
| Rotas inválidas | Empresas e tarefas desconhecidas não recebem conteúdo de outra empresa nem resposta HTTP de sucesso |

## Execução técnica

**CI completa aprovada:** [execução 43142248739](https://github.com/tjsasakifln/canteiro-360/actions/runs/43142248739), sobre o código da revisão `61ed5b653f473e29eb90ec48d2230e17fa9298a5`. O checkout efetivo foi o merge temporário `5c8871c23a41589f47f4075764267a5ab54c6ec7`; sua árvore `fa495de7a66c1df9065e4564b717b20df9b76780` é idêntica à árvore da revisão da PR. A confirmação está no [registro da execução](issue-003/ci-43142248739/run.json) e no [log completo](issue-003/ci-43142248739/job.log).

Os registros locais de testes nativos, Clippy, compilação SSR/WASM e tamanho dos arquivos correspondem à implementação de interface da revisão `ab2685bea1c9c10c1b81e9bcbfdfe2b49b1e1632`. Depois dela, os ajustes de código ficaram restritos às esperas dos testes de navegador e aos prazos das conexões positivas de um teste de banco. Os 15 cenários locais de PostgreSQL foram repetidos após esse último ajuste. A CI aprovada executou novamente todas as etapas sobre a revisão final.

| Verificação local | Resultado e evidência |
|---|---|
| Testes nativos | 45 aprovados, zero falhas; [log](issue-003/native-tests.log). As 18 verificações que exigem banco/navegador não são executadas pelo comando nativo e têm etapas explícitas próprias. |
| PostgreSQL 17.7 real | 15 aprovados, zero falhas e zero ignorados na execução explícita; [log](issue-003/postgresql-tests.log). Instância e testes executados na mesma sessão isolada. |
| Clippy nativo e WASM | Sem avisos tratados como erro; [nativo](issue-003/clippy-native.log) e [WASM](issue-003/clippy-wasm.log). |
| Formatação e regras | Formatação, política de linguagens/segredos e consistência das 72 issues e 192 regras aprovadas. |
| Dependências | `cargo deny --locked check` aprovado. A exceção anterior de `paste 1.0.15` permanece restrita e documentada; nenhuma nova exceção foi criada. |
| Compilação SSR/WASM | Aprovada com Rust 1.99.1 e wasm-bindgen 0.2.130; [registro](issue-003/build-local.json). |
| Pacote servido | WASM 348.809 B, interoperabilidade JS 8.193 B, CSS 5.570 B, nos limites de 1 MiB, 80 KiB e 60 KiB; [medição local](issue-003/budget-local.json). Inclui as duas folhas CSS e o conteúdo inline. |

O executor local impede sockets Unix, necessários para iniciar o Chromium disponível. A prova de interação e as capturas foram obtidas pela CI com navegador real Chromium e ChromeDriver `156.0.8078.80`. A simulação de viewport não é apresentada como ensaio em aparelho físico, zoom manual de 400% ou teste com leitor de tela.

### Resultados da execução aprovada

| Verificação | Resultado e evidência |
|---|---|
| Testes nativos | 45 aprovados, zero falhas. As 18 verificações ignoradas neste comando foram executadas nas etapas explícitas de banco e navegador. |
| PostgreSQL 17.7 | 15 aprovados, zero falhas e zero ignorados; [log da CI](issue-003/ci-43142248739/persistence.log). Migrações e identidades distintas também aprovadas. |
| Navegador | Três testes aprovados: comportamento da fundação, desempenho da rota inicial e percurso completo do protótipo. [Protótipo](issue-003/ci-43142248739/prototype-functional.json) e [fundação](issue-003/ci-43142248739/browser-functional.json). |
| Contexto e recuperação | Cancelar conserva empresa, período e texto; confirmar descarta explicitamente; Voltar pelo histórico entre empresas exige decisão. Empresa anterior não permanece na nova tarefa. |
| Aprovação | Tab e Shift+Tab contidos, Escape e cancelamento restituem foco; confirmação leva ao resultado identificado como simulação. O painel deixa de apresentar a decisão simulada como pendente. |
| Apresentação | 15 combinações de cinco tarefas e três larguras, sem transbordamento horizontal da página, sem campos sem rótulo, com um título principal e uma navegação atual por tela. As tabelas conservam rolagem própria. |
| Contraste de texto | 22 relatórios, 453 combinações observadas somadas, todas aprovadas. Menor razão: 6,4700447145:1. Cores computadas de texto e fundo sólido; limites do método constam em cada relatório. |
| Console do protótipo | Nenhuma entrada; sem erro grave, divergência de hidratação ou pânico nos percursos executados. |
| Dependências, compilação e políticas | Formatação, plano, linguagens, segredos, Clippy nativo e WASM, auditoria e compilação aprovados. [Compilação](issue-003/ci-43142248739/build.json). |
| Arquivos servidos | WASM 348.440 B, JS de interoperabilidade 8.193 B e CSS 5.570 B; todos abaixo dos limites. [Medição da CI](issue-003/ci-43142248739/budget.json). JS inclui 7.524 B com Brotli e 669 B inline sem compressão. |
| Desempenho da fundação | 80 navegações aprovadas na rota `/`, com cenários de cache e larguras distintos. [Resumo](issue-003/ci-43142248739/browser-performance.json) e [amostras](issue-003/ci-43142248739/performance-samples.json). Esses tempos não são atribuídos às cinco rotas do protótipo. |

O [manifesto](issue-003/manifest.json) identifica os 157 arquivos rastreados fora de `docs/validacao/`, seus hashes e os arquivos de evidência preservados. O hash agregado da implementação e dos testes é `e72a8d8d9f040f27e7a6de066f84c914111ce3220d23cd666f6ee0a843e9c8e1`. A consolidação deste registro acrescenta apenas documentação e evidência dentro de `docs/validacao/`, sem alterar o código testado. Capturas HTML e logs auxiliares permanecem no artefato da CI; scripts gerados não foram versionados como evidência.

### Capturas conferidas

| Tarefa | 320 CSS px | 360 CSS px | 1280 CSS px |
|---|---|---|---|
| Painel | [320](issue-003/ci-43142248739/prototype-painel-320.png) | [360](issue-003/ci-43142248739/prototype-painel-360.png) | [1280](issue-003/ci-43142248739/prototype-painel-1280.png) |
| Orçamento | [320](issue-003/ci-43142248739/prototype-orcamento-320.png) | [360](issue-003/ci-43142248739/prototype-orcamento-360.png) | [1280](issue-003/ci-43142248739/prototype-orcamento-1280.png) |
| Medição | [320](issue-003/ci-43142248739/prototype-medicao-320.png) | [360](issue-003/ci-43142248739/prototype-medicao-360.png) | [1280](issue-003/ci-43142248739/prototype-medicao-1280.png) |
| Aprovações | [320](issue-003/ci-43142248739/prototype-aprovacoes-320.png) | [360](issue-003/ci-43142248739/prototype-aprovacoes-360.png) | [1280](issue-003/ci-43142248739/prototype-aprovacoes-1280.png) |
| Documentos | [320](issue-003/ci-43142248739/prototype-documentos-320.png) | [360](issue-003/ci-43142248739/prototype-documentos-360.png) | [1280](issue-003/ci-43142248739/prototype-documentos-1280.png) |

Estados: [vazio](issue-003/ci-43142248739/prototype-state-empty.png), [carregando](issue-003/ci-43142248739/prototype-state-loading.png), [erro](issue-003/ci-43142248739/prototype-state-error.png) e [sem permissão](issue-003/ci-43142248739/prototype-state-forbidden.png).

Interação: [revisão da aprovação](issue-003/ci-43142248739/prototype-approval-modal.png), [resultado com foco](issue-003/ci-43142248739/prototype-approval-result.png), [troca para Horizonte](issue-003/ci-43142248739/prototype-company-changed.png), [retorno pelo histórico](issue-003/ci-43142248739/prototype-history-context-guard.png), [atalho](issue-003/ci-43142248739/prototype-keyboard-shortcut.png), [movimento reduzido](issue-003/ci-43142248739/prototype-reduced-motion.png) e [consulta sem scripts](issue-003/ci-43142248739/prototype-ssr-no-scripts.png).

As 15 capturas de tela/largura e as quatro de estado são idênticas, byte a byte, às imagens da revisão visual anterior das correções. As capturas finais de aprovação, resultado, troca e histórico também receberam revisão independente e conferência final. Não foi encontrado bloqueador visual nos pontos revistos.

Nas quatro imagens de estado, a captura de página inteira conserva a posição de elementos fixos enquanto a janela estava rolada até os controles. Por isso o link de salto pode aparecer sobre uma região anterior da imagem, embora sua posição calculada esteja totalmente acima da janela visível original. A geometria foi conferida nos quatro casos; esse efeito da captura não foi registrado como foco indevido do produto.

Comandos de referência, com os pré-requisitos e variáveis externos descritos em `docs/desenvolvimento.md` e `docs/persistencia.md`:

```sh
cargo fmt --all -- --check
cargo xtask check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo clippy --locked -p canteiro-web --no-default-features --features hydrate --target wasm32-unknown-unknown -- -D warnings
cargo test --locked --workspace
cargo test --locked -p canteiro-persistence -- --ignored --nocapture
cargo deny --locked check
cargo xtask build
cargo xtask budget
cargo test --locked -p canteiro-e2e -- --ignored --nocapture
```

A suíte `prototype_accessibility_context_and_recovery` executa o aplicativo Rust/Leptos real e registra as medidas e capturas em `target/evidence`. As interações usam teclas e cliques WebDriver; os scripts de diagnóstico apenas observam DOM, estilo e foco. A integração também mantém os testes da fundação e do PostgreSQL.

## Revisão adversarial

Revisão de requisitos, revisão independente de código e implementação de testes negativos foram conduzidas separadamente. As correções abaixo foram conferidas; a execução aprovada confirmou os percursos de navegador, inclusive histórico e aprovação, e repetiu os testes de banco. A revisão final não identificou bloqueador técnico nos cenários executados.

| Achado | Correção e prova aplicável |
|---|---|
| Voltar para outra empresa podia descartar um rascunho sem decisão | Estado associado à empresa, restauração da última rota e diálogo antes do descarte; cenário de histórico com cancelamento e nova tentativa de confirmação na suíte de navegador |
| Contexto desconhecido recebia HTTP 200 | Vocabulário de rotas compartilhado com a fronteira HTTP; testes de GET, HEAD e método inválido, além das dez rotas válidas |
| Painel mantinha pendência depois da aprovação demonstrativa | O painel deriva seu estado do mesmo registro local; faturado e pago continuam separados e não são alterados pela simulação |
| Data malformada podia alcançar conversões inadequadas | Validação de formato ASCII e calendário antes da comparação; testes negativos e de ano bissexto |
| Texto dinâmico dentro de textarea arriscava divergência entre SSR e hidratação | Valor controlado pela propriedade do campo, sem filho dinâmico de texto bruto; conservação do texto verificada nos percursos de edição e navegação |
| Composição de tipos excedia o limite de análise da compilação SSR | Fronteira de tipos da shell limitada, com compilação nativa e WASM concluída |
| Documentação sugeria entrada de arquivo já entregue e retorno de foco idêntico em toda saída | Texto corrigido: arquivos ainda não são recebidos; cancelar devolve foco ao acionador e concluir conduz ao resultado |
| A primeira execução do novo teste leu o atributo HTML de um seletor, sem observar seu valor atual | Leituras de valores editáveis passam a usar a propriedade DOM via WebDriver; as verificações de valor foram preservadas. A [execução 43130838092](https://github.com/tjsasakifln/canteiro-360/actions/runs/43130838092) aprovou a fundação, o banco e os dois testes de navegador anteriores, mas interrompeu o novo protótipo nesse ponto; não constitui aprovação integral. |
| Tab saía do diálogo de troca de empresa para BODY | A [segunda execução](https://github.com/tjsasakifln/canteiro-360/actions/runs/43133648356) revelou o defeito no navegador real. O componente Rust agora controla o ciclo das ações dos dois diálogos, preservando Escape e atalhos do navegador. O teste continua exigindo foco contido, sem aceitar BODY. |
| O teste observava o diálogo fechado antes da restauração feita pelo evento `close` | O [algoritmo HTML](https://html.spec.whatwg.org/multipage/interactive-elements.html#close-the-dialog) remove `open` antes de enfileirar o evento. A terceira execução registrou a restauração correta após a leitura prematura. As esperas agora exigem diálogo fechado, empresa correta e foco restituído, ou resultado visível e focado após aprovação. Os limites de tempo anteriores e as demais asserções foram preservados. |
| O teste de identidade do banco reutilizava um prazo de 120 ms em conexões que deveriam ser aceitas | A [execução 43141185900](https://github.com/tjsasakifln/canteiro-360/actions/runs/43141185900) aprovou 14 cenários de PostgreSQL e interrompeu uma conexão positiva por prazo excedido, antes do navegador. As quatro aberturas positivas desse cenário passam a usar o limite normal de teste de dois segundos. As conexões inválidas conservam o limite curto e as verificações de recusa; produção e testes específicos de prazo não mudaram. Os 15 cenários passaram novamente em PostgreSQL 17.7 real; não se atribui ao executor uma lentidão que não foi medida. |
| A tabela móvel fragmentava nomes de serviços; o menu de 320 px fragmentava rótulos | A revisão visual encontrou falhas que a medida de transbordamento, isoladamente, não detectava. Coluna Serviço com mínimo de 220 px; quebra entre palavras e ícones decorativos retirados apenas na menor largura. A tabela conserva rolagem própria. |
| O cabeçalho móvel afastava a tarefa e a ação principal | Aviso de demonstração abreviado, título decorativo repetido removido e opções de demonstração movidas para depois da tarefa. Empresa, obra, contrato e revisão permanecem identificados no cabeçalho. |
| Campo nativo de data dependia do formato regional do navegador | Resumo reativo do período em português, junto à orientação do formulário, sem alterar os valores civis ISO ou as regras de validação. |

Não atribuir aos testes automatizados conformidade integral WCAG ou teste manual com leitor de tela. Os resultados de interação e as capturas acima identificam a revisão realmente executada; o teste com profissionais permanece pendente.

## Encerramento

A implementação da [PR 75](https://github.com/tjsasakifln/canteiro-360/pull/75) está tecnicamente verificada para avaliação. A issue 3 permanece aberta, sem fechamento automático, até o teste com pelo menos três profissionais reais e o tratamento de eventuais problemas graves. A sequência de integração das próximas issues continua sujeita à aceitação desta etapa.
