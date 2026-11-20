# Validação da issue 12

## Objeto

Carteira de empreendimentos, obras e ativos sobre a base da PR 82. Os módulos novos são `portfolio`, com `0011_portfolio.sql`, autorização explícita de consulta e gestão no escopo de obra, painel filtrado e mapa privado. Guia: [Cadastros e carteira](../cadastros-carteira.md).

## Verificação executada

O grupo `registry_` da aplicação passou com PostgreSQL 17.11 real e Rust 1.99.1: 9 aprovados, 0 falhas, 18,60 s em 19/11/2026 (horário de Brasília). Três casos específicos da carteira e dois cenários compartilhados de contexto e expiração verificaram:

| Critério | Evidência executada |
|---|---|
| Múltiplos contratos, uma obra | Dois contratos vinculados permanecem em uma única entrada na carteira; os relacionamentos não são duplicados por repetição. |
| Rateio exato | Percentual incompleto é rejeitado antes de gravar; os testes puros também verificam diferença decimal de dimensão e percentual. |
| Coordenadas e ausência | Uma obra sem coordenadas permanece sem localização; os testes puros rejeitam valores fora dos limites e o preenchimento de apenas uma coordenada. |
| Mapa, busca, totais e consulta direta | O leitor de uma obra vê uma única entrada e um ponto; não encontra, conta nem consulta outra obra sem concessão. O administrador também precisa de acesso explícito. |
| Histórico e concorrência | Repetição simultânea retorna o mesmo recibo; conteúdo diferente com a mesma chave falha; edição obsoleta falha e conserva as duas revisões. |
| Expiração durante uma consulta | Consulta direta e histórico esperaram um bloqueio real da linha; a concessão venceu durante a espera e nenhuma resposta com dados foi liberada. |
| Atualização legítima de contexto | Renomear e selecionar novamente a empresa permite recuperar o mesmo recibo com o contexto vigente, sem duplicar obra ou histórico. |
| Revogação e outra empresa | Uma empresa diferente não consulta o identificador conhecido. A revogação impede recuperar o recibo e remove a obra da lista. |

A primeira execução encontrou uma falha na função SQL de conferência do vínculo: um ramo fazia referência a um campo existente somente na outra tabela. A função passou a distinguir a tabela antes de preparar a expressão. O grupo inteiro foi reexecutado com as migrações corrigidas e passou.

Comando executado: `cargo test -p canteiro-application registry_ --offline -- --ignored --test-threads=1`. A interface também compilou com `cargo check -p canteiro-web --features ssr --offline`.

A verificação central também aprovou 47 testes de aplicação com PostgreSQL, incluindo os nove casos `registry_`. O [relatório integrado das issues 10 a 14](ISSUES-010-014.md#resultados-da-versão-integrada) acompanha a verificação da versão e as evidências.

Os cinco testes puros de `canteiro-domain portfolio::` passaram em 19/11/2026 (horário de Brasília), incluindo somas exatas, diferença faltante de percentual e dimensão, limites de coordenadas e vínculos repetidos.

A revisão independente aprovou a correção dos quatro achados de autorização temporal, identidade de repetição, descarte de respostas após navegar e associação do histórico à obra selecionada. Na [CI 44648996986](https://github.com/tjsasakifln/canteiro-360/actions/runs/44648996986), o percurso de carteira executou criação e revisão da obra, autorização, coordenadas e verificações de largura antes de parar no formulário de projetos. A cota de artefatos impediu recuperar as capturas. A inspeção visual e a conclusão do percurso integrado permanecem pendentes de nova execução, conforme o [registro consolidado](ISSUES-010-014.md). A grade do mapa não usa serviços externos e todas as operações também estão disponíveis pela lista e pelos campos de texto.

## Limites preservados

A situação da carteira não produz medição, faturamento ou aceite técnico. Referências de acesso continuam separadas de contratos formalizados. A carteira depende da concessão explícita por obra e não atribui acesso geral a administradores. Este registro não fecha a issue nem declara prontidão comercial.
