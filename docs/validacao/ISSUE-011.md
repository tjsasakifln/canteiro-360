# Validação da issue 11

## Objeto

Cadastros de clientes, fornecedores e profissionais, implementados sobre a base da PR 82. Os arquivos de domínio, contratos, persistência, aplicação, HTTP e Leptos usam o módulo `parties`; a migração é `0010_parties.sql`. Guia: [Cadastros e carteira](../cadastros-carteira.md).

## Verificação executada

Em 20/11/2026, o grupo `registry_` da aplicação foi executado com PostgreSQL 17.11 real, papéis de migração e operação separados, RLS forçada e Rust 1.99.1. Os nove testes do grupo passaram, incluindo três casos específicos da carteira e dois cenários compartilhados de contexto e expiração. Os casos de cadastro cobriram:

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

Os três testes puros de `canteiro-domain parties::` passaram em 20/11/2026, incluindo vetores oficiais, máscaras inválidas e período profissional.

A interface compilou para renderização no servidor com `cargo check -p canteiro-web --features ssr --offline`. A revisão independente conferiu autorização temporal, identidade da repetição, descarte de respostas após sair da tela e correspondência entre o histórico e o cadastro selecionado. Os quatro achados foram corrigidos, revistos e os testes de aplicação afetados passaram. A inspeção visual e a execução em navegador pertencem à validação integrada do lote 10 a 14 e devem ser vinculadas à versão publicada antes do aceite técnico final.

## Limites preservados

Validar dígitos não confirma situação cadastral na Receita Federal. O arquivo de exportação é JSON textual; não se declara emissão fiscal. Os comprovantes financeiros que consumirão versões bancárias pertencem às etapas financeiras e à issue 70. A criação de um profissional não concede acesso ao sistema. Este registro não fecha a issue nem autoriza operação comercial.
