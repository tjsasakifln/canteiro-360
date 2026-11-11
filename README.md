# Canteiro 360

Gestão de obras e projetos privados no Brasil, com a disciplina operacional documentada nos manuais públicos do SICOP/SC e uma experiência de uso própria, rápida e intuitiva.

**Situação atual:** especificação e plano de implementação. O aplicativo ainda não foi implementado nem liberado para clientes. A entrega comercial da versão 1.0 depende da execução e aprovação das **72 issues em ordem numérica**, incluindo os testes e pilotos exigidos na última etapa.

## Comece aqui

| Documento | Para que serve |
|---|---|
| [LEIAME](LEIAME.md) | Objetivo, escopo, decisões e orientação de execução |
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
