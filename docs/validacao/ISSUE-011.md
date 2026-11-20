# Validação da issue 11

## Objeto

Cadastros de clientes, fornecedores e profissionais, implementados sobre a base da PR 82. Os arquivos de domínio, contratos, persistência, aplicação, HTTP e Leptos usam o módulo `parties`; a migração é `0010_parties.sql`. Guia: [Cadastros e carteira](../cadastros-carteira.md).

## Verificação executada

Em 19/11/2026 (horário de Brasília), o grupo `registry_` da aplicação foi executado com PostgreSQL 17.11 real, papéis de migração e operação separados, RLS forçada e Rust 1.99.1. Os nove testes do grupo passaram, incluindo três casos específicos da carteira e dois cenários compartilhados de contexto e expiração. Os casos de cadastro cobriram:

| Critério | Evidência executada |
|---|---|
| CNPJ numérico e alfanumérico | Vetores oficiais `12345678000195` e `12ABC34501DE35` aceitos; erro no dígito rejeitado; exportação mantém texto. |
| Deduplicação sem revelar outra empresa | Mesmo identificador reutiliza o cadastro dentro da empresa; outra empresa mantém identidade própria e não consulta a primeira. |
| Conta bancária restrita e histórico | Membro comum não lê nem altera; a política SQL também oculta as contas. Duas alterações preservam ambas as versões. Exportação comum não inclui os dados. |
| Profissional sem login | Cadastro profissional não acrescenta usuário nem concessão. Importação com linha inválida não grava as linhas válidas anteriores. |
| Concorrência e repetição | Duas requisições simultâneas com a mesma chave retornam recibos idênticos. Outro conteúdo com essa chave falha. Versão antiga não sobrescreve revisão. |
| Contexto e expiração | Após renomear e selecionar novamente a empresa, a mesma intenção recupera o recibo, sem nova revisão. Consulta e histórico que aguardaram um bloqueio real da linha falham quando o vínculo vence durante a espera. |
| História protegida | Tentativas de remover histórico cadastral, bancário e recibos foram rejeitadas. Inativação preserva versões e impede nova conta bancária. |

Comando executado: `cargo test -p canteiro-application registry_ --offline -- --ignored --test-threads=1`, com `CANTEIRO_TEST_DATABASE_URL` apontando exclusivamente ao PostgreSQL local de teste. Resultado do grupo: 9 aprovados, 0 falhas, 18,60 s na execução após as correções da revisão independente.

A verificação central também aprovou 47 testes de aplicação com PostgreSQL, incluindo os nove casos `registry_`. O [relatório integrado das issues 10 a 14](ISSUES-010-014.md#resultado-final-conferido) acompanha a verificação da versão e as evidências.

Os três testes puros de `canteiro-domain parties::` passaram em 19/11/2026 (horário de Brasília), incluindo vetores oficiais, máscaras inválidas e período profissional.

A interface compilou para renderização no servidor com `cargo check -p canteiro-web --features ssr --offline`. A revisão independente conferiu autorização temporal, identidade da repetição, descarte de respostas após sair da tela e correspondência entre o histórico e o cadastro selecionado. Os quatro achados foram corrigidos, revistos e os testes de aplicação afetados passaram.

## Interface e evidências de navegador

### Execução anterior: CI 44648996986

Na [CI 44648996986](https://github.com/tjsasakifln/canteiro-360/actions/runs/44648996986), o percurso de cadastros executou gravação pelo teclado, contato, recusa de CNPJ inválido com preservação dos campos e verificações de largura antes de parar no formulário de projetos. A cota de artefatos impediu recuperar as capturas dessa execução. Esse resultado permanece histórico e não comprova a aparência da correção posterior.

### CI 44657829805: percurso concluído e falha visual identificada

A [CI 44657829805](https://github.com/tjsasakifln/canteiro-360/actions/runs/44657829805) aprovou os 13 testes de navegador, incluindo o percurso completo das cinco telas. A head funcional era `f1c297d029fdc31742bbc0d933c5e8ed7acb86b5`; o checkout efetivamente testado e registrado nas evidências foi `2de5b86fa1d6ebf3ccad1f064aac59dbaf1b0464`.

As capturas `registry-cadastros-360.png` e `registry-cadastros-1280.png` foram recuperadas pelos registros da CI, com origem, tamanho e SHA-256 conferidos. A inspeção confirmou a mensagem de CPF/CNPJ inválido, a preservação do nome de rascunho, do identificador recusado e do contato, além da permanência do cadastro salvo na lista. Não foi observado transbordamento horizontal nas duas larguras.

A mesma revisão encontrou uma falha visual: os campos de contato e o seletor direto de tipo usavam apresentação nativa pequena; rótulos e metadados da lista apareciam colados. A correção em `style/registry.css` amplia a fonte e os campos, define altura mínima de 44 pixels, separa rótulos e metadados e preserva checkboxes compactos com rótulos clicáveis. Dois agentes revisaram o CSS sem encontrar falha material. A aprovação funcional dessa CI não constitui aprovação visual da versão corrigida. O [relatório integrado](ISSUES-010-014.md#ci-44657829805-e-revisão-das-capturas) registra os resultados completos e as limitações de auditoria e disponibilização dos artefatos.

### CI 44663208420: correção confirmada no estado ensaiado

A [CI 44663208420](https://github.com/tjsasakifln/canteiro-360/actions/runs/44663208420) testou a head `7166902c789aac63092a470b13175eefbd18f067`, pelo checkout `36edb2b1a98ebfac9385b73a180b49f71f0bb61a`. A compilação de produção, o orçamento de arquivos e os 13 testes de navegador passaram; o percurso integrado terminou em 30,49 segundos. A mesma CI aprovou os 47 testes de aplicação com PostgreSQL, incluindo os nove `registry_`. A auditoria continuou bloqueada pelos avisos de manutenção `RUSTSEC-2026-0543` e `RUSTSEC-2026-0555`, e o upload convencional falhou por cota de armazenamento. A aprovação funcional e visual aqui registrada não altera o resultado global da CI.

As novas capturas foram recuperadas com conferência de origem, tamanho e SHA-256 e abertas para revisão: [cadastros em 360 px](issue-010-014/registry-cadastros-360.png) e [cadastros em 1280 px](issue-010-014/registry-cadastros-1280.png). O [registro de medidas](issue-010-014/registry-interface.json) informa:

| Largura da janela | Controles visíveis medidos | Menor altura | Menor largura | Largura do conteúdo |
|---|---:|---:|---:|---:|
| 360 px | 37 | 44 px | 83 px | 360 px |
| 1280 px | 37 | 44 px | 83 px | 1265 px |

Para checkboxes e radios, a medição usa o rótulo associado, mantendo o controle compacto. A imagem de desktop tem 1265 pixels de largura porque a barra de rolagem ocupa 15 pixels da janela de 1280.

A revisão confirmou os campos de contato e o seletor de tipo com fonte legível, largura adequada e rótulos separados. Os botões mantêm o texto legível, e o título do cadastro aparece separado dos metadados da lista. A mensagem de CPF/CNPJ inválido continua visível, com nome de rascunho, identificador recusado e contato preservados. Não foram observados cortes de controles, sobreposições ou transbordamento horizontal. A correção visual está aprovada para esse estado nas duas larguras.

Essa inspeção não abrange todos os estados de cadastro profissional, importação, dados bancários, permissão ou histórico, nem equivale a uma auditoria completa de acessibilidade. Os cenários específicos conservam as evidências de teste descritas acima. Os resultados locais anteriores e as capturas da CI 44657829805 permanecem históricos; as imagens vinculadas nesta seção pertencem à CI 44663208420.

## Limites preservados

Validar dígitos não confirma situação cadastral na Receita Federal. O arquivo de exportação é JSON textual; não se declara emissão fiscal. Os comprovantes financeiros que consumirão versões bancárias pertencem às etapas financeiras e à issue 70. A criação de um profissional não concede acesso ao sistema. Este registro não fecha a issue nem autoriza operação comercial.
