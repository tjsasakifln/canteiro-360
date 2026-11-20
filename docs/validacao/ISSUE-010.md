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

## Verificações executadas

Foram executados dois testes de domínio e doze testes da aplicação, incluindo quatro cenários com PostgreSQL 17 em banco isolado, usando o papel de execução restrito. A rodada centralizada final aprovou os testes puros na suíte geral de 208 testes e os quatro cenários de banco no conjunto de 47 testes de aplicação. Essa rodada já inclui a guarda de rodapé final do PDF e o décimo segundo teste. Os resultados e a evidência de navegador são separados no [relatório integrado](ISSUES-010-014.md); a execução visual ainda depende da rodada de CI registrada ali.

| Cenário | Resultado observado |
|---|---|
| Extensão falsa, PDF forjado, PDF com ação ativa, PNG incompleto, texto inválido, tamanho excessivo e fórmula CSV | Recusados. |
| Parser PDF com contêiner passivo de assinatura | Aceito sem regravar os bytes. |
| PDF com 10.380 níveis de aninhamento e PDF com mais de 200 mil tokens | Recusados antes da recursão do parser. |
| Conteúdo de fluxo com colchetes, tamanho incorreto e referência indireta de tamanho, inclusive nome escapado | Bytes do fluxo não consomem a profundidade estrutural; tamanho falso e indireto são recusados. |
| Objeto escondido dentro de fluxo, número incorreto na tabela e endereço anterior forjado | Recusados antes do parser; atualização incremental legítima aceita. |
| Rodapé `startxref` e `%%EOF` escondidos dentro de fluxo após o rodapé legítimo | Teste acrescentado; execução do estado final reservada à rodada centralizada. |
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

## Revisão

A revisão do supervisor identificou que a primeira inspeção por marcadores PDF era insuficiente e que bloquear qualquer AcroForm também impediria assinaturas passivas. A implementação passou a validar a estrutura com parser nativo e a aceitar o contêiner passivo sem transferir ou autenticar assinaturas.

A auditoria identificou [RUSTSEC-2026-0187](https://rustsec.org/advisories/RUSTSEC-2026-0187) na versão inicialmente escolhida do parser. A dependência foi atualizada para `lopdf 0.47.0`, sem funcionalidades de incorporação de fontes, removendo também `ttf-parser` da árvore. A auditoria final de avisos não encontrou aviso novo dessas dependências. Permanecem os avisos preexistentes da base sobre `chrono` e `chrono-tz`, sem exceção no gate; o registro consolidado da entrega trata esse bloqueio.

A revisão cruzada também corrigiu revalidação final no recibo de verificação concluída e precisão da data de assinatura. A expiração do link é conferida novamente depois da leitura do conteúdo. Os testes de corrida usam o relógio do PostgreSQL e observam o bloqueio real antes de aguardar o vencimento.

Uma segunda revisão cruzada identificou a possibilidade de a tabela de referências apontar para um objeto escondido dentro de fluxo. A conferência de limites passou a validar os endereços dos objetos e das tabelas antes de chamar o parser. O vetor de teste correspondente usa 33 níveis e confirma recusa sem executar recursão profunda.

A revisão de ciclo de vida da interface acrescentou uma guarda independente dos sinais reativos para descartar respostas após desmontar a página. Cada mudança de contexto invalida consultas anteriores; erros não apagam os campos do formulário. Datas de apresentação e de assinatura usam horário de Brasília.

Após sucesso ou troca explícita de vínculo, o seletor de arquivo também é limpo no navegador para permitir escolher o mesmo arquivo novamente. Em erro, a seleção permanece disponível para nova tentativa.

## Dependências e aceite

A implementação parte da revisão `17880bc98de8d1b8d20be3c5075aa53c3fbc6742`, da PR 82, que entrega a issue 9. Este registro não fecha a issue 10 antes da integração da base e da conclusão das verificações da versão entregue. O teste com profissionais da issue 3 e o aceite humano pendente da issue 6 permanecem registrados nas respectivas etapas.
