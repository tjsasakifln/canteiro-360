# Canteiro 360

Gestão de obras e projetos privados no Brasil, com a disciplina operacional documentada nos manuais públicos do SICOP/SC e uma experiência de uso própria, rápida e intuitiva.

**Situação atual:** fundação Rust e persistência PostgreSQL das issues 1 e 2 integradas. A implementação da issue 3 acrescenta sistema visual e percursos demonstrativos de painel, orçamento, medição, aprovação e documentos, com dados fictícios. O aceite integral da issue 3 permanece condicionado ao teste com três profissionais exigido no escopo. O produto permanece em desenvolvimento, sem liberação para clientes. A entrega comercial da versão 1.0 depende da execução e aprovação das **72 issues em ordem numérica**, incluindo os testes e pilotos exigidos na última etapa.

Esta sequência de implementação inclui formulários e tabelas da issue 4, precisão e calendários da issue 5, autenticação da issue 6 e [empresas, convites e isolamento](docs/organizacoes.md) da issue 7. O [guia de acesso](docs/seguranca-acesso.md) descreve cadastro verificado, sessões, recuperação e segundo fator. As etapas 3 a 7 aguardam os aceites anteriores para integração em `main`.

## Compilar e executar

Com Rustup e as ferramentas de compilação disponíveis:

```sh
rustup show
cargo xtask setup
cargo xtask build
```

Para executar o servidor, provisione PostgreSQL e as duas identidades descritas no [guia de persistência](docs/persistencia.md). Injete a configuração de desenvolvimento, a chave externa `CANTEIRO_AUTH_KEY` e a origem pública conforme o [guia de acesso](docs/seguranca-acesso.md). O processo de migração recebe as duas conexões externas; a API recebe apenas a conexão de execução:

```sh
target/release/canteiro-db migrate
env -u CANTEIRO_MIGRATION_DATABASE_URL target/release/canteiro-api
```

A API recusa inicialização sem banco migrado, com ambiente divergente, chave de acesso ausente ou identidade de banco privilegiada. Abrir `http://127.0.0.1:3000/acesso` para cadastro, login, recuperação e gestão de sessões. O envio de e-mail usa a Resend com remetente e credencial externos; homologação exige essa configuração e HTTPS. O executável geral de processamento oferece apenas `--check`; a fila de mensagens de acesso desta etapa é consumida pela API.

Para experimentar os percursos da issue 3, abra `http://127.0.0.1:3000/prototipo/aurora/painel`. O seletor permite alternar entre duas empresas fictícias. As opções da demonstração exibem estados vazio, carregando, erro e sem permissão, além da densidade. Consultas e links são renderizados no servidor; edição, confirmação e troca de contexto dependem da hidratação. Os rascunhos e as aprovações simuladas existem somente na aba atual e não produzem gravações comerciais.

O [guia de desenvolvimento](docs/desenvolvimento.md) detalha compilação, verificações, ensaios do navegador e testes explícitos com banco real. A integração contínua executa esses cenários em PostgreSQL `17.7`, com identidades e dados fictícios descartáveis.

## Comece aqui

| Documento | Para que serve |
|---|---|
| [LEIAME](LEIAME.md) | Objetivo, escopo, decisões e orientação de execução |
| [Desenvolvimento](docs/desenvolvimento.md) | Compilação reproduzível, execução e validação |
| [Persistência](docs/persistencia.md) | PostgreSQL, migrações, identidades, limites e recuperação |
| [Acesso e recuperação](docs/seguranca-acesso.md) | Cadastro verificado, sessões, segundo fator, e-mail e configuração |
| [Empresas, equipe e convites](docs/organizacoes.md) | Seleção de empresa, vínculos, pedidos de acesso e isolamento |
| [Sistema visual](docs/sistema-visual.md) | Componentes, contexto, teclado, estados e apresentação móvel |
| [Verificação da issue 3](docs/validacao/ISSUE-003.md) | Evidência técnica, revisão e aceite humano ainda necessário |
| [Aceite da issue 1](docs/validacao/ISSUE-001.md) | Versões compiladas, medidas, revisão e evidências |
| [PRD](PRD.md) | Requisitos do produto e critérios de sucesso |
| [Plano de implementação](docs/BACKLOG.md) | Issues 01 a 72, dependências e fases |
| [Issues no GitHub](https://github.com/tjsasakifln/canteiro-360/issues?q=is%3Aissue%20sort%3Acreated-asc) | Fila de execução e evidências de aceite |
| [Regras e rastreabilidade](docs/MATRIZ-RASTREABILIDADE.md) | Regra, fonte, adaptação, issue e teste |
| [Arquitetura em Rust](docs/arquitetura-rust.md) | Front-end, servidor, dados, segurança e decisões técnicas |
| [Experiência do usuário](docs/experiencia.md) | Jornadas, componentes, acessibilidade e desempenho |
| [Prontidão comercial](docs/prontidao-comercial.md) | Condições para vender e operar o serviço |
| [Fontes](docs/fontes/README.md) | Manuais consultados, versões, hashes e limitações |
| [Revisão adversarial](docs/validacao/REVISAO-ADVERSARIAL.md) | Falhas encontradas, correções e verificações desta especificação |

## Diretriz técnica

Lógica de aplicação em **Rust**, inclusive a interface: **Leptos com renderização no servidor e hidratação em WebAssembly**, **Axum/Tokio**, **SQLx/PostgreSQL** e armazenamento privado de documentos. HTML, CSS, SQL e configuração continuam sendo formatos declarativos necessários. O código de interoperabilidade JavaScript gerado pelo compilador é permitido; uma segunda aplicação de negócio em JavaScript ou TypeScript não faz parte da arquitetura.

## Compromisso funcional

Preservar as regras operacionais demonstradas pelas fontes e registrar toda adaptação privada. Contrato, execução, medição, aprovação, faturamento, pagamento, recebimento e aceite técnico têm registros próprios. Cálculos são decimais, alterações preservam o histórico e aprovações são verificadas no servidor.

O Canteiro 360 é um produto independente. Os manuais servem de referência funcional; não há vínculo institucional, reutilização do código proprietário, identidade visual ou credenciais do SICOP.
