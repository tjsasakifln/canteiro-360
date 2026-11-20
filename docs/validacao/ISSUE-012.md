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

A verificação central também aprovou 47 testes de aplicação com PostgreSQL, incluindo os nove casos `registry_`. O [relatório integrado das issues 10 a 14](ISSUES-010-014.md#resultado-final-conferido) acompanha a verificação da versão e as evidências.

Os cinco testes puros de `canteiro-domain portfolio::` passaram em 19/11/2026 (horário de Brasília), incluindo somas exatas, diferença faltante de percentual e dimensão, limites de coordenadas e vínculos repetidos.

A revisão independente aprovou a correção dos quatro achados de autorização temporal, identidade de repetição, descarte de respostas após navegar e associação do histórico à obra selecionada.

## Interface e evidências de navegador

### Execução anterior: CI 44648996986

Na [CI 44648996986](https://github.com/tjsasakifln/canteiro-360/actions/runs/44648996986), o percurso de carteira executou criação e revisão da obra, autorização, coordenadas e verificações de largura antes de parar no formulário de projetos. A cota de artefatos impediu recuperar as capturas dessa execução. Esse resultado permanece histórico e não comprova a aparência da correção posterior.

### CI 44657829805: percurso concluído e falha visual identificada

A [CI 44657829805](https://github.com/tjsasakifln/canteiro-360/actions/runs/44657829805) aprovou os 13 testes de navegador, incluindo o percurso completo das cinco telas. A head funcional era `f1c297d029fdc31742bbc0d933c5e8ed7acb86b5`; o checkout efetivamente testado e registrado nas evidências foi `2de5b86fa1d6ebf3ccad1f064aac59dbaf1b0464`.

As capturas `registry-carteira-360.png` e `registry-carteira-1280.png` foram recuperadas pelos registros da CI, com origem, tamanho e SHA-256 conferidos. A inspeção confirmou a obra revisada na versão 2, o rateio de 100%, a dimensão preservada, os campos de coordenadas e o ponto correspondente no mapa. Não foi observado transbordamento horizontal nas duas larguras. Na captura móvel, o link de pular conteúdo aparecia em foco sobre a latitude; o registro seguinte remove apenas esse foco transitório antes da captura, depois das operações por teclado.

A revisão encontrou seletores diretos com apresentação nativa pequena e título da obra colado aos metadados da lista. A correção em `style/registry.css` amplia a fonte e os controles, define altura mínima de 44 pixels, separa rótulos e metadados e mantém o campo de rateio na largura disponível. Dois agentes revisaram o CSS sem encontrar falha material. A aprovação funcional dessa CI não constitui aprovação visual da versão corrigida. O [relatório integrado](ISSUES-010-014.md#ci-44657829805-e-revisão-das-capturas) registra os resultados completos e as limitações de auditoria e disponibilização dos artefatos.

### CI 44663208420: correção confirmada no estado ensaiado

A [CI 44663208420](https://github.com/tjsasakifln/canteiro-360/actions/runs/44663208420) testou a head `7166902c789aac63092a470b13175eefbd18f067`, pelo checkout `36edb2b1a98ebfac9385b73a180b49f71f0bb61a`. A compilação de produção, o orçamento de arquivos e os 13 testes de navegador passaram; o percurso integrado terminou em 30,49 segundos. A mesma CI aprovou os 47 testes de aplicação com PostgreSQL, incluindo os nove `registry_`. A auditoria continuou bloqueada pelos avisos de manutenção `RUSTSEC-2026-0543` e `RUSTSEC-2026-0555`, e o upload convencional falhou por cota de armazenamento. A aprovação funcional e visual aqui registrada não altera o resultado global da CI.

As novas capturas foram recuperadas com conferência de origem, tamanho e SHA-256 e abertas para revisão: [carteira em 360 px](issue-010-014/registry-carteira-360.png) e [carteira em 1280 px](issue-010-014/registry-carteira-1280.png). O [registro de medidas](issue-010-014/registry-interface.json) informa:

| Largura da janela | Controles visíveis medidos | Menor altura | Menor largura | Largura do conteúdo |
|---|---:|---:|---:|---:|
| 360 px | 29 | 44 px | 110 px | 360 px |
| 1280 px | 29 | 44 px | 103 px | 1265 px |

Para checkboxes e radios, a medição usa o rótulo associado, mantendo o controle compacto. A imagem de desktop tem 1265 pixels de largura porque a barra de rolagem ocupa 15 pixels da janela de 1280.

A revisão confirmou seletores de relação com o ativo, situação e unidade legíveis, com rótulos separados; o campo de municípios ocupa a largura disponível e mostra o rateio informado. Título e metadados da obra estão separados na lista. A obra permanece na versão 2, com dimensão de 100 unidades e rateio de 100%. As coordenadas aparecem sem obstrução nas duas larguras, e o ponto permanece dentro da grade do mapa. Não foram observados cortes de controles, sobreposições ou transbordamento horizontal. A correção visual está aprovada para esse estado nas duas larguras.

O campo de título em uma linha mostra apenas parte do texto em 360 px; o título completo está visível na lista. A revisão não abrange todos os estados de permissão, ausência de coordenadas, contratos, rateio inválido ou histórico, nem equivale a uma auditoria completa de acessibilidade. Esses cenários conservam as evidências de teste descritas acima. Os resultados locais anteriores e as capturas da CI 44657829805 permanecem históricos; as imagens vinculadas nesta seção pertencem à CI 44663208420.

## Limites preservados

A situação da carteira não produz medição, faturamento ou aceite técnico. Referências de acesso continuam separadas de contratos formalizados. A carteira depende da concessão explícita por obra e não atribui acesso geral a administradores. A grade do mapa não usa serviços externos; no celular, seus rótulos pequenos são uma limitação complementar, com a lista e os campos de coordenadas disponíveis sem interação precisa com o mapa. Este registro não fecha a issue nem declara prontidão comercial.
