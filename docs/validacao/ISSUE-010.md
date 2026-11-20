# Issue 10: documentos, anexos e versões imutáveis

## Implementação

A página `/documentos` usa a empresa selecionada e mostra somente obras, contratos, orçamentos e projetos com permissão explícita de consulta. Anexar exige também o ato correspondente ao vínculo. Uma empresa administrada pelo usuário não concede leitura automática dos documentos das obras.

O envio recebe o conteúdo completo antes de gravar qualquer anexo. Arquivos válidos na inspeção inicial entram em quarentena privada, identificados pelo autor, vínculo, categoria, descrição, tamanho, nome, SHA-256 e chave de tentativa. A interrupção antes do recebimento completo não cria versão documental. Uma tentativa repetida com o mesmo conteúdo recupera o mesmo envio; outra entrada com a mesma chave é recusada.

O serviço de verificação usa o protocolo ClamAV INSTREAM. Somente a resposta exata `stream: OK` permite promover o conteúdo a versão. Indisponibilidade, resposta desconhecida, erro ou término de prazo mantêm a quarentena. Uma resposta de detecção recusa o arquivo. A interface permite repetir a verificação sem duplicar o documento e informa que um arquivo em quarentena ainda não é um anexo válido.

A verificação externa acontece sem segurar o bloqueio da organização. Ao finalizar, o serviço abre nova transação, confere novamente sessão, empresa, vínculo, ato e versão, compara os bytes ao resumo recebido e só então grava a versão. Duas substituições da mesma versão não sobrescrevem uma à outra.

## Arquivos aceitos e limites

- Até 5 MiB por arquivo e 100 MiB de conteúdo em quarentena por organização.
- PDF com tabela de referências clássica, sem criptografia, objetos comprimidos, referências em fluxo, tamanho de fluxo indireto ou ações ativas. O parser `lopdf 0.47.0`, em modo estrito, confere catálogo, árvore de páginas e referências. Antes dele, uma inspeção iterativa limita profundidade a 32 e quantidade de tokens a 200 mil, sem contar o conteúdo binário dos fluxos como estrutura. Depois dele, os limites são 100 mil objetos, 10 mil páginas, profundidade de árvore de 32 e 200 mil nós inspecionados. A descompressão também fica limitada a 10 MiB por fluxo. Recursos ainda não aceitos são informados sem recomendar a alteração de um original assinado.
- Contêiner passivo de assinatura PDF é permitido. O arquivo não é regravado pelo parser e seu resumo permanece sobre os bytes originais.
- PNG com assinatura, cabeçalho, tamanhos, CRC de blocos e término conferidos, até 64 milhões de pixels. Blocos fora da lista permitida são recusados.
- TXT e CSV em UTF-8, sem caracteres de controle inadequados. CSV com prefixos de fórmula é recusado, inclusive em células entre aspas. Isso é uma política conservadora de aceitação; o conteúdo original não é alterado.
- Extensão e conteúdo precisam corresponder. O tipo declarado pelo navegador não decide o formato.

`CANTEIRO_DOCUMENT_SCANNER` deve apontar a um endereço IP e porta do verificador, por exemplo `127.0.0.1:3310`. O serviço tem prazo máximo de dez segundos por verificação. Sem configuração, os arquivos permanecem em quarentena. O operador precisa manter o mecanismo e as definições do antivírus atualizados. Os testes deste módulo não afirmam validação de um mecanismo antivírus real.

A sessão e o vínculo são autorizados antes de decodificar ou analisar o arquivo e novamente antes de gravar. A análise de PDF ocorre fora da thread de atendimento e do bloqueio da organização, com até duas análises simultâneas. A requisição espera no máximo três segundos. Esse prazo não cancela uma thread bloqueante: a vaga de análise continua ocupada até ela terminar. Saturação ou prazo excedido não criam upload nem versão e orientam nova tentativa.

Cada entrada da tabela de referências precisa apontar para o início real de um objeto, com número e geração correspondentes, encontrado fora de strings, comentários e fluxos. Os endereços `startxref` e `Prev` precisam apontar para seções reais dessa tabela. Isso impede que o parser carregue um objeto escondido em bytes que a inspeção havia tratado como conteúdo binário. Atualizações incrementais legítimas continuam aceitas.

## Versões, acesso e assinatura

O conteúdo e os metadados de cada versão são imutáveis. A versão ativa é uma referência a uma versão existente. Histórico e assinaturas não admitem UPDATE, DELETE ou TRUNCATE pelo papel operacional. O registro de assinatura referencia documento, revisão e SHA-256, com signatário, data e evidência. Ele não concede aprovação interna nem atesta autenticidade da assinatura.

A data da assinatura usa `UtcInstant`, com até seis casas decimais. Precisão maior é recusada antes de persistir, evitando truncagem silenciosa no PostgreSQL e conflito falso na repetição da mesma evidência.

Downloads usam links com duração de dois minutos, associados à sessão, ao autor e à versão. Conhecer o identificador ou copiar a URL para outra sessão não basta. A abertura revalida seleção de empresa, vínculo, escopo e validade, inclusive após revogação. Respostas são privadas, não armazenáveis em cache, com `nosniff`, política de referência restrita e sandbox. A prévia fica restrita a PNG e texto; PDF é disponibilizado para download.

A migração `0009_documents.sql` mantém o conteúdo fora dos recursos públicos. `document_versions` recebe somente documentos promovidos após a verificação; `document_uploads` não é uma fonte válida para obrigações técnicas. A ligação externa usa a chave composta de empresa, documento, versão e SHA-256.

## Verificações de domínio e aplicação

Foram executados dois testes de domínio e doze testes da aplicação, incluindo quatro cenários com PostgreSQL 17 em banco isolado, usando o papel de execução restrito. A rodada centralizada inicial aprovou os testes puros na suíte geral de 208 testes e os quatro cenários de banco no conjunto de 47 testes de aplicação. Essa rodada já incluiu a guarda de rodapé final do PDF e o décimo segundo teste. Os resultados das CI2 e CI3 e a revisão visual posterior da correção de CSS estão separados por versão no histórico abaixo.

Os cenários reproduzíveis estão nos [testes de domínio](../../crates/domain/src/documents.rs), nos [testes do parser PDF](../../crates/application/src/auth/organizations/documents/pdf.rs), nos [testes do protocolo de verificação](../../crates/application/src/auth/organizations/documents/scanner.rs) e nos [cenários com PostgreSQL](../../crates/application/src/auth/organizations/documents_tests.rs). O [relatório integrado](ISSUES-010-014.md) discrimina os resultados por revisão.

| Cenário | Resultado observado |
|---|---|
| Extensão falsa, PDF forjado, PDF com ação ativa, PNG incompleto, texto inválido, tamanho excessivo e fórmula CSV | Recusados. |
| Parser PDF com contêiner passivo de assinatura | Aceito sem regravar os bytes. |
| PDF com 10.380 níveis de aninhamento e PDF com mais de 200 mil tokens | Recusados antes da recursão do parser. |
| Conteúdo de fluxo com colchetes, tamanho incorreto e referência indireta de tamanho, inclusive nome escapado | Bytes do fluxo não consomem a profundidade estrutural; tamanho falso e indireto são recusados. |
| Objeto escondido dentro de fluxo, número incorreto na tabela e endereço anterior forjado | Recusados antes do parser; atualização incremental legítima aceita. |
| Rodapé `startxref` e `%%EOF` escondidos dentro de fluxo após o rodapé legítimo | Recusados antes do parser; vetor aprovado na rodada centralizada e incluído na CI2. |
| Protocolo do verificador | Bytes originais transmitidos; resposta exata aceita, detecção recusa, resposta ambígua não libera. |
| Dois envios simultâneos com a mesma chave | Um envio em quarentena, nenhuma versão documental antes da liberação. |
| Mesma chave com conteúdo diferente | Conflito, sem reaproveitar sucesso anterior. |
| Repetição após revogação de vínculo | Recusada. |
| Repetição de envio já concluído aguardando um bloqueio real do upload até a permissão expirar | Recusada depois de adquirir o bloqueio, sem devolver o recibo antigo. |
| Nova versão após assinatura da primeira | Bytes e assinatura da primeira preservados; segunda sem assinatura herdada. |
| Assinatura com sete casas decimais e assinatura válida com seis | A primeira é recusada; a segunda preserva a precisão e admite repetição sem duplicar. |
| Consulta e download de revisão anterior | Mesmo conteúdo e SHA-256 originais. |
| Alteração e remoção pelo papel operacional | Conteúdo, assinatura e histórico protegidos. |
| Identificador de upload apresentado como anexo | Recusado. |
| Link compartilhado com outro usuário, inclusive leitor autorizado do mesmo vínculo | Recusado por vínculo com a sessão original. |
| Outra organização, prazo do link vencido e vínculo de membro expirado | Consulta ou download recusados. |

Os cenários de versão e acesso semeiam explicitamente conteúdo previamente verificado para testar essas fronteiras. O teste do protocolo usa um servidor de ensaio que devolve respostas controladas. O ensaio completo de navegador e a revisão visual da versão integrada pertencem ao registro consolidado desta entrega, não são substituídos por esses testes.

Comandos utilizados:

```sh
cargo test -p canteiro-domain documents --lib
cargo test -p canteiro-application documents --lib -- --include-ignored --test-threads=1
cargo deny check advisories
```

## Navegador e revisão visual por versão

### CI2: execução 44657829805, anterior à correção visual

A [CI2, execução 44657829805](https://github.com/tjsasakifln/canteiro-360/actions/runs/44657829805) testou a head `f1c297d029fdc31742bbc0d933c5e8ed7acb86b5` pelo checkout de integração `2de5b86fa1d6ebf3ccad1f064aac59dbaf1b0464`, com a mesma árvore de arquivos. O conjunto executou 320 testes aprovados: 213 gerais, 37 de persistência, 47 de aplicação, dez de HTTP e 13 de navegador. O percurso integrado das cinco telas terminou em 17,18 segundos. O job não ficou aprovado: a auditoria permaneceu bloqueada pelos dois avisos de manutenção de `chrono` e `chrono-tz`, e o upload convencional de artefatos permaneceu recusado pela cota do GitHub Actions.

O [roteiro integrado da CI2](https://github.com/tjsasakifln/canteiro-360/blob/f1c297d029fdc31742bbc0d933c5e8ed7acb86b5/tools/e2e/tests/registry.rs) selecionou e enviou um TXT fictício pela interface de documentos. A fixture do protocolo INSTREAM respondeu positivamente e a aplicação publicou a versão ativa. O teste também conferiu, por HTTP, os bytes originais no download, a recusa de outra sessão e a repetição de um upload recusado sem criar outra versão ou outro recibo. O download por HTTP não equivale a ter acionado visualmente o botão de preparar link. A fixture controlada não valida um mecanismo antivírus real nem suas definições.

As duas capturas de documentos foram recuperadas dos registros da CI, junto com as demais telas, mediante conferência do SHA do checkout, dos tamanhos e dos hashes. Estes arquivos pertencem exclusivamente à CI2:

| Captura | Dimensões do PNG | SHA-256 |
|---|---|---|
| `registry-documentos-360.png` | 360 × 2553 px | `093dabcf60c88f8b0e17147fc3bbe392a79f725c4d1ce8a59235a37792b0c9f5` |
| `registry-documentos-1280.png` | 1265 × 1804 px | `fddd86d0cbcb721953814a9f4e108ac305c80674ef0ea241f55a0307722f7bdb` |

A largura de 1265 px corresponde ao viewport de 1280 px descontada a barra de rolagem. A revisão abriu ambas as imagens e confirmou o estado de sucesso, a versão 1 ativa, a descrição preservada, o seletor de arquivo limpo após o envio e o conteúdo completo até o rodapé. O autor extenso quebra em linhas no celular. As ações de preparar download, prévia e registrar assinatura estão visíveis. Essas capturas não mostram erro, quarentena, o conteúdo do resumo de assinaturas ou o link já preparado, portanto não aprovam visualmente esses estados.

A revisão também identificou um problema material: os botões secundários conservavam apresentação nativa pequena, e os cartões não tinham espaçamento interno porque dependiam de uma variável definida somente no protótipo. A aprovação funcional da CI2 não encerrou esse achado de interface.

### Versão atual: CI3 44663208420 e revisão visual concluídas

No módulo de documentos, a correção enviada na head `7166902c789aac63092a470b13175eefbd18f067` está restrita à apresentação em [documents.css](../../style/documents.css): cartões com espaçamento de 24 px, reduzido a 16 px no celular; fonte herdada e tamanho mínimo de 44 px nos botões secundários; campos e resumo expansível com mínimo de 44 px; ações empilhadas e quebra de textos longos. As regras de foco, feedback e estado desabilitado foram preservadas. O agente de cadastros realizou a revisão independente do CSS, sem identificar falha material por leitura do código.

O [teste integrado](../../tools/e2e/tests/registry.rs) foi ampliado para medir botões, campos, resumos expansíveis e links de navegação em 360 e 1280 px. Ele recusa dimensões inferiores a 44 px e continua verificando transbordamento horizontal e título único. O percurso mantém os comandos reais de teclado. A remoção do foco transitório e a rolagem ao topo acontecem somente antes da captura neutra, depois das operações e das medições.

A [CI3, execução 44663208420](https://github.com/tjsasakifln/canteiro-360/actions/runs/44663208420) usou o checkout `36edb2b1a98ebfac9385b73a180b49f71f0bb61a` e terminou em 20/11/2026 às 03:39:48 UTC. Foram aprovados 320 testes executados: 213 gerais, 37 de persistência, 47 de aplicação, dez de HTTP e 13 de navegador. O percurso integrado terminou em 30,49 segundos. As etapas de compilação de produção, orçamento e transporte das capturas passaram. Somente auditoria de dependências e upload convencional de artefatos falharam, pelos dois avisos de manutenção e pela cota de armazenamento, respectivamente. Esses bloqueios permanecem obrigatórios e impedem declarar o conjunto da CI aprovado.

As [medidas recuperadas do navegador](issue-010-014/registry-interface.json) registram o estado de documentos abaixo. A contagem inclui a navegação compartilhada. Os mínimos são apresentados como inteiros no relatório; a recusa do teste compara as dimensões sem arredondamento.

| Captura final | Dimensões do PNG | Controles medidos | Menor largura × altura | Largura do documento |
|---|---|---|---|---|
| [Documentos em 360 px](issue-010-014/registry-documentos-360.png) | 360 × 2974 px | 17 | 141 × 44 px | 360 px |
| [Documentos em 1280 px](issue-010-014/registry-documentos-1280.png) | 1265 × 2111 px | 17 | 141 × 44 px | 1265 px |

As duas larguras apresentam um único `h1`. O SHA-256 da captura móvel é `cf0a04ab9b41dc14e8d3acf889aed1e99c35fb38b8078c2264305bfeca5759db`; o da captura de desktop é `84f790a72e4f6765a658815a77b993bf340bb904cd537479dc5c5023de94b6d0`. Esses hashes identificam os arquivos da CI3, distintos das imagens históricas da CI2.

A revisão abriu ambas as novas imagens e aprovou a correção no estado exibido: cartões com espaçamento interno, botões com fonte legível e separados entre si, ações empilhadas no celular, versão 1 ativa, descrição da versão e autor legíveis, mensagem de sucesso e seletor de arquivo limpo. Não foram encontrados cortes de cartões, botões ou rodapé, nem um novo problema material. A medição confirmou o mínimo de 44 px e a ausência de transbordamento horizontal nas duas larguras.

Essa aprovação visual é limitada ao estado de sucesso capturado. As imagens não exibem erro, quarentena, resumo de assinaturas expandido ou link já preparado. O protocolo de antivírus continua representado por uma fixture controlada, e o download é conferido por HTTP no teste. A revisão não afirma autenticação de assinatura, validação de um antivírus real ou inspeção visual de estados não mostrados.

## Revisão

A revisão do supervisor identificou que a primeira inspeção por marcadores PDF era insuficiente e que bloquear qualquer AcroForm também impediria assinaturas passivas. A implementação passou a validar a estrutura com parser nativo e a aceitar o contêiner passivo sem transferir ou autenticar assinaturas.

A auditoria identificou [RUSTSEC-2026-0187](https://rustsec.org/advisories/RUSTSEC-2026-0187) na versão inicialmente escolhida do parser. A dependência foi atualizada para `lopdf 0.47.0`, sem funcionalidades de incorporação de fontes, removendo também `ttf-parser` da árvore. A auditoria final de avisos não encontrou aviso novo dessas dependências. Permanecem os avisos preexistentes da base sobre `chrono` e `chrono-tz`, sem exceção no critério obrigatório; o registro consolidado da entrega trata esse bloqueio.

A revisão cruzada também corrigiu revalidação final no recibo de verificação concluída e precisão da data de assinatura. A expiração do link é conferida novamente depois da leitura do conteúdo. Os testes de corrida usam o relógio do PostgreSQL e observam o bloqueio real antes de aguardar o vencimento.

Uma segunda revisão cruzada identificou a possibilidade de a tabela de referências apontar para um objeto escondido dentro de fluxo. A conferência de limites passou a validar os endereços dos objetos e das tabelas antes de chamar o parser. O vetor de teste correspondente usa 33 níveis e confirma recusa sem executar recursão profunda.

A revisão de ciclo de vida da interface acrescentou uma guarda independente dos sinais reativos para descartar respostas após desmontar a página. Cada mudança de contexto invalida consultas anteriores; erros não apagam os campos do formulário. Datas de apresentação e de assinatura usam horário de Brasília.

Após sucesso ou troca explícita de vínculo, o seletor de arquivo também é limpo no navegador para permitir escolher o mesmo arquivo novamente. Em erro, a seleção permanece disponível para nova tentativa.

## Dependências e aceite

A implementação parte da revisão `17880bc98de8d1b8d20be3c5075aa53c3fbc6742`, da PR 82, que entrega a issue 9. Este registro não fecha a issue 10 antes da integração da base e da conclusão das verificações da versão entregue. O teste com profissionais da issue 3 e o aceite humano pendente da issue 6 permanecem registrados nas respectivas etapas.
