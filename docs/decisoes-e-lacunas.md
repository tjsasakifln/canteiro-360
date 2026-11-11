# Decisões, precedência e limites de equivalência

Este registro resolve como o produto será implementado onde há contradição, detalhe ausente ou adaptação indispensável. Não são pendências silenciosas nem autorização para omitir funções. Uma nova evidência pode alterar a decisão, mas exige revisão do requisito, das issues e dos testes afetados.

| ID | Questão e evidência | Decisão executável | Issue e prova |
|---|---|---|---|
| D01 | 2016 exigia quantitativos completos na final; pacote 02/2017 admite termo com saldo. | Aplicar fluxo 2017: final com saldo identificado, termo apropriado e liberação explícita. | 48; final com saldo e sem liberação prematura |
| D02 | Medições 2010 p.35 recomenda registro mensal zero; pacote 01/2019 exige total positivo. | Separar competência sem avanço de medição financeira positiva. Encerramento zero tem registro técnico, sem cobrança fictícia. | 31, 36 e48; não gerar título de zero nem impedir encerramento legítimo |
| D03 | SICOP informa truncagem, sem escala e ponto de aplicação suficientes. | C360-DECIMAL-1 usa diferença de valores acumulados truncados por segmento. Sem alegar igualdade centavo a centavo com algoritmo não publicado. | 05 e32; vetores independentes |
| D04 | Contratos 2025 p.35 combina exemplo mensal civil e tabela 30/60/90/180/365; IN 04/2026 art. 5 exige prazo em dias sem alterar termo final. | Separar cadência do cronograma e prazo do contrato. Orçamento padrão 30 dias; programação mensal privada civil. Preservar unidade/âncora/data final do instrumento e registrar dias equivalentes. | 05,18,23,26; janeiro/fevereiro, âncora 31, mês de 28 dias e ano bissexto |
| D05 | Guia distingue copiar com atualização e duplicar preservando preços. | Ações separadas, prévia, identificação da tabela nova e nenhuma mudança no original. | 20; tabela 10 → 11 mantém duplicação em 10 |
| D06 | Variação de escopo e revisão temporal aparecem com vocabulário distinto entre guias. | Entidades separadas, com vínculo de cada variação à obra pertinente. Etapas da mesma obra não viram obras artificiais. | 18 e24; orçamento multiobra e histórico temporal |
| D07 | Manual Empreitadas 2025 publica estrutura e progressão, não alocação completa dos resíduos. | Valor global por etapa; diferença acumulada; distribuição interna por maiores restos sem dupla soma. Etapa medida não é reprecificada. | 25,33,40; avanço 20 → 45%, suplemento contratual e total fechado |
| D08 | Ordem de BDI, desconto, fórmulas de composições e limites numéricos não está integralmente publicada. | Fórmulas e escalas em `regras/calculos.md`, entradas classificadas e política versionada. | 05,17,51;100×1,2×0,9=108; proibir dupla incidência |
| D09 | Fórmula exata e prorrata de reajuste não estão suficientemente especificadas. | Índice simples I/I0, competência e data próprias, provisório explícito e complemento pela diferença; divisão do período requer execução evidenciada. | 35 e43; provisório 800 → definitivo 950 gera 150 |
| D10 | Contagem de dias sobrepostos, parcial e parada aberta não é detalhada. | Intervalo [início,retomada), proibir sobreposição por frente, distinguir projeção e prazo formal. | 30 e42; parada 10 → 13 soma 3 dias uma vez |
| D11 | ART/RRT tem campos legados e dispensas de formulário por tipo. | Formulário não determina dispensa legal. Configuração por atividade e evidência, com registro profissional específico. | 14 e27; exigibilidade por escopo, sem falso documento |
| D12 | Limites 25%,60 meses, autoridades e tramitações públicas aparecem em fontes históricas. | Alçadas, limites e cláusulas privadas explícitos; nenhuma obrigação pública automática. | 08,23,38-40; misto+100/-100 não vira alteração sem acréscimo |
| D13 | Menus de faturas/pagamentos são descritos, mas integração pública não especifica todo o financeiro privado. | Contas operacionais, alocação, retenções, adiantamentos e caixa próprios; contabilidade/fiscal completo fora do escopo. | 44-47; conciliação sem dupla contagem |
| D14 | App permite enviar validação incompleta, mas fonte contém ambiguidade sobre elegibilidade da medição. | Upload parcial é rascunho; servidor decide elegibilidade pela matriz e versão. Nenhum pendente é aprovado implicitamente. | 34 e62; pacote parcial, reenvio e revogação offline |
| D15 | Fontes antigas enviam senha por e-mail e dão consulta ampla por órgão. | Recuperação segura por token e consulta privada por concessão; MFA e segregação moderna. | 06-08; não enumerar contas, reuso de token e empresa errada |
| D16 | Dois endereços do catálogo retornaram 404. | Registrar indisponibilidade; usar fluxo 2021 acessível e modelo próprio de empreendimento, sem inventar conteúdo perdido. | 12,52-54; fontes SIC-018 e045 com status real |
| D17 | Não localizado manual detalhado suficiente de Gestão de Projetos. | Definir requisitos próprios para escopo, entregas, revisões, tarefas, riscos e mudanças. Não atribuir equivalência interna desconhecida. | 13,55-57; jornadas de projeto com contrato e obra |
| D18 | Regras exatas de importação Excel não são publicadas no manual que mostra o botão. | Formato privado versionado, simulação, validação e atomicidade; sem promessa de importar qualquer saída SICOP. | 19 e60; arquivo inválido não aplica parte silenciosamente |
| D19 | Transparência e integrações de governo não se aplicam automaticamente a clientes privados. | Portal e mapas privados; API com escopos e autorização; nenhuma conta pública usada. | 58,59,61; divulgação e envio apenas ao destino autorizado |
| D20 | Linguagem e experiência exigidas pelo responsável pelo produto. | Rust integral na lógica, Leptos SSR/WASM, design próprio e metas mensuráveis. Código gerado de interoperabilidade é permitido. | 01-05,63-64; compilação, jornada e desempenho |

| D21 | IN SIE 04/2026 arts. 4 a 15 atualiza prazos, pausas, registro e responsabilidades; art. 13 contém regra geral e exceção. | Documento válido prevalece; execução contida na vigência; dias de pausa consolidados por registro privado equivalente ao apostilamento, sem estender vigência. Para extensão de vigência ou outra prorrogação, aditivo. Nenhum rito público automático. | 23,27,30,40,42; pausa de 3 dias, vigência inalterada, bloqueio de execução durante pausa e aditivo quando exceder vigência |

## Alterar uma decisão

Registrar fonte nova ou problema reproduzível, comportamento atual, proposta, efeitos nos dados e testes de referência. Aprovação do responsável pelo produto não pode substituir obrigação legal aplicável. Se afetar documento consolidado, preservar a versão histórica e criar política nova. Nenhuma atualização de dependência pode mudar resultados financeiros silenciosamente.

## Interpretação de “sistema pronto”

A especificação fica pronta quando os documentos, fontes, dependências e critérios são coerentes e revisados. O sistema fica pronto para uso comercial quando as 72 issues foram implementadas e aceitas, inclusive publicação e pilotos. Esses estados não são intercambiáveis. Não há alegação de software funcional nesta entrega documental.
