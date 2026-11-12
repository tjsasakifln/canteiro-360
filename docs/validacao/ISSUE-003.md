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

Os resultados finais, a versão testada e os artefatos da integração serão registrados nesta seção após a execução. A existência dos cenários e do código de teste não significa que já passaram.

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

Revisão de requisitos, revisão independente de código e implementação de testes negativos foram conduzidas separadamente. Os achados, correções e retestes finais serão consolidados após a execução da versão candidata. Não atribuir aos testes automatizados conformidade integral WCAG ou teste manual com leitor de tela.

## Encerramento

Uma PR tecnicamente verificada pode ser apresentada para avaliação. Não usar fechamento automático da issue 3 nem tratar os componentes como definitivamente aceitos enquanto o teste com profissionais estiver pendente. A sequência de integração das próximas issues continua sujeita à aceitação desta etapa.
