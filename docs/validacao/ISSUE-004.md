# Verificação da issue 4

## Objeto e situação

Formulários pt-BR, rascunho por serviço, comparação de valores e tabela paginada da base fictícia de medição. A implementação usa a base técnica da PR 75, revisão `2013cef3c5097c087c9e4a116c125d2dfb248596`.

**Situação inicial deste registro:** implementação preparada para a verificação integrada. O teste de navegador e a execução completa da CI ainda precisam ser registrados nesta versão. A issue 3 permanece pendente do teste com três profissionais; este trabalho não substitui esse aceite.

## Critérios verificáveis

| Critério | Prova prevista na implementação |
|---|---|
| `1.234,56` vira `1234.56`; entrada ambígua não muda silenciosamente | Vetores independentes de interpretação, endpoint HTTP e digitação real com leitura do valor interpretado. |
| 10.000 serviços sem 10.000 linhas no DOM | Página limitada no HTTP e no HTML sem scripts; navegação por teclado e exportação do conjunto filtrado. |
| Erro preserva os demais campos e permite corrigir por foco | Validação local e remota, resumo de erros, falha real de rede e retorno ao rascunho. |
| 20 para 35 representa 15 no período, com unidade | Fixture conhecida, prévia e grade; caso monetário adicional de 0,335 para distinguir diferença de acumulados de multiplicação do período. |
| Rascunho não se perde ao filtrar, paginar ou mudar de tarefa | Estado por serviço e guardas de contexto; ensaios de histórico, cancelamento e descarte explícito. |
| Seleção da página é diferente de todos os resultados | Contagens explícitas, cópia da página e download TSV do conjunto. |

## Revisão adversarial durante a implementação

A revisão de contratos e a revisão da interface/testes foram realizadas separadamente. Os achados já tratados incluem geração da massa de dados somente no servidor, mensagem de excesso com limite e saldo, cálculo monetário por acumulados, unidade fixa do serviço, justificativa condicional de preço, descrição verdadeira do envio de prévias, limite de busca, preservação de rascunhos, invalidação de conferência antiga, foco de retorno e leitura pontual do estado sem clonar todos os rascunhos a cada tecla.

Os componentes foram integrados aos avisos de fechamento de aba e troca de empresa existentes. A grade conserva identidade, unidade e edição durante rolagem horizontal. Entradas de data permitem digitar a barra em teclados móveis. A confirmação remota não pode ser reutilizada depois que os dados mudam.

## Comandos de conferência

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo clippy --locked -p canteiro-web --no-default-features --features hydrate --target wasm32-unknown-unknown -- -D warnings
cargo test --locked --workspace
cargo xtask check
cargo deny --locked check
cargo xtask build
cargo xtask budget
cargo test --locked -p canteiro-persistence -- --ignored --nocapture
cargo test --locked -p canteiro-e2e -- --ignored --nocapture
```

Banco e navegador exigem os pré-requisitos externos descritos em `docs/desenvolvimento.md`. Um teste ignorado no comando básico não é um teste aprovado; as etapas explícitas precisam concluir. Resultados de laboratório não equivalem a ensaio com profissionais, leitor de tela ou aparelho físico.

## Limites desta etapa

As APIs são demonstrações sem gravação. Não há autenticação real, permissão comercial, rascunho persistido no banco, medição aprovada ou reconhecimento financeiro. Nenhuma migração é necessária porque a base de exemplos é imutável e o rascunho pertence à aba. O cálculo completo da issue 5 e o funcionamento integrado dos módulos posteriores permanecem no plano original.

O encerramento da issue exige a evidência integrada desta versão e o atendimento da dependência da issue 3. Não usar fechamento automático ou integração da PR enquanto essa dependência estiver pendente.
