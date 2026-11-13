# Teste inicial com profissionais: issue 3

**Estado deste arquivo:** roteiro preparado, sem participantes registrados e sem teste humano realizado neste registro.

## Objetivo

Verificar se pelo menos três profissionais representativos conseguem identificar a ação útil, o valor e o estado nos protótipos do Canteiro 360. Corrigir problemas graves e repetir as tarefas afetadas antes de considerar os componentes definitivamente aceitos.

Este teste atende ao critério específico da [issue 3](https://github.com/tjsasakifln/canteiro-360/issues/3). Não substitui os testes de experiência integrada, os pilotos profissionais ou o aceite comercial das etapas posteriores. Revisores automatizados, agentes e testes de navegador não contam como participantes humanos.

## Preparação

- Usar somente os exemplos fictícios disponíveis no protótipo. Não solicitar dados, credenciais, documentos ou valores de clientes.
- Registrar o endereço do ambiente, commit completo, navegador, tamanho da janela e data de cada sessão. Uma alteração funcional durante o teste exige identificar a nova versão.
- Reunir pelo menos três pessoas com atuação profissional pertinente. Buscar funções complementares, como orçamento/planejamento, fiscalização/gestão de obra e aprovação/gestão contratual.
- Informar que a interface é uma demonstração e que as ações não produzem aprovação ou gravação comercial. Evitar apresentar os caminhos dos menus antes das tarefas.
- Pedir autorização antes de gravar tela ou voz. É possível conduzir o teste apenas com notas. Manter eventual gravação em destino de acesso restrito, fora do repositório.
- Identificar participantes por códigos P01, P02 e P03 no registro. Nome, contato e outros dados pessoais não são necessários neste registro do projeto.
- Executar ao menos uma sessão na largura móvel de 360 CSS px e uma em computador. A matriz automatizada de larguras continua sendo uma verificação separada.

## Instrução inicial ao participante

“Esta é uma demonstração de gestão de obras com dados fictícios. Queremos entender se você encontra o que precisa e compreende as informações. Nenhuma ação cria uma aprovação ou registro comercial. Tente cumprir os objetivos como faria no seu trabalho. Diga o que está procurando e o que entende da tela. Você pode pedir ajuda quando precisar.”

O moderador não deve apontar botões, citar o menu correto ou explicar o significado dos valores antes da resposta. Se precisar ajudar, registrar a intervenção e o ponto em que foi necessária.

## Tarefas

| Código | Objetivo entregue ao participante | O que observar |
|---|---|---|
| T01 | “Veja a obra e diga qual é a próxima ação útil, quem precisa agir e se existe alguma pendência.” | Identifica contexto, estado e ação sem confundir indicador com ato disponível |
| T02 | “Encontre os documentos e identifique qual versão pode consultar, mesmo que haja etapa de cadastro pendente.” | Consulta documentos sem esperar concluir todos os menus; distingue revisão |
| T03 | “Confira o orçamento. Diga seu total e onde encontra os serviços e suas unidades.” | Lê total, unidades e valores; não confunde total com linha editável |
| T04 | “Prepare a medição demonstrativa e explique seu valor, estado e o que falta para concluí-la.” | Compreende período e revisão; identifica a pendência de comissão; não interpreta bloqueio como falha sem solução |
| T05 | “Revise a aprovação, explique o que ela faria e depois cancele sem aprovar.” | Identifica empresa, obra, contrato, versão, valor e consequência; sai sem confirmar |
| T06 | “Troque para a outra empresa. Diga qual obra e contrato está vendo e o que aconteceu com o que estava preenchendo.” | Não mistura organizações; compreende preservação ou descarte decidido do rascunho |
| T07 | “Se aparecer uma falha durante o preenchimento, tente corrigir e continuar.” | Percebe a mensagem, encontra recuperação e mantém entradas válidas |

O moderador prepara o cenário de cada tarefa sem expor a resposta. Nos estados demonstrativos, selecionar o cenário necessário antes de entregar o objetivo. A tarefa de erro pode usar uma entrada inválida controlada ou a apresentação de erro disponível.

Depois das tarefas, perguntar: “O que você entendeu por contratado, medido, faturado e pago pelo contratante?” Registrar a explicação com palavras do participante. Não considerar suficiente que tenha lido os rótulos.

## Registro da versão

Preencher após uma sessão real; conservar em branco até ocorrer.

| Campo | Valor |
|---|---|
| Endereço testado | |
| Commit completo | |
| Data e fuso | |
| Responsável por conduzir o teste | |
| Escopo e estado do protótipo | |
| Referência restrita de evidência, se houver | |

## Participantes

| Código | Atuação profissional | Familiaridade com ferramentas de gestão | Dispositivo e navegador | Largura CSS | Data |
|---|---|---|---|---|---|
| P01 | | | | | |
| P02 | | | | | |
| P03 | | | | | |

Se houver participantes adicionais, acrescentar códigos sequenciais. Não preencher papéis ou respostas por hipótese.

## Resultados por tarefa

Duplicar as linhas necessárias para cada participante. Usar “sem ajuda”, “com ajuda”, “não concluiu” ou “não executada” no resultado. “Não executada” não equivale a aprovação.

| Participante | Tarefa | Resultado | Tempo | Ação identificada | Valor compreendido | Estado compreendido | Ajuda ou erro observado |
|---|---|---|---|---|---|---|---|
| | | | | | | | |

Quando ação, valor ou estado não se aplicarem à tarefa, registrar o motivo. Para compreensão, anotar a resposta observada; não substituir por uma nota geral de satisfação.

## Classificação das falhas

| Gravidade | Critério | Tratamento |
|---|---|---|
| Grave | Participante age na empresa/obra/versão errada; interpreta simulação como ato real; aprova sem compreender o valor; perde rascunho sem decisão; não consegue completar ou cancelar uma tarefa essencial | Corrigir e repetir a tarefa antes do aceite dos componentes |
| Relevante | Precisa de ajuda para descobrir a ação, compreender o estado ou resolver um bloqueio; erra leitura de valores sem produzir ato indevido | Corrigir e testar novamente o trecho afetado |
| Melhoria | Hesitação ou preferência estética sem impedir ação, compreensão ou recuperação | Registrar decisão e prioridade, sem ocultar defeitos funcionais |

Um bom resultado agregado não compensa um erro grave. Não declarar conformidade integral de acessibilidade com base neste roteiro; os ensaios técnicos e manuais específicos precisam de evidência própria.

## Correções e retestes

| Problema | Evidência observada | Gravidade | Correção | Commit corrigido | Participante/tarefa do reteste | Resultado |
|---|---|---|---|---|---|---|
| | | | | | | |

## Decisão de aceite

Manter os itens abaixo desmarcados até existir evidência correspondente.

- [ ] Participaram pelo menos três profissionais reais, com funções e cenário registrados.
- [ ] A versão testada está identificada por commit completo.
- [ ] Os participantes identificaram ação, valor e estado nos percursos pertinentes.
- [ ] As intervenções, incompreensões e falhas foram registradas.
- [ ] Problemas graves foram corrigidos e as tarefas afetadas foram repetidas.
- [ ] A revisão da evidência foi realizada pelo responsável pelo produto ou pessoa por ele autorizada.

**Decisão registrada:** pendente de execução do teste com profissionais.

## Referências

- [Issue 3](https://github.com/tjsasakifln/canteiro-360/issues/3), critérios de aceite.
- [Sistema visual](../../sistema-visual.md), contratos dos componentes e limites da demonstração.
- [Experiência](../../experiencia.md), contexto, acessibilidade, recuperação e validação com usuários.
