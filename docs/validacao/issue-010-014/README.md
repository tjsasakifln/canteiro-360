# Evidências das issues 10 a 14

## Versão final conferida

Código funcional: `7166902c789aac63092a470b13175eefbd18f067`.

Checkout da [CI 44663208420](https://github.com/tjsasakifln/canteiro-360/actions/runs/44663208420): `36edb2b1a98ebfac9385b73a180b49f71f0bb61a`, com a mesma árvore de arquivos do código funcional. A [PR 83](https://github.com/tjsasakifln/canteiro-360/pull/83) permanece em rascunho sobre a base da PR 82.

Foram aprovados 320 testes executados, compilação de produção, formatação, política, Clippy nativo e WASM e orçamento dos arquivos. O job falhou exclusivamente na auditoria de `chrono` e `chrono-tz` sem manutenção e no upload por cota do GitHub Actions. As capturas foram recuperadas do transporte delimitado dos logs e conferidas por origem, tamanho e SHA-256.

| Registro | Conteúdo |
|---|---|
| [ci-44663208420.json](ci-44663208420.json) | Resultados, etapas, fontes, medidas, bloqueios e limites da rodada final |
| [ci-44663208420-tests.log](ci-44663208420-tests.log) | Nomes e resultados dos testes extraídos do registro real, sem variáveis de ambiente ou transporte de imagens |
| [ci-44663208420-checks.log](ci-44663208420-checks.log) | Compilação, orçamento, avisos de auditoria e erro de cota |
| [source-manifest.json](source-manifest.json) | 81 arquivos de código e configuração alterados, conferidos contra os blobs remotos |
| [registry-interface.json](registry-interface.json) | Medidas projetadas das dez telas, ambiente e versão do navegador |
| [ci-44663208420-visual-manifest.json](ci-44663208420-visual-manifest.json) | SHA-256, tamanho e fonte de cada imagem e JSON recuperado |
| [ci-44663208420-visual-inventory.json](ci-44663208420-visual-inventory.json) | Inventário original do transporte; ausência esperada da imagem de falha |

## Capturas revisadas

Os arquivos são capturas completas de Chrome 156.0.8078.80, com dados fictícios. A largura do viewport é 360 ou 1280 pixels; no desktop a barra de rolagem resulta em PNG com 1265 pixels. As cinco telas foram abertas pelos revisores responsáveis e os pontos decisivos foram conferidos pelo supervisor. Os controles medidos têm pelo menos 44 pixels em cada dimensão, com rótulos associados para checkboxes e radios. Não houve transbordamento horizontal nos dez estados capturados.

| Tela e estado observado | Celular | Desktop |
|---|---|---|
| Cadastros: documento inválido e rascunho preservado | [360 px](registry-cadastros-360.png) | [1280 px](registry-cadastros-1280.png) |
| Carteira: obra revisada, versão 2, coordenadas e rateio de 100% | [360 px](registry-carteira-360.png) | [1280 px](registry-carteira-1280.png) |
| Projetos: projeto salvo em elaboração, sem obra ou contrato | [360 px](registry-projetos-360.png) | [1280 px](registry-projetos-1280.png) |
| Documentos: versão 1 ativa e confirmação de envio | [360 px](registry-documentos-360.png) | [1280 px](registry-documentos-1280.png) |
| Obrigações: ART pendente, erro de profissional ausente e campos mantidos | [360 px](registry-obrigacoes-360.png) | [1280 px](registry-obrigacoes-1280.png) |

A aprovação visual se limita a esses estados. Não comprova todos os estados de assinatura, quarentena, aprovação ou uma auditoria completa de acessibilidade. Datas e seletor de arquivo nativos refletem o idioma do Chrome; os resumos da aplicação são brasileiros. Os rótulos do mapa móvel são pequenos, com alternativas pelos campos de coordenadas e lista.

## Evidências históricas e tentativas sem aprovação

- `ci-44648996986.json` registra a primeira CI, com duas falhas de navegador posteriormente corrigidas.
- `ci-44657829805.json` e `ci-44657829805-tests.log` registram 320 testes aprovados antes das correções visuais finais. As imagens dessa rodada não são as imagens finais deste diretório.
- `registry-*.log`, `build.json`, `budget.json` e `build-artifacts.json` preservam verificações locais anteriores. `build-artifacts.json` identifica a fonte `f1c297d`; seus tamanhos não substituem os da CI final.
- `native-forms-*.log`, `evidence-*.log` e `visual-controls-*.log` preservam verificações focais das correções.
- `local-browser-unavailable.log` registra a recusa do executor ao iniciar o socket do Chrome, antes de abrir página. Não é aprovação de navegador.
- `local-build-7166902-unavailable.log` registra a tentativa local encerrada por sinal 9. Não é aprovação de compilação; a CI final compilou essa versão com sucesso.

O [relatório integrado](../ISSUES-010-014.md) explica os resultados, as correções e os limites de aceite. Nenhuma evidência declara integração em `main`, prontidão comercial, autenticação de documento de conselho, inscrição no CNO ou validação de uma base real de antivírus.
