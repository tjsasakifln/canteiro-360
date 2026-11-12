# Verificação da issue 4

## 1. Objeto, versão e situação

Formulários pt-BR, rascunho por serviço, comparação de valores e tabela paginada da base fictícia de medição. O [guia de uso e contratos](../formularios-tabelas.md) descreve os componentes e as rotas entregues. O escopo é o da [issue 4](../issues/004.md), incluindo as regras OMDEC03, UX03, UX06, UX08 e UX11.

**Situação deste registro em 12/11/2026: validação técnica aprovada na CI 43227797776.** A fonte `5588b0c6` passou em 60 testes nativos, 15 testes PostgreSQL e seis testes de navegador, incluindo os três grupos da planilha. Histórico, rascunhos, respostas atrasadas, remontagem do editor, foco e troca de empresa concluíram seus percursos. Os resultados e capturas foram conferidos com a fonte testada. A aprovação técnica não autoriza integração fora da ordem: a PR 75 e a issue 3 continuam aguardando três profissionais reais. Esta consolidação não realiza merge nem fecha a issue 4.

| Referência | Identificação |
|---|---|
| Base da implementação | [PR 75](https://github.com/tjsasakifln/canteiro-360/pull/75), revisão `2013cef3c5097c087c9e4a116c125d2dfb248596` |
| Proposta da issue 4 | [PR 76](https://github.com/tjsasakifln/canteiro-360/pull/76), dependente da PR 75 |
| Fonte aprovada tecnicamente | `5588b0c695c13259e6e19b0a5a1256e39112a6c8` |
| Árvore dessa fonte | `ccdc477a623394a0af87450cc64a406f3f7821d1` |
| Referência local correspondente | `a168534aa2a20da2fc7b9e2e37b5df1f343bc813`, com a mesma árvore acima |
| CI aprovada | [CI 43227797776](https://github.com/tjsasakifln/canteiro-360/actions/runs/43227797776), job `131215442189` |
| Checkout executado | `7f4ad5576ac14c373c13de965a8822e19ae948be`; commit completo confirmado no log e árvore igual à fonte conferida pela API |
| Artefato final | `13950282541`, ZIP com SHA-256 `dd1f884fed3739eb42848c12c3f14392c1a2fb35b8edc69f9bc488adbca7081f` |
| Seleção e rastreabilidade | [Registro da execução](issue-004/ci-43227797776/run.json), [manifesto da CI](issue-004/ci-43227797776/manifest.json) e [manifesto geral](issue-004/manifest.json) |
| Execução anterior preservada | [CI 43220954664](https://github.com/tjsasakifln/canteiro-360/actions/runs/43220954664), fonte `b6dc408221b0bc5a1e71da01947e277c2380f170`; falha de histórico descrita na seção 7 |
| Pendência de integração | Aceite da dependência da issue 3 com pelo menos três profissionais reais; PRs e issues permanecem abertas nesta consolidação |

A issue 3 ainda exige teste com pelo menos três profissionais reais. Sua implementação técnica não substitui esse aceite. A issue 4 deve permanecer dependente da base, sem integração fora da ordem ou fechamento automático.

A fonte intermediária `b6c0e5a127bfcd732e5e0c53e76ac8d4fac50a96`, árvore `a714c583ed85a7a827e97a1511cd956a3d8ed7e0`, iniciou a [CI 43214142902](https://github.com/tjsasakifln/canteiro-360/actions/runs/43214142902). A consulta aos jobs confirmou seu cancelamento durante PostgreSQL; compilação release e navegador não chegaram a executar. A alteração posterior ficou restrita ao auxiliar de navegação por Tab dos testes: aceitar o alvo já focado e permitir atravessar os controles das 25 linhas. Nenhum resultado parcial dessa execução é apresentado como aprovação de uma fonte posterior.

## 2. Entregas submetidas à verificação

As páginas `/prototipo/aurora/planilha` e `/prototipo/horizonte/planilha` consultam conjuntos fictícios de 10.000 serviços. A página tem 25, 50 ou 100 linhas; busca e ordenação acontecem no servidor. Os cabeçalhos distinguem contratado, anterior acumulado, atual acumulado, diferença no período, preço e saldos. A unidade acompanha o serviço e não pode ser trocada silenciosamente durante a edição.

Os campos conservam o texto original e apresentam a interpretação brasileira, o erro correspondente e a orientação para correção. A observação torna-se obrigatória quando o preço da simulação difere da referência. Os componentes públicos de entrada, comparação e paginação são separados do estado específico da medição.

O rascunho pertence ao serviço e à empresa da aba. Ele distingue edição, conferência remota e cópia local, com autor demonstrativo, revisão e horário. Mudar um campo invalida a conferência anterior. Filtrar, paginar e navegar na mesma empresa preservam entradas; trocar de empresa exige decisão sobre o descarte. Cópia e TSV usam a base de referência, sem apresentar rascunhos locais como dados gravados.

As rotas de consulta, exportação e prévia são limitadas aos exemplos fictícios. A busca admite até 80 caracteres, observações até 1.000 e o corpo de prévia até 16 KiB. Consulta inválida, decimal recebido como número JSON, formato ambíguo, precisão excedida, data inexistente, percentual fora de faixa, unidade incompatível e acumulado fora dos limites são recusados. As respostas usam `no-store`.

## 3. Critérios e evidências da revisão aprovada

Os resultados seguintes pertencem à fonte `5588b0c695c13259e6e19b0a5a1256e39112a6c8`, executada na CI 43227797776. Os relatórios de [apresentação/HTTP/SSR](issue-004/ci-43227797776/forms-presentation.json), [rascunhos e contexto](issue-004/ci-43227797776/forms-functional.json) e [resposta local](issue-004/ci-43227797776/forms-local-response.json) terminaram aprovados, com console vazio nos três grupos.

| Critério ou regra | Cenário discriminante | Evidência e alcance |
|---|---|---|
| Interpretação pt-BR | `1.234,56` resulta em `1234.56`; `1.234` isolado é recusado; três casas em quantidade não viram milhar; preço permite seis casas. | Contratos e HTTP aprovados; a jornada funcional concluiu preenchimento, interpretação e correção de dinheiro, quantidade, data, percentual e unidade. |
| Consulta de 10.000 serviços | A resposta e o DOM inicial contêm somente a página solicitada; busca, ordenação e paginação não dependem da carga integral no cliente. | Apresentação aprovada com 25 linhas no DOM, paginação sem scripts, busca e ordenação no servidor. Os probes da mesma fonte conferem páginas de 25, 50 e 100 linhas nas duas empresas. |
| Preservação e correção de erros | Erro de um campo mantém os demais, oferece resumo acionável e leva o foco à correção; falha de rede não simula gravação. | Data impossível, valor ambíguo e limite excedido preservaram os demais campos. Falha real da prévia manteve entradas; falha real da consulta permitiu retentativa e recuperação. |
| OMDEC03 | Anterior 20, atual 35, contratado 100 e PU 10 produzem 15 no período, R$ 150,00 e saldo de 65 unidades. | Contratos, HTTP, grade e conferência de edição aprovados. A alteração demonstrativa de preço também apresentou a comparação de valores sem modificar a referência contratual. |
| Centavos por acumulados | PU 0,335, anterior 1 e atual 2 produzem R$ 0,34, e não R$ 0,33 obtido pela truncagem isolada do incremento. | Contratos e prévia HTTP confirmados. O vetor específico não foi editado pela interface nessa jornada. |
| UX03 | Campo condicional exige observação ao alterar o preço simulado; opções avançadas, rótulo e motivo são alcançáveis por teclado. | Obrigatoriedade, abertura dos detalhes, foco na observação e conferência aprovados. Tab/Shift+Tab percorreu a grade e retornou à primeira linha em 360 e 1280 CSS px. |
| UX06 | Mensagem de excesso mostra teto de 100 e saldo de 80 a partir do anterior 20, com unidade; entradas permanecem disponíveis. | Contratos conferem a mensagem completa. Na interface, o cenário 101 mostrou o teto de 100 e permitiu corrigir para 35 e conferir novamente. |
| UX08 | Seleção da página e de todos os resultados são ações distintas; cópia e TSV apresentam cabeçalhos e contexto corretos. | Exportação, seleção da página, seleção do conjunto completo e cópia com cabeçalhos aprovadas; rascunhos locais continuam identificados separadamente da base exportada. |
| UX11 | Seleção e expansão respondem em até 100 ms, sem apresentar uma confirmação fictícia do servidor. | Grupo próprio aprovado: 80 efeitos observados, nenhuma violação; máximo de 40,7 ms. |
| Respostas atrasadas | Prévia antiga não certifica entrada nova; consulta concluída não retira o foco de quem começou a digitar enquanto aguardava. | POST de 36 terminou em 1.213,7 ms após mudança para 37 e não foi aplicado. GET da segunda página terminou em 1.205,8 ms, com entrada 38, nota e foco preservados. |
| Instância do editor | Resposta pendente de editor desmontado não certifica outra montagem com os mesmos campos. | POST HTTP 200 de 4.007,1 ms foi ignorado após Documentos/Voltar; a nova prévia explícita confirmou 16 m² e R$ 160,00. |
| Continuidade e contexto | Rascunho sobrevive à navegação na mesma empresa; cancelar troca mantém entradas; confirmar descarte não transporta dados à outra empresa. | Filtro, paginação, retorno pelo histórico, cancelamento da troca e descarte confirmado passaram na jornada funcional. |

Os cenários financeiros são prévias fictícias e não demonstram aprovação, faturamento, pagamento, recebimento ou capacidade comercial do produto. Os resultados negativos das fontes anteriores permanecem nas seções históricas e não foram reclassificados como aprovados.

## 4. Primeira execução e falha preservada

A [CI 43203244043](https://github.com/tjsasakifln/canteiro-360/actions/runs/43203244043) executou a fonte `0a81b3bcc41789a475e6ffe9277e3d99eb10d999`. O checkout temporário foi `213cf6237f3be44a2957d6003e057ae811d5893c`; sua árvore `12c62c90e1dabe8a26ffda6d9a4563818e0c3de6` é igual à árvore da fonte. A identificação está no [registro da execução](issue-004/ci-43203244043/run.json).

| Verificação nessa execução | Resultado constatado |
|---|---|
| Testes nativos | 60 aprovados |
| PostgreSQL real | 15 aprovados na execução explícita |
| Formatação, regras, Clippy nativo e WASM, dependências, compilação e orçamento padrão | Aprovados antes da etapa de navegador |
| Navegador da fundação | 2 testes aprovados |
| Desempenho da fundação | 80 navegações da rota `/`, com os quatro grupos de perfil e cache registrados |
| Planilha | Falha no primeiro percurso sem scripts: zero linhas no DOM ativo, quando eram exigidas 25 |
| Demais percursos da planilha | Não concluídos; a falha interrompeu o teste então agrupado |
| Navegador do protótipo da issue 3 | Não executado nessa etapa, pois o comando terminou após a falha do binário anterior |
| Conclusão da CI | Falhou; não constitui aceite da issue 4 |

O [JSON da falha](issue-004/ci-43203244043/forms-functional.json) e a [captura correspondente](issue-004/ci-43203244043/forms-failure.png) registram a tela parada em consulta. A [inspeção do DOM serializado](issue-004/ci-43203244043/ssr-dom-inspection.json) encontrou zero linhas ativas e 25 dentro de um `<template>` inerte. O [trecho do template](issue-004/ci-43203244043/ssr-inert-fragment.html) foi extraído desse snapshot sem os scripts da página; o registro identifica o hash do snapshot completo e o alcance da extração. O console estava vazio; portanto, ausência de erro no console não significava consulta utilizável.

O modo padrão `SsrMode::OutOfOrder` do Leptos emite o conteúdo suspenso e um script para inseri-lo no documento. `Resource::new_blocking` isoladamente não elimina essa dependência. O teste HTTP anterior contava texto bruto da resposta e podia passar mesmo quando as linhas não apareciam sem JavaScript.

A correção fixa `SsrMode::Async` exclusivamente na rota dinâmica do protótipo, fazendo o servidor resolver os recursos antes de emitir o HTML. O teste HTTP agora recusa `<template>` e o fallback `table-loading`, exige `aria-busy="false"` e conserva a conferência de quantidade, página, empresa, links e tamanho da resposta. O teste real com scripts desativados permanece obrigatório.

O teste HTTP reforçado passou localmente também antes da troca do modo, pois naquela execução o recurso terminou antes da emissão do fallback. Não houve reprodução local vermelha desse caso. A evidência da falha real é a primeira execução de navegador preservada; a segunda CI confirmou a correção com scripts desativados, conforme a seção 5.

### Medidas históricas da fundação

Os números seguintes pertencem somente à primeira execução. O [relatório de desempenho](issue-004/ci-43203244043/browser-performance.json) registra quatro grupos de 20 navegações na rota `/`. O maior LCP p75 foi 628 ms, o maior tempo p75 até hidratação foi 1.685,1 ms, a maior resposta controlada p75 foi 27,2 ms e o CLS p75 foi zero em todos os grupos. Esses tempos não são atribuídos à planilha nem constituem INP de visitantes. A seção 10 apresenta os resultados próprios da fonte final.

| Recurso contabilizado pelo orçamento da rota `/` | Primeira execução | Limite |
|---|---:|---:|
| WASM Brotli | 579.323 B | 1.048.576 B |
| JavaScript externo Brotli mais inline bruto dessa rota | 8.608 B | 81.920 B |
| CSS Brotli | 6.687 B | 61.440 B |

O [orçamento preservado](issue-004/ci-43203244043/budget.json) soma os arquivos servidos e o conteúdo inline da rota `/`. A atribuição desse limite à planilha exige conferir também seu conteúdo inline, incluindo os dados serializados da página. A seção 5 registra essa soma para seis respostas da segunda fonte; a seção 10 registra a comparação da fonte final.

## 5. Segunda execução: SSR confirmado e jornada funcional interrompida

A [CI 43208107786](https://github.com/tjsasakifln/canteiro-360/actions/runs/43208107786) executou a fonte `253b8d6524c017b1fbc1fb342ac5d20609b50e9b`. O checkout temporário `cbcbab951a55a8b569182ca18756efa27f666eb8` possui a mesma árvore da fonte, `c13d38a49548d66269030b00dc59d022c02e9da4`; a correspondência e os dois pais foram conferidos na API do GitHub. O [registro da execução](issue-004/ci-43208107786/run.json) identifica também o artefato `13939618485`, cujo ZIP foi conferido pelo SHA-256 `7d568728679c6a8b87139cbaeb19195276e55797a3371c2a79e83a4a5e044082`. O [manifesto](issue-004/ci-43208107786/manifest.json) distingue os 16 arquivos selecionados do ZIP, o log original do job obtido separadamente e as verificações locais complementares.

| Verificação nessa execução | Resultado constatado |
|---|---|
| Formatação, regras, Clippy nativo e WASM, dependências, compilação e orçamento padrão | Aprovados antes da etapa de navegador |
| Testes nativos | 60 aprovados; 21 ignorados no comando básico |
| PostgreSQL real | 15 aprovados na execução explícita |
| Navegador da fundação | 2 testes aprovados, incluindo 80 navegações da rota `/` |
| Planilha, apresentação e SSR | Aprovado, incluindo navegação com scripts desativados |
| Planilha, resposta local | Aprovado: 80 observações, nenhuma violação do limite de 100 ms |
| Planilha, rascunhos e contexto | Falhou no clique de edição após a fase de seleção/cópia |
| Navegador do protótipo da issue 3 | Aprovado, executado mesmo após a falha anterior por `--no-fail-fast` |
| Conclusão da CI | Falhou; cinco dos seis testes de navegador passaram, sem aprovação integral da issue 4 |

O [grupo de apresentação](issue-004/ci-43208107786/forms-presentation.json) confirmou página limitada, busca e ordenação no servidor, interpretação decimal, recusa do valor ambíguo e os dois exemplos de cálculo. A [captura sem scripts](issue-004/ci-43208107786/forms-ssr-without-scripts.png) mostra a segunda página, iniciada em AUR-00026, após a navegação real. As verificações de 360 e 1280 CSS px encontraram 25 linhas no DOM e nenhum controle sem nome acessível. As capturas de [360 px](issue-004/ci-43208107786/forms-table-360.png) e [1280 px](issue-004/ci-43208107786/forms-table-1280.png) pertencem somente a essa revisão.

O [relatório funcional](issue-004/ci-43208107786/forms-functional.json) e a [captura da falha](issue-004/ci-43208107786/forms-functional-failure.png) registram `element click intercepted` em `edit-row-1`, na coordenada `(355, 624)`: o cabeçalho fixo da coluna de serviço receberia o clique. O foco estava em `clear-selection`, a hidratação estava pronta e o console estava vazio. A fase de seleção/cópia havia sido percorrida; a edição de campos, os erros de rede, as respostas atrasadas e a troca de contexto posteriores não foram concluídos. O HTML completo da falha contém scripts e não foi incorporado ao repositório; seu hash consta no registro da execução.

A revisão posterior ajustou as margens de rolagem, a largura das colunas e a área de seleção. A [inspeção do ChromeDriver fixado](issue-004/chromedriver-scroll-inspection.json) registra que sua rolagem automática altera os deslocamentos diretamente, sem consultar a reserva de rolagem CSS; por isso o ensaio passou a preparar o alvo com ações nativas de roda, mantendo o clique real e a exigência de visibilidade integral e de alvo correto no ponto do clique. O teste registra a geometria antes e depois. Um cenário independente percorre a grade por Tab, volta à primeira linha por Shift+Tab e verifica o retorno do editor em 360 e 1280 CSS px, sem essa preparação por roda. A CI 43215052002 confirmou a geometria inicial e a preparação por roda seguida do clique original. O cenário independente por Tab/Shift+Tab permaneceu posterior à falha funcional e não foi executado; a seção 6 limita a conclusão ao que foi observado.

### Resposta local da planilha na segunda execução

O [relatório com todas as observações](issue-004/ci-43208107786/forms-local-response.json) registra 20 alternâncias de seleção e 20 de expansão em cada perfil, após a hidratação. O limite de 100 ms foi aplicado a cada uma das 80 observações.

| Perfil | Ação | Observações | Mínimo | p75 | Máximo |
|---|---|---:|---:|---:|---:|
| Computador, 1366 × 768 CSS px | Seleção | 20 | 10,2 ms | 17,8 ms | 32,3 ms |
| Computador, 1366 × 768 CSS px | Expansão | 20 | 7,9 ms | 14,9 ms | 20,2 ms |
| Celular simulado, 360 × 800 CSS px | Seleção | 20 | 9,8 ms | 12,6 ms | 22,3 ms |
| Celular simulado, 360 × 800 CSS px | Expansão | 20 | 8,9 ms | 15,7 ms | 22,4 ms |

Esses valores medem o evento até o segundo quadro de animação, com efeito esperado confirmado no DOM. Não são INP de visitantes, latência da ferramenta nem medida do instante físico de exibição. São distintos das 80 navegações da fundação. Nesse outro conjunto, o [relatório da rota `/`](issue-004/ci-43208107786/browser-performance.json) registra máximos entre grupos de 580 ms de LCP p75, 1.637,1 ms até hidratação p75, 31,9 ms de resposta controlada p75 e CLS p75 zero.

### Orçamento e verificações locais da segunda fonte

O [orçamento da CI](issue-004/ci-43208107786/budget.json) registrou 579.507 B de WASM Brotli, 7.955 B de JavaScript externo Brotli e 6.687 B de CSS Brotli. Somado ao inline da rota `/`, o JavaScript foi de 8.624 B. A [prova local da mesma árvore](issue-004/ci-43208107786/worksheet-payload-local.json) capturou a última página de 25, 50 e 100 linhas em ambas as empresas, todas ativas e sem linhas inertes. O maior HTML entre essas seis respostas foi de 125.586 B; o maior conteúdo inline bruto foi de 29.083 B.

A [soma derivada para as seis respostas](issue-004/ci-43208107786/worksheet-budget.json) é de **37.038 B de JavaScript**, combinando 29.083 B inline com 7.955 B externos Brotli, abaixo do limite de 81.920 B. O cálculo identifica os hashes das duas fontes e não é uma medição de carregamento nem uma prova de todas as buscas possíveis. A [verificação local da serialização](issue-004/ci-43208107786/serialization-safety.json) da mesma árvore preserva as 12 respostas examinadas: cinco vetores distintos não vazios aplicados às duas empresas, mais duas referências vazias. Ela é complementar à CI e tem o alcance descrito na seção 8.

O [relatório do protótipo](issue-004/ci-43208107786/prototype-functional.json) aprovou a automação existente de contexto, teclado, semântica e recuperação. Ele declara expressamente que não houve leitor de tela nem participação dos três profissionais. A aprovação automatizada não resolve a pendência de aceite da issue 3.

## 6. Execução da fonte 8ff30342: acesso ao editor e falha de preenchimento

A [CI 43215052002](https://github.com/tjsasakifln/canteiro-360/actions/runs/43215052002) executou a fonte `8ff3034252cfef06268576ad273be28422c0130f`, árvore `1f22073b21f0dd90c5f17dff63210e9268b11e39`. O log confirma o checkout `2b81444c6dbd1183140c23c2415762fa806f1cf0`; sua árvore coincide com a fonte, conforme a API do GitHub. O [registro da execução](issue-004/ci-43215052002/run.json) identifica o job `131172197268` e o artefato `13944064545`. O ZIP foi conferido pelo SHA-256 `f9d853bd3cdc232465f0ed0b169a78e4ed388990609c8245df85386e635b0ff9`. O [manifesto](issue-004/ci-43215052002/manifest.json) separa 19 arquivos extraídos do ZIP, o log original obtido separadamente e cinco provas ou metadados locais.

| Verificação nessa execução | Resultado constatado |
|---|---|
| Formatação, regras, Clippy, dependências, compilação e orçamento padrão | Aprovados |
| Testes nativos e PostgreSQL | 60 nativos e 15 PostgreSQL aprovados; 21 ignorados somente no comando básico |
| Navegador da fundação e do protótipo | Dois testes da fundação, com 80 navegações, e um do protótipo aprovados |
| Planilha, apresentação/SSR e resposta local | Dois grupos aprovados; 80 ações locais sem violação do limite de 100 ms |
| Planilha, rascunhos e contexto | Falhou ao conferir o texto do preço preenchido no editor |
| Conclusão da CI | Falhou; cinco dos seis testes de navegador passaram, sem aceite integral |

O [grupo de apresentação](issue-004/ci-43215052002/forms-presentation.json) repetiu a confirmação SSR, a navegação sem scripts e as regras de consulta e cálculo. Os registros de geometria em [360 px](issue-004/ci-43215052002/forms-grid-geometry-360.json) e [1280 px](issue-004/ci-43215052002/forms-grid-geometry-1280.json) confirmam que identificação, descrição, unidade e edição da primeira linha cabem horizontalmente sem deslocamento lateral; seleção e edição têm alvos de pelo menos 44 CSS px. Essa verificação inicial não equivale ao percurso final por Tab/Shift+Tab.

A revisão visual encontrou uma área vazia extensa depois do rodapé nas capturas de [1280 px](issue-004/ci-43215052002/forms-table-1280.png) e [360 px](issue-004/ci-43215052002/forms-table-360.png), com alturas totais de 5.524 e 7.395 px, respectivamente. As asserções geométricas dessa execução não detectavam esse aumento da altura da página. A contenção dos elementos visualmente ocultos nos rótulos e uma verificação específica da área após o rodapé foram implementadas na fonte b6dc4082 e verificadas pela CI 43220954664, conforme a seção 7. Essa confirmação específica não elimina a falha posterior de navegação.

O [registro da preparação por roda](issue-004/ci-43215052002/forms-pointer-scroll.json) passou com visibilidade integral e o botão de edição como alvo no centro do clique. O clique WebDriver original abriu o editor. A [falha funcional](issue-004/ci-43215052002/forms-functional.json), registrada em `tools/e2e/tests/forms.rs:566:5`, ocorreu depois: o texto do preço era `101.234,56`, em vez de `1.234,56`. O snapshot também contém `3535` no acumulado e foco na observação. A [captura correspondente](issue-004/ci-43215052002/forms-functional-failure.png) conserva esse estado. O comportamento de `Browser::fill`, que nessa fonte usava `clear` antes da digitação, motivou a inspeção estática e o ajuste posterior descritos na seção 8; a asserção esperada foi preservada. Correção dos campos, respostas atrasadas, falhas de rede, troca de empresa e regressão final por teclado não foram concluídas nessa execução.

O [relatório de resposta local](issue-004/ci-43215052002/forms-local-response.json) contém 80 observações e nenhuma violação, com máximo de 37,0 ms. Os p75 de seleção e expansão foram, respectivamente, 19,9 e 17,5 ms no computador, e 17,3 e 17,1 ms no celular simulado. São as mesmas condições e o mesmo método laboratorial da seção 9, sem atribuição de INP real.

O [orçamento desta CI](issue-004/ci-43215052002/budget.json) registrou 580.021 B de WASM Brotli, 7.955 B de JavaScript externo Brotli e 6.778 B de CSS Brotli. Com as [seis respostas locais já medidas dessa árvore](issue-004/ci-43215052002/worksheet-payload-local.json), o maior HTML foi de 133.780 B e o maior inline bruto de 29.083 B. A [soma derivada](issue-004/ci-43215052002/worksheet-budget.json) continua em 37.038 B de JavaScript para essas amostras. Nenhum novo probe foi executado nesta consolidação. As provas anteriores de serialização e de correspondência da fonte foram preservadas na mesma pasta; uma nova fonte exigirá atribuição explícita das verificações aplicáveis.

## 7. Execução da fonte b6dc4082: campos e altura confirmados, falha no histórico

A [CI 43220954664](https://github.com/tjsasakifln/canteiro-360/actions/runs/43220954664) executou a fonte `b6dc408221b0bc5a1e71da01947e277c2380f170`, árvore `48046848edf6c004b91987c5eaee0f8b3e53b933`. O log confirma o checkout `4c6097ba480045dd1dd9f377cba0742a23fa5ec6`, cuja árvore foi conferida pela API. O [registro da execução](issue-004/ci-43220954664/run.json) identifica o job `131192033151`, o artefato `13946404607` e o SHA-256 do ZIP `26ddd80f1539f717337c3906606c4a4ce204c2d3dfc9822d2eb2b58bc32d6223`. O [manifesto](issue-004/ci-43220954664/manifest.json) distingue 21 arquivos originais selecionados, um log de job obtido separadamente e cinco provas ou metadados locais, sem HTML com scripts.

Formatação, regras, Clippy, dependências, compilação e orçamento padrão passaram. Foram aprovados 60 testes nativos, 15 PostgreSQL e cinco dos seis testes de navegador: dois da fundação, um do protótipo e os grupos de apresentação/SSR e resposta local da planilha. O grupo de rascunhos e contexto falhou. Os 21 testes ignorados no comando básico foram executados explicitamente em seus grupos.

| Cenário dessa execução | Resultado e evidência |
|---|---|
| Preenchimento e recuperação de erros | A fase `exact_fields_and_error_recovery` terminou antes da falha seguinte. Confirmou texto exato, interpretações, observação condicional, data impossível, valor ambíguo, foco, preservação dos demais campos, limite de quantidade, prévia e salvamento local. |
| Altura após o rodapé | A nova regressão passou em 360 e 1280 CSS px, com alturas de 3.134 e 2.077 px. |
| Retorno pelo histórico | Falhou após `history_back`: a URL voltou à planilha, o DOM permaneceu em Documentos e a espera pela planilha pronta terminou em dez segundos. |
| Cenários posteriores | Respostas atrasadas, falhas reais de rede, troca de empresa e percurso final da grade por Tab/Shift+Tab não executaram. |

A conclusão da fase de campos é uma inferência da ordem sequencial da fonte testada e do ponto do timeout seguinte, apoiada pela [captura do valor ambíguo](issue-004/ci-43220954664/forms-ambiguous-error-1280.png) e pela [captura do rascunho salvo](issue-004/ci-43220954664/forms-draft-saved.png). O erro anterior de `101.234,56` no preenchimento não se repetiu nesse cenário. Salvar o rascunho continua sendo uma ação local da aba, sem persistência comercial.

Os registros de altura de [360 px](issue-004/ci-43220954664/forms-document-height-360.json) e [1280 px](issue-004/ci-43220954664/forms-document-height-1280.json) conferem documento, corpo e conteúdo do navegador contra o rodapé e a tolerância de 32 CSS px. As capturas de [360 px](issue-004/ci-43220954664/forms-table-360.png) e [1280 px](issue-004/ci-43220954664/forms-table-1280.png) têm 3.134 e 2.077 px de altura, respectivamente, em vez dos 7.395 e 5.524 px observados na execução anterior. O [grupo de apresentação](issue-004/ci-43220954664/forms-presentation.json) também repetiu os critérios de SSR, consulta, cálculo, rótulos e geometria inicial.

O [relatório funcional da falha](issue-004/ci-43220954664/forms-functional.json) preserva a URL de planilha, o título de Documentos, a ausência de `forms-shell` pronto e seis registros de console. O console inclui acesso a valor reativo descartado e uma chamada de `RwSignal<bool>::get`. A [captura da falha](issue-004/ci-43220954664/forms-functional-failure.png) mostra o conteúdo de Documentos. A recuperação ficou pendente nessa execução, que não comprovou a preservação do rascunho depois do retorno. A confirmação posterior pertence à fonte e à CI da seção 10.

O [grupo de resposta local](issue-004/ci-43220954664/forms-local-response.json) aprovou as 80 observações, com máximo de 30,2 ms. Os p75 de seleção/expansão foram 17,5/16,6 ms no computador e 15,6/16,5 ms no celular simulado. O [orçamento](issue-004/ci-43220954664/budget.json) registrou 580.021 B de WASM Brotli, 7.955 B de JavaScript externo Brotli e 6.780 B de CSS Brotli. A [soma derivada para as seis respostas locais da mesma árvore](issue-004/ci-43220954664/worksheet-budget.json) foi de 37.038 B de JavaScript. As provas locais foram apenas copiadas para preservar essa fonte; nenhum novo probe foi executado nesta consolidação.

## 8. Revisão adversarial e correções

As revisões de contratos/API, interface e ensaios foram distribuídas separadamente. A leitura do código, a confirmação local de pontos específicos e o resultado integrado do navegador são evidências distintas. Uma correção no código não será descrita como reteste de navegador aprovado antes de sua execução.

| Achado ou risco examinado | Correção ou decisão aplicada | Verificação disponível |
|---|---|---|
| Prévia local poderia construir a massa completa de duas empresas. | `demo_row` constrói somente o serviço solicitado; a massa permanece nas consultas do servidor. | Revisão independente e testes de contratos. |
| Valor do período poderia perder centavos pela multiplicação isolada do incremento. | Diferença entre os valores acumulados truncados, com vetor PU 0,335. | Testes de contratos e HTTP. |
| Troca de unidade ou preço sem contexto poderia produzir interpretação enganosa. | Unidade deve coincidir com o serviço; alteração do preço simulado exige observação e não modifica contrato. | Validação comum ao cliente e servidor; percurso de interface aprovado na CI 43227797776. |
| Erros insuficientes ou coerção silenciosa. | Mensagens identificam limite, saldo, unidade e correção; os campos originais são preservados. | Vetores negativos, limites de consulta e teste HTTP. |
| Conferência antiga poderia habilitar uma cópia de dados já modificados ou de um editor reaberto. | Toda alteração invalida a conferência; a resposta exige instância ativa, mesma empresa, linha e entrada. O foco adiado repete a guarda e exige conferência válida. | Teste nativo de estado, revisão independente e ensaios reais de prévia atrasada e desmontagem aprovados na CI 43227797776. |
| Final de consulta poderia interromper uma edição iniciada durante a espera. | A guarda de foco só move ao resumo quando o foco ainda está no corpo ou em controles de consulta/paginação/retentativa. | GET atrasado aprovado na CI 43227797776, com entrada, nota e foco preservados após a carga da página 2. |
| HTML bruto poderia aparentar SSR completo com linhas invisíveis. | Modo `Async`, verificações de template, fallback e `aria-busy`, mantendo o ensaio sem scripts. | 11 testes HTTP e Clippy após a correção; navegador sem scripts aprovado na segunda CI. |
| Uma falha precoce ocultava resultados independentes da planilha e impedia o protótipo seguinte. | Três testes separados para a planilha e `--no-fail-fast` no comando de navegador. | Na segunda CI, dois grupos de forms e o protótipo seguinte passaram apesar da falha funcional; a CI terminou com falha. |
| Cabeçalho fixo interceptou o clique de edição após a seleção. | Margens de rolagem, larguras e áreas de seleção ajustadas; ensaio conserva clique real, registra a geometria e acrescenta percurso por teclado. | A CI 43227797776 confirmou preparação por roda e clique, além do percurso independente por Tab/Shift+Tab em 360 e 1280 CSS px. |
| Preenchimento automatizado conservou o valor anterior antes do texto esperado. | Inspeção do `CLEAR` fixado; fonte b6dc4082 substitui esse comando por edição nativa de teclado e confere foco, esvaziamento e texto final. | Na CI 43220954664, o cenário de preenchimento e correção anterior ao histórico concluiu suas asserções; a jornada completa falhou posteriormente. |
| A página passou nas asserções de largura, mas apresentou área vazia extensa após o rodapé. | Fonte b6dc4082 contém os elementos visualmente ocultos nos rótulos e acrescenta verificação da altura após o rodapé. | A regressão de altura e as capturas da CI 43220954664 confirmaram a correção nas duas larguras. |
| Retorno pelo histórico manteve Documentos no DOM apesar da URL de planilha. | Fonte 5588b0c6 faz a transição de prontidão somente de `false` para `true`, evitando notificação redundante durante navegação. | Hipótese causal demonstrada na reprodução nativa, sem identificar diretamente o bool da CI anterior. Histórico e preservação do rascunho aprovados no navegador da CI 43227797776. |
| Busca serializada poderia encerrar o script de hidratação e criar elemento ativo. | Inspeção da dependência fixada e probe nativo com cinco vetores distintos nas duas empresas. | Árvores c13d38a4, 1f22073b, 48046848 e ccdc477a verificadas separadamente em 12 respostas cada; nenhuma criou o elemento marcado ou um script adicional. |
| Cópia e exportação poderiam aparentar incluir rascunhos gravados. | A base de referência e a cópia local são identificadas; seleção da página e do conjunto completo têm contagens e ações distintas. | Teste de TSV, revisão dos rótulos e jornada integral de seleção/cópia aprovados na CI 43227797776. |

Também foram tratados o limite de busca, a abertura das opções avançadas para corrigir observações, o retorno de foco, a entrada de data com barra em teclado móvel e a leitura pontual do rascunho sem clonar todas as edições a cada tecla.

Durante a correção SSR, os 11 testes HTTP, a análise Clippy de API/SSR e de WASM, a formatação e a conferência de espaços do diff passaram localmente. A asserção de `aria-busy="false"` foi novamente executada após sua inclusão. Esses resultados locais não substituem a execução completa da revisão identificada na seção 1.

### Fonte b6dc4082 e resultados observados

A comparação com a fonte 8ff30342 identifica alterações em somente três arquivos: `style/prototype.css`, `tools/e2e/tests/support/mod.rs` e `tools/e2e/tests/forms.rs`. A [inspeção mínima do comando CLEAR](issue-004/chromedriver-clear-inspection.json) registra que o ChromeDriver fixado atribui um valor vazio diretamente, emite `change` sem `input` e transfere o foco ao corpo do documento. Isso é compatível com a restauração do estado anterior por um campo controlado antes da digitação seguinte. A inspeção é estática e não comprova o resultado de execução do ajuste.

O auxiliar de preenchimento agora usa clique, Ctrl+A e Backspace, verifica que o campo ficou vazio e manteve foco, procura novamente o elemento e digita o texto solicitado. Ao final, exige igualdade exata do texto. A regra de interpretação brasileira e as asserções da jornada permanecem as mesmas. No retorno pelo histórico da empresa, o teste também aguarda a página consultada estar pronta.

O rótulo de seleção recebeu `position: relative` para conter o elemento de texto visualmente oculto. A nova regressão compara as alturas do documento, do corpo e do conteúdo informado pelo navegador com o limite formado pelo rodapé ou pela janela, admitindo somente 32 CSS px adicionais. Ela grava medidas separadas em 360 e 1280 CSS px antes das capturas. A CI 43220954664 confirmou a regressão de altura e avançou pelo preenchimento e salvamento local. Ela falhou no retorno pelo histórico; a preservação do rascunho após essa navegação só foi confirmada na execução posterior da fonte 5588b0c6.

### Fonte 5588b0c6: hipótese causal e proteção da instância do editor

Em relação à fonte b6dc4082, a nova árvore altera somente `crates/web/src/prototype/mod.rs`, `crates/web/src/prototype/worksheet/editor.rs` e `tools/e2e/tests/forms.rs`. As duas escritas de prontidão da página agora ocorrem somente quando o valor ainda é `false`. A primeira transição continua no efeito após a hidratação; o SSR conserva seu estado inicial. A guarda de empresa, a restauração da URL, o pedido de confirmação e o descarte autorizado continuam sendo executados na navegação.

A [reprodução nativa preservada](issue-004/lifecycle-probe/README.md) usa a biblioteca instalada e um percurso reduzido com `Resource` e efeitos de 25 controles. A mesma fonte, SHA-256 `8406e0d30a933d4e1e4b0b1c5e6bb6d469ffa2cb48fc3e050243e7c73c6611ba`, foi executada em quatro variantes. A [comparação original](issue-004/lifecycle-probe/comparison.json) e sua [conferência](issue-004/lifecycle-probe/verification.json) registram:

| Variante da reprodução | Panics em efeitos/tarefas | Limpezas da planilha | Limpezas do shell |
|---|---:|---:|---:|
| Padrão | 50 | 2 | 1 |
| Somente proprietário estável | 25 | 2 | 1 |
| Somente prontidão idempotente | 0 | 2 | 1 |
| Ambas as alterações | 0 | 2 | 1 |

A variante padrão registra 25 panics ao ir para Documentos e outros 25 ao sair para Home. Fixar somente o proprietário conserva as falhas na saída para Home. Evitar a notificação redundante de prontidão elimina esses registros no programa reduzido, mantendo as limpezas. O critério é a contagem dos panics, das limpezas e sua ordem nos logs; o processo pode continuar após o panic de uma tarefa, portanto seu código de saída não determina aprovação.

Essa diferença demonstra a hipótese causal no laboratório: uma notificação redundante pode deixar efeitos agendados que leem sinais já descartados. O log da CI 43220954664 identifica `RwSignal<bool>::get`, mas não identifica diretamente qual variável booleana produziu a falha. A reprodução não executa o DOM real nem permite equiparar seus 50 registros aos três panics daquela CI. A CI 43227797776 confirmou separadamente o retorno da interface, a continuidade do rascunho e o contexto, sem identificar retroativamente a variável da falha antiga.

No editor, cada montagem recebe uma guarda própria desativada na limpeza. Uma resposta de prévia só pode atualizar a conferência enquanto a instância estiver ativa e empresa, serviço e entrada coincidirem. O callback de foco repete as condições e exige conferência válida para a entrada atual. Isso impede que um POST pendente da instância encerrada certifique outra montagem com valores idênticos, preservando o rascunho existente da aba. Não há nova autorização comercial nem persistência no servidor.

O ensaio de desmontagem e reabertura exige POST real atrasado, ausência do editor durante a visita a Documentos, retorno à mesma consulta e campos, resposta concluída sem conferência nem foco indevidos e recuperação por uma nova prévia explícita. A regressão da grade por Tab/Shift+Tab foi movida para o grupo de apresentação, mantendo as asserções nas duas larguras e tornando seu resultado independente da jornada funcional. A observação do foco e da geometria do link de salto e a ampliação da pilha de erro no laboratório melhoram o diagnóstico. Os dois grupos concluíram seus percursos na CI 43227797776, cujos resultados estão na seção 10.

### Verificações locais complementares da fonte 5588b0c6

A [medição das respostas da planilha](issue-004/worksheet-payload-local.json) foi repetida na fonte `5588b0c695c13259e6e19b0a5a1256e39112a6c8`, árvore `ccdc477a623394a0af87450cc64a406f3f7821d1`, usando o router Rust real e a última página de 25, 50 e 100 serviços nas duas empresas. O teste nativo passou; as seis respostas contêm somente as linhas ativas solicitadas, sem linhas em template. Os maiores valores entre essas seis amostras são 133.780 B de HTML bruto e 29.083 B de conteúdo inline bruto. A soma com arquivos externos da mesma CI está registrada na seção 10. Esses números não medem tempo de carregamento nem representam todas as buscas possíveis. A [prova anterior](issue-004/ci-43220954664/worksheet-payload-local.json) permanece preservada sem alterações.

A [verificação da serialização da busca](issue-004/serialization-safety.json) foi repetida na mesma fonte e árvore. Ela inspeciona a dependência fixada e 12 respostas reais do router: cinco vetores fictícios distintos não vazios, aplicados às duas empresas, produzem dez respostas; somam-se duas referências com busca vazia. O teste nativo e as sete verificações de cada resposta passaram. As 12 respostas mantiveram os dados idênticos após desserialização, sem criar o elemento marcado pelos vetores nem acrescentar scripts ao HTML. Os dois caminhos examinados em `hydration_context 0.3.2` escapam o caractere de abertura de tag antes da emissão dos dados serializados. O exame se limita a esse caminho, sem executar os vetores no navegador ou realizar auditoria geral de segurança. Os dois testes temporários foram removidos e a implementação rastreada permaneceu inalterada. A [prova da fonte b6dc4082](issue-004/ci-43220954664/serialization-safety.json) permanece no histórico; não foi sobrescrita.

## 9. Protocolo de navegador executado na fonte 5588b0c6

O conjunto da fonte 5588b0c6 conserva **seis testes de navegador**: dois da fundação, um do protótipo e três da planilha. No comando nativo básico há **21 testes ignorados**, correspondentes a 15 cenários de PostgreSQL e esses seis testes. Ignorado não significa aprovado; os 15 testes de banco e os seis de navegador foram executados explicitamente e passaram nesta CI. Os resultados da fonte b6dc4082 permanecem limitados à execução histórica da seção 7.

| Grupo da planilha | Conteúdo | Relatório | Resultado atual |
|---|---|---|---|
| Apresentação, HTTP e SSR | Página limitada, busca e ordenação, valores exatos, erros, leitura e paginação sem scripts, 360 e 1280 CSS px, nomes, semântica, altura e percurso da grade por Tab/Shift+Tab. | `forms-presentation.json` | Aprovado na CI 43227797776 |
| Resposta local | Seleção da página e expansão das opções, medidas separadas após hidratação. | `forms-local-response.json` | Aprovado na CI 43227797776 |
| Rascunhos e contexto | Teclado, seleção/cópia, campos, erros, histórico, resposta de editor desmontado, demais respostas atrasadas, falhas reais de rede, troca de empresa e descarte decidido. | `forms-functional.json` | Aprovado na CI 43227797776 |

Os três grupos usam um controle de execução exclusivo dentro do binário, evitando medir resposta local em paralelo com seus próprios percursos funcionais. Cada grupo grava resultado e evidências de falha próprios. `--no-fail-fast` permite executar os demais binários mesmo quando um falha; uma falha continua produzindo resultado final negativo e não é ocultada.

O ensaio de resposta local prevê 20 alternâncias de seleção e 20 de expansão em cada um dos dois perfis, computador e celular simulado. São 80 observações de ações, distintas das 80 navegações da fundação. O observador mede do `event.timeStamp` até o segundo `requestAnimationFrame`, confirma o efeito no DOM e não inclui o percurso da chamada do WebDriver. A condição usa redução de CPU de quatro vezes, latência de 150 ms e rede de 4 Mbps de descida e 1 Mbps de subida. O limite de 100 ms é conferido por amostra; isso é uma aproximação laboratorial de resposta visual, não uma medida de INP real ou do instante físico de exibição.

Os testes de resposta atrasada mantêm requisições HTTP reais com latência de 1.200 ms. Um deles altera o acumulado de 36 para 37 enquanto a prévia de 36 está pendente e exige recusa da confirmação antiga. O outro inicia o GET da segunda página, digita 38 no editor já aberto e verifica foco, conteúdo e página carregada após a resposta e duas animações. Os relatórios devem demonstrar que o atraso realmente ocorreu; não basta afirmar a configuração.

O novo ensaio de desmontagem acrescenta uma condição diferente: envia 36 com latência configurada de 4.000 ms, visita Documentos e volta ao editor com os mesmos valores antes da conclusão. Exige observação do foco enquanto o POST ainda está pendente, resposta HTTP 200 com duração de pelo menos 3.000 ms e, após duas animações, ausência de conferência aproveitada, cópia habilitada ou foco roubado pela resposta antiga. Uma nova prévia explícita de 36 deve recuperar 16 m² no período e R$ 160,00. O relatório `forms-unmounted-preview.json` deve registrar essas etapas; sua existência parcial não basta para considerar o cenário aprovado.

## 10. Resultado final da CI 43227797776

O [log original do job](issue-004/ci-43227797776/job.log) confirma o checkout completo `7f4ad5576ac14c373c13de965a8822e19ae948be`, os resultados dos comandos e a publicação do artefato `13950282541`. O ZIP de 6.688.584 B contém 106 arquivos e foi conferido pelo SHA-256 `dd1f884fed3739eb42848c12c3f14392c1a2fb35b8edc69f9bc488adbca7081f`. O log do job foi obtido separadamente e não pertence a esse ZIP. O [registro da execução](issue-004/ci-43227797776/run.json) e o [manifesto da seleção](issue-004/ci-43227797776/manifest.json) preservam essa distinção.

| Verificação da fonte 5588b0c6 | Resultado observado |
|---|---|
| Fonte, checkout e igualdade das árvores | Commit completo no log; árvore `ccdc477a623394a0af87450cc64a406f3f7821d1` igual à fonte publicada, conferida pela API |
| Formatação, regras e Clippy nativo/WASM | Aprovados |
| Testes nativos | 60 aprovados, zero falhas; 21 ignorados somente no comando básico |
| PostgreSQL real | 15 aprovados na execução explícita; migrações e identidade de execução verificadas |
| Auditoria de dependências e licenças | Comando de auditoria aprovado |
| Compilação SSR/WASM e orçamento | Aprovados, conforme [build](issue-004/ci-43227797776/build.json) e [orçamento](issue-004/ci-43227797776/budget.json) |
| Navegador da fundação | Dois testes aprovados, incluindo 80 navegações da rota `/` |
| Navegador do protótipo | Um teste aprovado, conforme [relatório](issue-004/ci-43227797776/prototype-functional.json) |
| Navegador da planilha | Três grupos aprovados; apresentação, jornada funcional e 80 ações locais |
| Total do navegador | Seis testes aprovados, zero falhas |
| Capturas, relatórios e rastreabilidade | Seleção conferida com o ZIP, log separado, provas locais identificadas e revisão independente dos resultados |
| Aceite da dependência da issue 3 | Pendente: pelo menos três profissionais reais; PR 75 e issue 3 continuam abertas |

### Rascunhos, respostas atrasadas e contexto

O [ensaio de prévia após desmontagem](issue-004/ci-43227797776/forms-unmounted-preview.json) registra a ausência de requisições concluídas antes e depois da definição do foco na nova montagem. O POST antigo terminou com HTTP 200 em 4.007,1 ms. Depois da resposta e de duas animações, os seis campos, a URL e a consulta permaneciam iguais; `form-current` conservava o foco, não havia comparação e a cópia local continuava desabilitada. A nova prévia explícita confirmou 16 m² e R$ 160,00. A [captura desse estado](issue-004/ci-43227797776/forms-unmounted-preview-ignored.png) complementa o relatório.

O [ensaio de alteração durante a prévia](issue-004/ci-43227797776/forms-delayed-preview.json) enviou 36, digitou 37 enquanto aguardava e recebeu HTTP 200 após 1.213,7 ms sem aplicar a resposta antiga. O [ensaio da consulta atrasada](issue-004/ci-43227797776/forms-delayed-table-focus.json) carregou a segunda página em 1.205,8 ms, com primeira linha 26 e `busy=false`, preservando a entrada 38, a observação e o foco em `form-current`. Esse segundo JSON comprova carga e estado resultante, mas não registra diretamente o código HTTP do GET.

O grupo funcional completo passou, incluindo histórico, cancelamento da troca de empresa, confirmação do descarte e recuperação de falhas reais da prévia e da consulta. O [rascunho salvo na aba](issue-004/ci-43227797776/forms-draft-saved.png), a [falha de rede com entradas preservadas](issue-004/ci-43227797776/forms-preview-network-error.png) e o [contexto Horizonte após a decisão](issue-004/ci-43227797776/forms-context-horizonte.png) mostram etapas distintas dessa jornada. Nada nesse percurso grava uma medição comercial.

### Apresentação, teclado e capturas

A seleção preserva 14 capturas da planilha, incluindo a [consulta sem scripts](issue-004/ci-43227797776/forms-ssr-without-scripts.png), as tabelas de [360 px](issue-004/ci-43227797776/forms-table-360.png) e [1280 px](issue-004/ci-43227797776/forms-table-1280.png), o [editor em 360 px](issue-004/ci-43227797776/forms-edit-360.png), o [erro de entrada ambígua](issue-004/ci-43227797776/forms-ambiguous-error-1280.png) e a [seleção com escopo explícito](issue-004/ci-43227797776/forms-selection-scope.png). O manifesto identifica todas as imagens e seus hashes. Relatórios e imagens repetidos do protótipo, logs auxiliares e HTML com scripts não foram copiados para esta seleção.

O [percurso independente da grade por teclado](issue-004/ci-43227797776/forms-sticky-controls.json) chegou à linha 25 por Tab e voltou à linha 1 por Shift+Tab, em 360 e 1280 CSS px. O botão permaneceu integralmente visível, com contorno de foco de 3 px e recebendo o clique no próprio centro; o retorno do editor preservou o foco. Esse ensaio não usou preparação por roda. As capturas de [360 px](issue-004/ci-43227797776/forms-sticky-edit-360.png) e [1280 px](issue-004/ci-43227797776/forms-sticky-edit-1280.png) acompanham a geometria. A preparação por roda da jornada de ponteiro continua registrada separadamente em [forms-pointer-scroll.json](issue-004/ci-43227797776/forms-pointer-scroll.json).

As medidas de [altura em 360 px](issue-004/ci-43227797776/forms-document-height-360.json) e [1280 px](issue-004/ci-43227797776/forms-document-height-1280.json) repetiram 3.134 e 2.077 px, dentro do limite formado pelo rodapé mais 32 CSS px. As capturas de página inteira mostram uma posição aparente do link de salto compatível com sua composição durante a captura. No estado de correção efetivamente medido, a [geometria e o foco](issue-004/ci-43227797776/forms-error-focus-geometry.json) registram `form-money` focado, `skipLinkFocused=false` e o link acima da janela, entre -67,2 e -15,2 CSS px. Essa observação específica não é uma medição geral de todos os estados do link e não deve ser substituída pela aparência isolada da captura inteira.

### Resposta local e desempenho da fundação

O [relatório de resposta local](issue-004/ci-43227797776/forms-local-response.json) conserva as 80 amostras, todas com efeito esperado observado e nenhuma acima de 100 ms. O máximo foi de 40,7 ms.

| Perfil | Ação | Observações | Mínimo | p75 | Máximo |
|---|---|---:|---:|---:|---:|
| Computador | Seleção | 20 | 21,2 ms | 25,4 ms | 40,7 ms |
| Computador | Expansão | 20 | 17,9 ms | 23,4 ms | 27,5 ms |
| Celular simulado | Seleção | 20 | 20,3 ms | 22,7 ms | 33,4 ms |
| Celular simulado | Expansão | 20 | 18,1 ms | 21,1 ms | 27,7 ms |

Na rota `/`, o [relatório de desempenho da fundação](issue-004/ci-43227797776/browser-performance.json) registra outras 80 navegações, separadas por perfil e condição de cache. As [amostras brutas dessa fonte](issue-004/ci-43227797776/performance-samples.json) foram preservadas. Entre os quatro grupos, os máximos de p75 foram 644 ms para LCP, 1.697,5 ms até hidratação e 23,7 ms para resposta controlada; CLS p75 foi zero em todos. Esses tempos não são atribuídos à planilha nem apresentados como INP de visitantes. O ambiente compartilhado informou dois processadores disponíveis, sem reserva exclusiva de quatro vCPUs ou aparelho móvel físico.

### Orçamento da mesma fonte

O [orçamento da CI](issue-004/ci-43227797776/budget.json) e a [soma derivada para a planilha](issue-004/ci-43227797776/worksheet-budget.json) referem-se à mesma árvore. A soma usa os seis HTMLs locais já medidos; não houve nova execução dos probes nesta consolidação.

| Recurso e escopo | Tamanho contabilizado | Limite |
|---|---:|---:|
| WASM Brotli | 580.238 B | 1.048.576 B |
| CSS Brotli | 6.780 B | 61.440 B |
| JavaScript externo Brotli mais inline bruto da rota `/` | 8.616 B | 81.920 B |
| JavaScript externo Brotli mais maior inline bruto das seis amostras de planilha | 37.030 B | 81.920 B |

Para a planilha, **29.083 B inline + 7.947 B externos Brotli = 37.030 B**. O maior HTML bruto entre as seis respostas é de 133.780 B. A soma identifica os hashes dos dois relatórios de origem e não representa tempo de carregamento nem cobertura de todas as buscas possíveis.

### Correspondência entre fonte e documentação

Os resultados finais mantêm separadas as quatro execuções concluídas que falharam e a versão intermediária cancelada. O [manifesto geral](issue-004/manifest.json) registra a seleção final, as origens, os hashes e a equivalência dos arquivos de implementação e testes com a fonte aprovada. A identificação deste registro é a fonte efetivamente testada. Uma publicação apenas documental precisa conservar a equivalência conferida pelo manifesto.

O [manifesto da fonte 5588b0c6](issue-004/source-manifest.json) registra os 171 arquivos rastreados fora de `docs/validacao/`, conferidos com a árvore `ccdc477a623394a0af87450cc64a406f3f7821d1`. Seu SHA-256 agregado é `a89388632946a474db2cb98af31d2c5dad158738d174f96aa7c1188a3f657507`. O [manifesto da fonte b6dc4082](issue-004/ci-43220954664/source-manifest.json) permanece preservado com o agregado `d8acfb02f50d449e80b15a4a45bd964c14a1d7c3ef3065c3ad0b065aded8b1e0`. A execução e sua publicação documental precisam conservar a correspondência entre código, testes e evidências, sem transferir automaticamente os resultados anteriores.

## 11. Comandos reproduzíveis

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
cargo test --locked -p canteiro-e2e --no-fail-fast -- --ignored --nocapture
```

Os pré-requisitos de banco, identidades, navegador e driver estão em [Desenvolvimento](../desenvolvimento.md). A CI aprovada usou PostgreSQL 17.7, Rust 1.99.1, `wasm-bindgen 0.2.130` e Chrome/ChromeDriver 156.0.8078.80. As cinco execuções completas registradas informaram dois processadores disponíveis e ambiente compartilhado, sem comprovar reserva exclusiva de quatro vCPUs.

## 12. Limites e decisão de aceite

As APIs são demonstrações sem gravação. Não há autenticação real, autorização comercial, rascunho persistido no banco, auditoria de ato financeiro, medição aprovada ou reconhecimento de dívida. Nenhuma migração nova é necessária para essa etapa porque os exemplos são imutáveis e os rascunhos pertencem à aba. Os testes PostgreSQL preservam a verificação da fundação existente.

Recarregar ou fechar a aba pode perder os rascunhos. O aviso de saída não é armazenamento durável; a cópia local não é confirmação de transação no servidor. O armazenamento para uso com conexão intermitente pertence à issue 62. A prévia exata implementada aqui não declara concluído o motor decimal e de calendários da issue 5.

A grade técnica usa uma região própria de rolagem horizontal; os demais elementos se reorganizam nas larguras verificadas. A automação de semântica e teclado e as verificações de contraste do protótipo não equivalem a uso com leitor de tela, aparelho Android físico, zoom manual de 400%, teste com profissionais ou declaração integral de WCAG. Tampouco os tempos da fundação substituem as medidas da planilha ou a validação integrada prevista nas etapas posteriores.

**A fonte 5588b0c6 está aprovada tecnicamente no escopo verificado.** A issue 4 permanece dependente do aceite da issue 3 com três profissionais reais, sem merge ou fechamento nesta consolidação. Os limites operacionais e humanos acima permanecem; a liberação comercial completa continua vinculada à issue 72.
