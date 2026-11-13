# Verificação da issue 7

## Objeto e situação

Implementação de [Organizações, convites e isolamento de dados](../issues/007.md), sobre a PR 78 da autenticação. O [guia de empresas](../organizacoes.md) descreve os percursos, as políticas e os limites da entrega.

| Referência | Identificação |
|---|---|
| Base remota | `feat/issue-6-autenticacao`, PR 78 |
| Commit da base | `5b711d9c11c3c4f99c53f5ef05fa12e9c4a52a05` |
| Árvore da base | `10782a0176e3cb643cfd4e652cbac633bbd638ab` |
| Branch da implementação | `feat/issue-7-organizacoes` |
| Migração | `0004_organizations_and_memberships.sql` |

Registro parcial durante a verificação da PR em rascunho. Os resultados abaixo são execuções locais já observadas; a compilação otimizada, a verificação visual no navegador e a CI ainda serão consolidadas com as respectivas revisões e evidências. Não representam aprovação desses passos pendentes.

## Entrega em revisão

A implementação inclui cadastro e seleção explícita de empresa, papéis de administrador e membro, convites vinculados ao e-mail verificado, pedidos de acesso com aprovação, validade e encerramento de vínculos, busca e exportação da equipe. Administração exige segunda etapa confirmada na sessão. Associação à empresa não dá acesso automático a obras.

Autorização e gravação compartilham uma transação. A persistência usa políticas obrigatórias de segurança por linha, versões e bloqueios ordenados; a proteção do último administrador também existe no banco. A seleção pertence à sessão e possui geração. Consultas e exportações recusam empresa ou contexto incompatível antes de entregar resultados.

A interface conserva campos não secretos apenas na memória da aba, com pessoa e empresa identificadas. Dados privados ficam ocultos durante a revalidação. Respostas atrasadas não repõem dados de outra empresa ou conta. Códigos transitórios não são guardados no armazenamento persistente do navegador.

Os contratos para objetos, cache, índices e filas definem a separação por empresa. Busca e exportação de membros já usam a fronteira autorizada. Provedores externos desses recursos continuam no escopo dos módulos consumidores.

## Resultados locais observados

| Verificação | Resultado |
|---|---|
| Testes nativos do workspace | 129 aprovados, zero reprovados; 59 cenários externos ignorados nesse comando |
| Persistência com PostgreSQL real | 23 aprovados, incluindo oito cenários novos |
| Aplicação com PostgreSQL real | 20 aprovados, incluindo oito cenários de organizações e 12 regressões da autenticação |
| Repetição dos oito cenários de organizações após os últimos ajustes do serviço | Oito aprovados, zero reprovados |
| HTTP com PostgreSQL real | Seis aprovados novamente na revisão final, incluindo três novos, em 100,87 s |
| Clippy nativo com avisos tratados como erro | Aprovado após correção do tipo de rejeição HTTP |
| Testes da interface após ajuste de revalidação | Dez aprovados, zero reprovados; Clippy WASM e da suíte de navegador aprovados |
| Auditoria de dependências | Avisos de segurança, origens, licenças e versões aprovados |
| Formatação e política | Aprovadas; plano com 72 issues e 192 regras preservado |

Resultados de repetições não são somados como testes novos. Os 59 cenários externos correspondem a 23 de persistência, 20 de aplicação, seis HTTP e dez de navegador. A execução de navegador ainda está pendente neste registro parcial.

## Revisão adversarial

A revisão verificou referências de empresa indevida, perda de vínculo, expiração, ausência de segunda etapa, versões antigas, uso repetido de convites, mudança de papel, seleção concorrente e preservação do último administrador. Foram corrigidos ciclos de bloqueio entre identidades e vínculos, a limpeza de seleção na auditoria apropriada e a retenção indevida de resultados de busca ao mudar a autorização. Regressões específicas acompanham as correções.

A revisão da interface também exigiu ocultar dados imediatamente ao perder acesso, preservar apenas rascunhos da mesma pessoa e empresa e aguardar a conclusão de uma mutação em voo antes da revalidação. Os resultados finais e as capturas desse percurso serão registrados após a execução real.

## Integração e limites

Esta branch parte da PR 78. As issues 3 a 6 dependem de aceite e integração na ordem prevista em `AGENTS.md`. O teste da issue 3 com pelo menos três profissionais reais permanece pendente e não é substituído por ensaios automatizados. Não há merge nem fechamento automático da issue 7 nesta entrega.

Convites são copiados e enviados manualmente pelo administrador; não há envio automático de e-mail nesta etapa. A camada geral de idempotência pertence à issue 9. Em uma criação de empresa cuja resposta se perdeu, a pessoa deve atualizar a lista antes de repetir. A entrega não representa implantação em produção nem aceite comercial da versão publicada.
