# Instruções para implementação

## Objetivo e ordem

Implementar o Canteiro 360 conforme [PRD](PRD.md), [plano](docs/BACKLOG.md) e [matriz de regras](docs/MATRIZ-RASTREABILIDADE.md). Executar as issues 01 a 72 em ordem. A issue atual depende da aceitação das anteriores. Pesquisa e revisão podem ser paralelas; não integrar uma etapa fora da ordem sem decisão explícita do responsável pelo produto e atualização documental.

## Regras obrigatórias

- Escrever a lógica da aplicação em Rust, inclusive front-end Leptos e ferramentas de aplicação. HTML, CSS, SQL e arquivos declarativos são permitidos; JavaScript gerado por interoperabilidade não autoriza uma segunda implementação de negócio em JS/TS.
- Validar autorização, empresa, vínculo, versão, estado e cálculo no servidor em toda mutação. Nunca confiar no cliente WebAssembly.
- Usar decimal exato para dinheiro, quantidade e percentual, serializado como texto. Não usar `f32` ou `f64` para decisões financeiras.
- Preservar registros definitivos. Corrigir por revisão controlada ou lançamento compensatório, conforme o estado e os efeitos já produzidos.
- Manter contratante, contratado, executor, conferente, aprovador e pagador como funções distintas. Associação a uma empresa não dá acesso automático a todas as obras.
- Separar contrato, execução, medição, aprovação, fatura, pagamento, recebimento e aceite técnico.
- Toda regra de origem SICOP deve apontar para a fonte e localização registradas. Não atribuir ao legado decisões próprias do produto.
- Não introduzir obrigações de contratação pública em contratos privados sem previsão expressa aprovada.
- Testes automatizados, exemplos e arquivos do repositório usam dados fictícios; nenhum segredo ou documento real de cliente entra no código. Pilotos e verificação operacional podem usar dados reais autorizados em ambiente controlado. Evidências sensíveis ficam em acesso restrito; a issue registra referência, versão e resultado sem expor o conteúdo.
- Não alterar requisitos silenciosamente para fazer um teste passar. Corrigir a implementação ou propor mudança rastreável.

## Critério de conclusão por issue

Entregar migrações, domínio, interface completa, autorizações, auditoria, testes positivos e negativos, documentação e evidência visual quando a tela mudar. Aplique os cenários específicos da issue e das regras vinculadas. Revisão adversarial deve verificar concorrência, saldo, empresa errada, versão antiga, duplicidade e comunicação de erro quando aplicável.

Referenciar a issue no PR, registrar versão/commit e resultado dos testes. Não afirmar aprovação comercial por sucesso de compilação ou por fechar tickets. A issue 72 contém o aceite da versão publicada.

## Comunicação e produto

Português brasileiro, texto direto, sem travessões. Explicar a consequência de uma operação, o motivo de um impedimento e como resolvê-lo. Não expor nomes de tabelas, detalhes do compilador, siglas internas ou explicações de implementação aos usuários finais quando isso não ajuda sua decisão.

## Revisão de especificação

A revisão registrada em `docs/validacao` refere-se aos documentos desta preparação. Cada implementação exige nova verificação proporcional ao risco. Preserve as evidências anteriores e identifique o objeto realmente testado.
