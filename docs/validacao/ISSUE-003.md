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

Os testes nativos, o Clippy, a compilação SSR/WASM e as medidas locais foram repetidos após as correções de foco, leitura móvel e resumo do período, sobre os arquivos de implementação da revisão `ab2685bea1c9c10c1b81e9bcbfdfe2b49b1e1632`. A [terceira execução de integração](https://github.com/tjsasakifln/canteiro-360/actions/runs/43137531210) aprovou essas etapas, o PostgreSQL e os dois testes de navegador anteriores. O novo teste passou pelo ciclo de foco corrigido, mas leu o seletor antes do evento de fechamento terminar sua restauração. A captura subsequente registra empresa e foco corretos. A espera foi corrigida para exigir o resultado completo; a nova execução ainda precisa concluir os percursos de histórico e aprovação. A existência de um cenário ainda não executado não significa que passou.

| Verificação local | Resultado e evidência |
|---|---|
| Testes nativos | 45 aprovados, zero falhas; [log](issue-003/native-tests.log). As 18 verificações que exigem banco/navegador não são executadas pelo comando nativo e têm etapas explícitas próprias. |
| PostgreSQL 17.7 real | 15 aprovados, zero falhas e zero ignorados na execução explícita; [log](issue-003/postgresql-tests.log). Instância e testes executados na mesma sessão isolada. |
| Clippy nativo e WASM | Sem avisos tratados como erro; [nativo](issue-003/clippy-native.log) e [WASM](issue-003/clippy-wasm.log). |
| Formatação e regras | Formatação, política de linguagens/segredos e consistência das 72 issues e 192 regras aprovadas. |
| Dependências | `cargo deny --locked check` aprovado. A exceção anterior de `paste 1.0.15` permanece restrita e documentada; nenhuma nova exceção foi criada. |
| Compilação SSR/WASM | Aprovada com Rust 1.99.1 e wasm-bindgen 0.2.130; [registro](issue-003/build-local.json). |
| Pacote servido | WASM 348.809 B, interoperabilidade JS 8.193 B, CSS 5.570 B, nos limites de 1 MiB, 80 KiB e 60 KiB; [medição local](issue-003/budget-local.json). Inclui as duas folhas CSS e o conteúdo inline. |

O executor local impede sockets Unix, necessários para iniciar o Chromium disponível. A prova de interação e as capturas serão obtidas pela CI com navegador real, sem substituir o teste por HTML estático. A simulação de viewport não é apresentada como ensaio em aparelho físico, zoom manual de 400% ou teste com leitor de tela.

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

Revisão de requisitos, revisão independente de código e implementação de testes negativos foram conduzidas separadamente. A revisão final de código não identificou novo bloqueador nos pontos abaixo; a execução em navegador ainda precisa confirmar os percursos correspondentes.

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
| O teste observava o diálogo fechado antes da restauração feita pelo evento `close` | O [algoritmo HTML](https://html.spec.whatwg.org/multipage/interactive-elements.html#close-the-dialog) remove `open` antes de enfileirar o evento. A terceira execução registrou a restauração correta após a leitura prematura. As esperas agora exigem diálogo fechado, empresa correta e foco restituído, ou resultado visível e focado após aprovação. Os limites de três segundos e as demais asserções foram preservados. |
| A tabela móvel fragmentava nomes de serviços; o menu de 320 px fragmentava rótulos | A revisão visual encontrou falhas que a medida de transbordamento, isoladamente, não detectava. Coluna Serviço com mínimo de 220 px; quebra entre palavras e ícones decorativos retirados apenas na menor largura. A tabela conserva rolagem própria. |
| O cabeçalho móvel afastava a tarefa e a ação principal | Aviso de demonstração abreviado, título decorativo repetido removido e opções de demonstração movidas para depois da tarefa. Empresa, obra, contrato e revisão permanecem identificados no cabeçalho. |
| Campo nativo de data dependia do formato regional do navegador | Resumo reativo do período em português, junto à orientação do formulário, sem alterar os valores civis ISO ou as regras de validação. |

Não atribuir aos testes automatizados conformidade integral WCAG ou teste manual com leitor de tela. Os resultados finais de interação e as capturas devem identificar a revisão realmente executada.

## Encerramento

Uma PR tecnicamente verificada pode ser apresentada para avaliação. Não usar fechamento automático da issue 3 nem tratar os componentes como definitivamente aceitos enquanto o teste com profissionais estiver pendente. A sequência de integração das próximas issues continua sujeita à aceitação desta etapa.
