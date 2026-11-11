# Issue 1: fundação Rust verificada

Data: 11/11/2026. Escopo: base compilável, página inicial, configuração e integração contínua. O resultado aprova esta etapa de implementação; a prontidão comercial continua na issue 72.

## Versão e evidência

- [Issue 1](https://github.com/tjsasakifln/canteiro-360/issues/1) e [PR 73](https://github.com/tjsasakifln/canteiro-360/pull/73).
- Código implementado: `41b4173acbd79e8b68e29a9f09bfdcf289b0f8b0`.
- [Execução limpa aprovada no GitHub Actions](https://github.com/tjsasakifln/canteiro-360/actions/runs/43045304102), job `130607544827`. O checkout do evento de PR testou a integração temporária `7885fe4b79c3050f4432cea0b1ed63199872e581`, baseada nesse código e na base `acc56fa602774f62b4d0a086fcb64fd94a744a37`.
- Artefato `13868181675`, SHA-256 do ZIP `8b651d5caeec5f28a3168ea85c0c3533309f0f2398498251d54f62fe866277dd`. O arquivo baixado teve o mesmo hash. Os JSON e as imagens estão preservados neste diretório, sem depender da retenção de 30 dias do Actions.
- O commit seguinte do PR acrescenta este registro, as evidências e ajustes de documentação/formatação. O código executável e os testes medidos permanecem os mesmos; a integração exige também aprovação da CI no último commit do PR.

## O que foi entregue

Workspace com domínio, aplicação, persistência, contratos, web, API e processamento. A interface usa Leptos com HTML renderizado por Axum e hidratação WASM. `xtask` coordena validação do plano, política de código, compilação, compressão e orçamento. O ensaio de navegador também é coordenado em Rust.

A página contém apresentação legível antes da hidratação, botão de detalhes operável após carregar, indicação de espera e orientação para recarregar quando a interação demora. Não apresenta cadastros, login ou transações como disponíveis. O executável de processamento informa que o consumidor ainda não existe, salvo a verificação explícita `--check`.

As configurações públicas de desenvolvimento e homologação não contêm credenciais. A configuração de persistência recebe o valor de fora e o oculta em diagnóstico e erros; nenhuma conexão de banco é aberta nesta etapa. `/healthz` informa somente que o processo HTTP está em execução.

## Critérios de aceite da issue

| Critério | Resultado e evidência |
|---|---|
| Checkout limpo compila servidor e WASM | Aprovado no executor novo do Actions. Compilação nativa `--release` e navegador `wasm32-unknown-unknown` com perfil `wasm-release`; [build.json](issue-001/build.json). |
| Conteúdo útil no HTML e interação sem divergência | Aprovado com scripts desativados e depois com hidratação real. Console normal e da recuperação vazios; [browser-functional.json](issue-001/browser-functional.json). |
| Teste quebrado impede aprovação | Teste temporário com asserção deliberadamente falsa retornou código 101. Removido o teste, a execução voltou a passar; [negative-probes.json](issue-001/negative-probes.json). |
| Segredo indevido e JS/TS próprio impedem aprovação | Configuração JSON aninhada com segredo sintético e arquivo TypeScript próprio foram recusados com código 1. Depois da remoção, política aprovada. Os casos de regressão também estão nos testes do `xtask`. |
| Versões, custo e tamanho registrados | Versões e medidas abaixo, com arquivos brutos preservados e condições do ensaio explícitas. |
| Evidência visual, teclado e revisão adversarial | Seis capturas, Tab/Enter/Espaço, foco, reajuste de conteúdo, falha real do WASM e recuperação. Revisão independente concluída e correções novamente verificadas. |

## Testes executados

A CI aprovou formatação, validação das 72 issues e 192 regras, política de linguagens/segredos, Clippy com avisos tratados como erro nos alvos nativo e WASM, auditoria de dependências/origens/licenças, compilação e orçamento.

Foram aprovados **28 testes nativos**: 2 de configuração, 5 HTTP, 1 de ocultação da configuração de persistência e 20 do `xtask`. Os testes HTTP incluem HTML útil, 404 para caminhos privados/inexistentes, recusa de método, resposta mínima de processo, compressão real do HTML e serviço dos arquivos Brotli sem alterar seus bytes.

As **duas suítes de navegador** passaram em execução explícita após o build. A marcação `ignored` no comando básico evita depender de navegador em todo teste unitário; a CI executa ambas com `--ignored`, e a falta do navegador, driver ou artefato é falha. Foram verificadas:

- Leitura da página sem scripts, hidratação sem divergência e console sem erro na condição normal.
- Abertura e fechamento dos detalhes por Enter/Espaço, foco de 3 px e alvo de toque de pelo menos 44 px.
- Ausência de rolagem horizontal em larguras de 320 e 360 CSS px.
- Retorno 404 e falha real depois de retirar o WASM e sua versão Brotli da cópia de teste.
- Orientação visível após dez segundos, acesso ao recarregamento por teclado e recuperação depois de restaurar os arquivos.
- 80 navegações de desempenho, com 20 por combinação de perfil e condição de cache, além de duas navegações de preparação.

Os erros de console na falta intencional do WASM são esperados e estão identificados separadamente. Não foram aceitos erros na navegação normal ou depois da recuperação. Ver [resumo da CI](issue-001/ci.json), [resultados dos testes](issue-001/tests.log) e [amostras completas](issue-001/performance-samples.json).

## Versões efetivamente compiladas

| Componente | Versão |
|---|---|
| Rust / Cargo | 1.99.1 / 1.99.1 |
| Leptos | 0.8.23 |
| Leptos Axum / Router / Meta | 0.8.10 / 0.8.17 / 0.8.7 |
| Axum / Tokio | 0.8.9 / 1.54.1 |
| SQLx | 0.9.0 |
| wasm-bindgen e respectivo CLI | 0.2.130 |
| cargo-deny | 0.20.2 |
| Chrome for Testing e ChromeDriver | 156.0.8078.80 |

`Cargo.lock` fixa também as dependências transitivas. O aviso de manutenção `RUSTSEC-2024-0436`, referente a `paste 1.0.15`, tem exceção restrita e justificada na [decisão 0001](../decisoes/0001-fundacao.md). Não há exceção genérica para novos avisos de segurança.

## Tamanho inicial e compilação

| Recurso | Medida contabilizada | Limite | Resultado |
|---|---:|---:|---|
| WASM, Brotli | 155.731 bytes, 152,08 KiB | 1.048.576 bytes, 1 MiB | Aprovado |
| JS gerado, Brotli + inline bruto | 7.780 bytes, 7,60 KiB | 81.920 bytes, 80 KiB | Aprovado |
| CSS, Brotli | 1.543 bytes, 1,51 KiB | 61.440 bytes, 60 KiB | Aprovado |

O JS externo mede 7.111 bytes com Brotli. Outros 669 bytes de scripts gerados pelo Leptos entram pelo tamanho bruto, como margem conservadora, pois são transferidos dentro do HTML. Os três arquivos externos também foram medidos pelo navegador: seus `encodedBytes` coincidem com o orçamento. Não há fonte, imagem ou recurso externo adicional carregado nesta entrada.

O WASM sem compressão mede 1.568.930 bytes; o executável do servidor mede 10.539.016 bytes. O HTML da rota mede 4.682 bytes sem compressão. A referência Brotli de HTML presente no JSON é uma compressão local de comparação, não uma alegação de tamanho observado na rede. O serviço de HTML comprimido foi testado separadamente.

O build registrou **215,658 s para servidor/processamento**, **77,804 s para WASM** e **0,107 s para gerar interoperabilidade**, com **303,652 s no total** incluindo as demais etapas. Era a primeira compilação dos artefatos de distribuição nesse checkout, indicada por `incremental: false`; a CI já havia compilado os perfis de análise/teste e instalado ferramentas. O tempo total do job foi **15 min 08 s**. Ver [build.json](issue-001/build.json) e [budget.json](issue-001/budget.json).

## Desempenho medido nesta entrada

Ambiente observado: Linux x86_64, Ubuntu 24.04 no Actions, processador reportado AMD EPYC 9V74, 2 processadores disponíveis e `MemTotal: 8122984 kB`, aproximadamente 7,75 GiB. Chrome e driver nas versões acima. Computador em 1366 × 768 CSS px e celular em 360 × 800 CSS px; CPU reduzida em quatro vezes, rede de 4 Mbps de descida, 1 Mbps de subida e 150 ms de latência.

| Perfil | Condição | Amostras | LCP p75 | Hidratação p75 | Resposta local p75 | CLS p75 |
|---|---|---:|---:|---:|---:|---:|
| Computador | Cache desativado e limpo | 20 | 408 ms | 738,5 ms | 29,1 ms | 0 |
| Computador | Repetição com cache habilitado | 20 | 384 ms | 696,1 ms | 26,4 ms | 0 |
| Celular simulado | Cache desativado e limpo | 20 | 376 ms | 695,2 ms | 21,2 ms | 0 |
| Celular simulado | Repetição com cache habilitado | 20 | 376 ms | 691,8 ms | 29,7 ms | 0 |

Os quatro grupos passaram pelos limites aplicados à rota: LCP p75 até 2.500 ms, CLS p75 até 0,1 e resposta local controlada p75 até 100 ms. Os percentis foram recalculados na revisão independente a partir das 80 amostras, com o mesmo resultado.

O campo `warm` dos dados significa repetição após uma navegação de preparação com cache habilitado. A política atual `Cache-Control: no-store` mantém a transferência dos três arquivos em todas essas navegações. Portanto, esses números não comprovam acerto de cache.

A resposta local mede o clique até duas oportunidades de pintura e não é INP de visitantes reais. O tempo de hidratação é contado desde o início da navegação até o estado de interface pronta; inclui carregamento, inicialização e execução. O heap JavaScript observado variou entre 1.822.523 e 9.228.245 bytes nas 80 navegações. As amostras também incluem contadores CDP. Isso não mede RSS do navegador, memória total do WASM ou pico de memória da compilação.

Este executor compartilhado de 2 processadores não equivale ao cenário integrado de 4 vCPUs e 8 GiB, sem concorrência externa, especificado em `docs/experiencia.md`. As medidas são uma referência inicial reproduzível da página entregue. Não comprovam capacidade das futuras jornadas, telemetria real, dispositivo físico ou conformidade WCAG completa. Leitor de tela e zoom real de 400% não foram ensaiados nesta etapa; o reajuste a 320 CSS px foi. O aparelho Android de referência selecionado, sem alegação de compra ou teste físico, está na [decisão 0001](../decisoes/0001-fundacao.md).

## Custo inicial observado e estimado

Foi utilizado um job padrão Linux x64 de dois processadores. A [tabela oficial de preços do GitHub Actions](https://docs.github.com/en/billing/reference/actions-runner-pricing), consultada em 11/11/2026, informa US$ 0,006 por minuto para esse executor e arredondamento de cada job para o minuto inteiro seguinte.

Para a execução aprovada de 15 min 08 s, o **custo bruto estimado de computação é 16 × US$ 0,006 = US$ 0,096**. Esse valor não é uma cobrança confirmada: a franquia disponível e o plano da conta não foram consultados. Armazenamento de artefatos, outras execuções e futuras instâncias de aplicação, PostgreSQL e objetos não estão incluídos. Nenhuma infraestrutura de produção ou banco foi provisionada por esta issue; o custo mensal do cenário integrado será registrado quando ele existir.

## Revisão adversarial independente

Um revisor independente da implementação procurou formas de contornar os controles, revisou as correções e conferiu a execução real. Foram corrigidos três problemas antes do aceite:

| Achado | Correção | Reteste |
|---|---|---|
| Segredo em JSON compacto/aninhado ou TOML inline podia escapar de expressões por linha. | Análise estrutural recursiva de JSON/TOML, nomes sensíveis em diferentes formas e controles de ambiente/YAML. | Casos de regressão do `xtask` e inclusão temporária de JSON sintético recusada pelo comando real. |
| CSS ou recurso remoto poderia escapar da soma dos arquivos locais. | Recusa de recursos externos não inventariados, imports CSS externos e referências locais ausentes; soma de todos os arquivos servidos. | Casos negativos com recursos adicionais, atributos HTML e imports CSS. |
| O cálculo de inline poderia sugerir uma transferência comprimida isolada que não existia. | Cobrança conservadora do inline bruto, compressão real da resposta HTML e verificação dos bytes dos arquivos Brotli servidos. | Testes HTTP, orçamento e medidas de transferência do navegador coincidentes. |

O revisor também conferiu as seis capturas, as 80 amostras, os percentis, a falha esperada do WASM, o console limpo após recuperação e os limites das alegações. Conclusão: **a aprovação técnica dos critérios próprios da issue 1 está sustentada, sem bloqueador demonstrado remanescente**. A integração do PR depende da CI do último commit e fica registrada no próprio PR.

As três mutações exigidas pela issue foram executadas temporariamente no checkout e removidas: teste quebrado, segredo sintético e TypeScript próprio. A base restaurada voltou a passar. Não foram mantidos dados de cliente, credenciais ou arquivos dessas mutações no produto.

## Evidência visual conferida

| Situação | Captura |
|---|---|
| Conteúdo inicial sem scripts | [ssr-without-scripts.png](issue-001/ssr-without-scripts.png) |
| Detalhes abertos e foco por teclado | [desktop-expanded.png](issue-001/desktop-expanded.png) |
| Reajuste a 360 CSS px | [mobile-360-expanded.png](issue-001/mobile-360-expanded.png) |
| Reajuste a 320 CSS px | [mobile-320-expanded.png](issue-001/mobile-320-expanded.png) |
| WASM ausente, orientação e recarregamento | [wasm-unavailable.png](issue-001/wasm-unavailable.png) |
| Página recuperada pelo teclado | [keyboard-recovery.png](issue-001/keyboard-recovery.png) |

As imagens são capturas da janela nos pontos percorridos pelo teste, inclusive após rolagem. A verificação de abertura/fechamento, foco e recuperação está nas asserções e no JSON funcional, além das imagens.

## Limites da etapa e reprodução

Não há mutação financeira ou de estado persistente nesta fundação; concorrência de saldo, empresa errada, duplicidade de transação e versão antiga serão verificadas quando esses comportamentos existirem. Banco/migrações começam na issue 2. O plano aprovado e a ordem das demais issues foram preservados.

O procedimento completo, os comandos, as variáveis públicas e a configuração dos ensaios estão em [Desenvolvimento](../desenvolvimento.md). Tentativas locais de compilação de distribuição foram interrompidas pelo ambiente compartilhado antes da CI; não foram usadas como evidência de sucesso. A aprovação de compilação e navegador deste registro vem da execução limpa identificada acima.
