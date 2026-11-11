# LEIAME

## 1. O produto

O Canteiro 360 permite a uma construtora, gerenciadora, incorporadora, escritório de engenharia ou proprietário profissional acompanhar obras e projetos privados desde o orçamento até o encerramento técnico e financeiro. O mesmo empreendimento pode reunir vários contratos, fornecedores, projetos e ordens de manutenção, com responsabilidades e saldos separados.

A proposta é dar ao responsável pela obra respostas verificáveis: o que foi contratado, o que mudou, o que foi executado, o que foi medido e aprovado, quanto foi faturado, quanto entrou ou saiu do caixa e qual decisão ainda depende dele. A interface deve facilitar essas respostas sem exigir que o usuário conheça a estrutura interna do sistema.

O ponto de partida funcional é o **SICOP de Santa Catarina**, identificado pelo [portal oficial](https://www.sicop.sc.gov.br/sicop/) e pelo [catálogo de manuais](https://www.sicop.sc.gov.br/download/manuaisSicop.jsp). Não se trata de outro sistema que utiliza a mesma sigla.

## 2. O que existe neste repositório

Este repositório contém o PRD, regras rastreáveis, decisões de cálculo, arquitetura, modelo de dados, especificação de experiência e **72 issues de implementação**. Os arquivos `docs/issues/001.md` a `072.md` preservam o conteúdo planejado; as issues nativas são a fila operacional. O [manifesto](docs/backlog.json) liga os números, os arquivos e os endereços publicados.

A fundação da issue 1 acrescenta o workspace Rust, servidor, página inicial renderizada no servidor e hidratada em WebAssembly. A issue 2 acrescenta PostgreSQL, migrações, tipos validados, transações com contexto de empresa e erros públicos seguros. O [guia de desenvolvimento](docs/desenvolvimento.md) e o [guia de persistência](docs/persistencia.md) explicam a operação. Os [registros de validação](docs/validacao) distinguem cada entrega. Ainda não há ambiente produtivo, autenticação funcional ou aplicação pronta para vender. Essas capacidades pertencem às próximas issues. A revisão original da especificação permanece preservada, separada da validação do código.

## 3. Como a fidelidade ao SICOP será tratada

A [matriz](docs/MATRIZ-RASTREABILIDADE.md) distingue:

| Classificação | Significado |
|---|---|
| Documentada | Comportamento localizado em fonte pública, com página ou seção |
| Adaptada | Função de controle preservada, com alteração explícita para contratos e organizações privadas |
| Decisão | Comportamento definido pelo produto onde a fonte não especifica o algoritmo ou onde é necessária capacidade comercial adicional |

Pacotes que modificam expressamente uma regra prevalecem sobre a redação antiga daquela regra. O ano no nome de um PDF ou a data de geração do arquivo não provam que todo o seu conteúdo esteja atualizado. As divergências conhecidas estão em [decisões e lacunas](docs/decisoes-e-lacunas.md).

O objetivo verificável é implementar integralmente a matriz aprovada. Os manuais não revelam todos os detalhes internos do sistema em produção. Por isso, não se promete igualdade de código, de telas ou de resultados não documentados. Essa distinção é especialmente necessária para truncagem monetária, distribuição de preço global e calendários.

As regras de medição acumulada, separação entre pré-aditivo e aditivo, aprovação de supressão medida, preservação de versões e liberação da medição final após recebimento provisório são centrais. As adaptações de autoridades, limites públicos e integrações estatais constam do PRD. Não há aplicação automática de percentuais históricos de licitação a contratos privados.

## 4. Toda a aplicação em Rust

A decisão do produto é usar Rust na interface, no servidor, nas regras de negócio, nos processamentos e nas ferramentas de aplicação. O front-end usará Leptos com HTML inicial renderizado no servidor e interatividade em WebAssembly. A escolha busca uma primeira tela útil rapidamente, compartilhamento de tipos e validações e componentes reativos sem uma segunda base de negócio em JavaScript.

O servidor usará Axum e Tokio; a persistência, PostgreSQL e SQLx. Preços, quantidades, percentuais e índices usarão representação decimal exata. Arquivos serão privados, com versões, verificação de integridade e acesso autorizado. A [arquitetura](docs/arquitetura-rust.md) define as fronteiras e os testes. As versões concretas e compatíveis serão fixadas e compiladas na issue 01, sem depender permanentemente de `latest`.

Rust não substitui medição de desempenho. Tamanho inicial do WebAssembly, tempo até a tela útil, resposta a interações, consultas e relatórios têm metas testáveis. O front-end não deve carregar uma planilha inteira para depois filtrá-la no navegador.

## 5. Como implementar em ordem

1. Ler PRD, arquitetura, experiência e regras da issue.
2. Implementar a menor issue ainda não aceita. Todas as anteriores precisam estar aceitas e integradas.
3. Entregar a funcionalidade completa daquela etapa, incluindo interface, validação do servidor, persistência, migração, auditoria, permissões e testes aplicáveis.
4. Registrar na issue os testes, a versão revisada e as evidências. Uma lista marcada sem demonstração não basta.
5. Corrigir falhas encontradas por revisão independente e conferir novamente os casos afetados.
6. Integrar a alteração, fechar a issue e avançar ao próximo número.

As issues foram criadas antes das primeiras pull requests para preservar a sequência inicial #1 a #72. Novas correções e pull requests podem receber números maiores. Os números 01 a 72 no título e no manifesto permanecem como identificadores da ordem aprovada. Uma mudança na ordem exige revisar o manifesto, as dependências e os critérios de aceite afetados.

Não se deve abrir um desvio genérico para deixar um requisito obrigatório fora da versão 1.0. Se um critério não passa, a issue permanece aberta. Preparação e pesquisa podem ocorrer em paralelo; a integração das funcionalidades segue a ordem acordada.

## 6. O que a versão 1.0 cobre

- Empresas, usuários, clientes, fornecedores, profissionais, vínculos e alçadas.
- Empreendimentos, obras, ativos, projetos, documentos, ART/RRT e licenças.
- Catálogos de custos, composições, orçamentos, cotações e cronogramas.
- Contratos por preço unitário ou global, serviços de projeto, supervisão e consultoria.
- Ordens de início, diário, fotos, qualidade, paralisações e retomadas.
- Medições acumuladas, conferência, reajustes, supressões medidas e relatórios.
- Pré-aditivos, aditivos, reprogramações e controle de saldo.
- Faturas, contas a pagar e a receber, adiantamentos, retenções, baixas e conciliação.
- Recebimentos provisório e definitivo, rescisões e garantias.
- Manutenção com contratos quadro, pedidos, ordens e fiscalização.
- Revisões de projeto, solicitações técnicas, tarefas, riscos e dependências.
- Portal privado do cliente/fornecedor, exportação, integração e uso em campo.
- Assinatura do serviço, atendimento, privacidade, operação e implantação de clientes.

O escopo não inclui autoria CAD/BIM, cálculo estrutural, folha de pagamento, contabilidade societária, emissão fiscal nacional completa para todos os municípios ou integração com órgãos públicos. Esses sistemas podem fornecer ou receber documentos por interfaces autorizadas; não são requisitos ocultos para completar o fluxo privado definido aqui.

## 7. Quando estará pronto para uso comercial

A issue 72 só poderá ser aceita depois das anteriores, com a versão efetivamente publicada verificada. Deve existir comprovação de duas organizações independentes percorrendo o ciclo completo, cobrança do serviço conciliada, termos e atendimento definidos, restauração de backup bem-sucedida, controle de acesso testado, metas de experiência e desempenho atendidas e nenhum defeito impeditivo em aberto.

O [documento de prontidão](docs/prontidao-comercial.md) define as evidências e os responsáveis. A conclusão das 72 implementações, com esses critérios cumpridos, constitui a entrega da versão 1.0. A manutenção do serviço, o suporte e as futuras atualizações continuam após o lançamento.

## 8. Uso das referências e dados

Os PDFs oficiais não foram incorporados ao repositório. Há links, identificação, localização das regras e hashes dos arquivos consultados. Tabelas de preços antigas são referências históricas; não se tornam preços atuais por aparecerem no catálogo. Importadores precisam guardar origem, competência, localidade, regime e direito de utilização da base.

Exemplos, testes automatizados e arquivos do repositório devem usar dados fictícios. Pilotos e verificação operacional usam dados reais apenas com autorização, ambiente controlado e evidências restritas; as issues registram referências e resultados, sem anexar documentos sensíveis. Não incluir contratos reais, CPFs, senhas, registros de clientes ou arquivos de terceiros sem autorização e finalidade definida. Acesso público ao portal do SICOP não autoriza copiar seu código, marca ou cadastros privados.
