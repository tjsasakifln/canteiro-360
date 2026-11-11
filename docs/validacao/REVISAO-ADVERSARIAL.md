# Revisão adversarial da especificação

**Data:** 11/11/2026. **Objeto:** documentos, requisitos, modelo, cálculos de referência e sequência de implementação. **Resultado:** aprovado para orientar a implementação, depois das correções e da reconferência descritas abaixo.

Esta aprovação não é de um aplicativo já construído. Compilação, testes do motor Rust, isolamento em execução, desempenho, acessibilidade do produto, restauração, pilotos e operação comercial continuam exigidos nas issues correspondentes. A [issue 72](https://github.com/tjsasakifln/canteiro-360/issues/72) só pode ser aceita com evidência da versão efetivamente publicada.

## 1. Como a revisão foi feita

Três agentes revisores trabalharam em frentes complementares: contratos e operação; orçamentos, medições e cálculos; produto brasileiro, segurança, experiência e comercialização. Cada um examinou documentos de outros autores, procurando contradições, resultados monetários incorretos, dependências futuras escondidas e critérios que permitissem aprovar uma entrega incompleta. A autoria de um documento não foi contabilizada como sua revisão independente.

As três passagens produziram 32 apontamentos, com sobreposição entre alguns problemas. As correções foram incorporadas aos documentos, aos registros JSON, à matriz e aos corpos das issues. Os revisores reabriram os trechos alterados e repetiram os casos afetados. Os 11 apontamentos de contratos, os 11 de cálculo e sequência e os 10 de produto foram encerrados no reteste local. Não permaneceu achado impeditivo identificado nessas revisões.

A coordenação conferiu pessoalmente os pontos decisivos, recalculou os exemplos monetários e de datas, validou vínculos e sincronismo e realizou a revisão editorial final. A comparação dessa última passagem confirmou alteração apenas de espaços, preservando IDs, URLs, valores e hashes das fontes. Os resultados observados da estrutura, dos exemplos e da publicação são registrados em [verificações](verificacoes.json).

## 2. Fontes e limites de equivalência

O inventário contém 50 referências SICOP/SIE: 48 endereços únicos do catálogo oficial, a apostila completa histórica de contratos e a IN SIE 04/2026 encontrada na legislação oficial. Foram recuperados 48 arquivos e conferidos seus SHA-256. Dois endereços publicados retornaram 404, registrados sem presumir o conteúdo ausente. Ver [catálogo](../fontes/README.md) e [registro de fontes](../fontes/sicop.json).

A revisão confrontou regras históricas com pacotes que as alteram expressamente. A IN SIE 04/2026 foi lida integralmente e incorporada nos pontos de prazos, documentos, paralisação e responsabilidades. Seu rito público foi adaptado ao instrumento privado, sem impor DOE, SGPe, e-Sfinge ou prorrogação legal administrativa a clientes privados.

Os documentos distinguem 52 regras documentadas, 79 adaptações e 61 decisões próprias, totalizando 192 requisitos rastreáveis. A classificação separa o que a fonte afirma do que o produto decide. Não se declarou igualdade de algoritmos internos não publicados, nem se atribuiu ao módulo de Gestão de Projetos um manual detalhado que não foi localizado. Essas limitações e as decisões executáveis constam de [decisões e lacunas](../decisoes-e-lacunas.md).

## 3. Problemas que alteravam o comportamento

O quadro consolida problemas sobrepostos das três revisões. Todos os itens abaixo foram corrigidos e reconferidos na especificação.

| Problema encontrado | Correção incorporada | Prova de referência ou localização |
|---|---|---|
| Prazo de execução e vigência podiam ser tratados como uma única duração | Campos independentes; execução contida na vigência; conversão de meses preserva o termo final | CT-05; cálculos §9; issues 23/27; V28/V29 |
| Dias de paralisação podiam ampliar vigência automaticamente | Registro privado de ajuste consolida execução, com memória e cronograma; extensão de vigência exige instrumento próprio | PA-01/02, AD-06; issues 30/40/42 |
| Aditivo de prazo poderia ser aplicado antes de existir cronograma atualizado | A issue 26 já fornece versões; a 40 exige memória e cronograma; a 42 amplia a reprogramação integrada | AD-06, CR-01; issues 26/40/42 |
| Percentual físico não podia cair mesmo após aumento formal do denominador | Permitir queda relativa, preservando quantidade e valores reconhecidos | 40 unidades sobre 100/200 resultam em 40%/20%; MED06; V30 |
| Final liberada era limitada a um título, contrariando faturamento parcial | Uma origem econômica, várias alocações limitadas pelo saldo | Origem 1.000 aceita 600 + 400 e recusa novo consumo; MED17; issues 44/48 |
| Reabrir medição antiga podia quebrar os acumulados seguintes | Só a última, sem sucessora consolidada nem efeito dependente, pode voltar à conferência | Acumulados 20/35 a PU 10 preservam total 350; V21; issues 31/36 |
| Reajuste confundia data-base, aniversário e índice mensal | Data-base escolhe I0; data de referência ancora ciclos; competência fica fixa durante o ciclo | V22 usa base dezembro/2024 e referência janeiro/2025; fevereiro/2026 deve 500, não 400 ou 600 |
| Supressão podia estornar reajuste pelo índice atual | Reverter reconhecimento e fator originais por segmento; nova execução calculada separadamente | Serviços 900, reajuste 95, bruto 995; V23; issues 41/43/70 |
| Encerramento sem nova cobrança exigia medição financeira fictícia | Referência discriminada entre final financeira e encerramento técnico sem cobrança | 100% medido em junho, conclusão em julho e dívida 500 preservada; RC-01; issues 48/49 |
| Cancelamento parcial de manutenção podia devolver saldo já consumido | Disponível = limite - consumido - reserva remanescente; confirmação técnica transfere reserva em consumo | Limite 100, OS 80, consumo 50, cancelamento 30 deixam disponível 50; V25 |
| Serviço percentual remetia a fórmula incompleta | Base acumulada elegível, exclusões explícitas, grafo sem ciclos e diferença de valores acumulados | Base 2.000 → 4.000 a 20% gera 400 no período; V24; issue 33 |
| Dois métodos decidiam onde colocar o centésimo residual | Maiores restos com desempate por posição estável e ID | 33,34; 33,33; 33,33 em valor e percentual; V10/V27 |
| Origem financeira única excluía compra direta, adiantamento e retenção | Tipar origens autorizadas e separar título, documento fiscal, execução e caixa | Compra direta 1.200 sem medição; adiantamento anterior à execução; issues 44-46 |
| Deduções podiam produzir título ordinário negativo | Limites conjuntos, saldo por origem e excesso preservado como crédito | Bruto 80, retenção 10, crédito 100: amortização 70, crédito restante 30, novo caixa zero; V26 |
| Parâmetros de preço tinham faixas indefinidas | Limites de custo, coeficiente, BDI, desconto e preço; crédito separado | Desconto 150% e PU negativo recusados; item gratuito identificado; cálculos §§2-3 |
| Exigência documental bloqueava orçamento antes da etapa prevista | ART/RRT aplicável é exigida ao aprovar; finalização técnica pode manter a pendência | SIC-048 p.4; ORC25/RT-04; issue 21 |
| Estado administrativo e links locais estavam inconsistentes | Preservar Pendência administrativa e eventos repetidos; corrigir caminhos do PRD | CT-03; modelo; issue 23; validação dos links relativos |
| Credenciais e recuperação não tinham contrato mínimo suficiente | Argon2id ou provedor aprovado; salt, custo, token de alta entropia, expiração, hash e proteção de MFA | Arquitetura §10; BR21; issue 06 |
| Mesma chave com pedido diferente podia retornar sucesso antigo | Unicidade por organização, ato e chave; comparar hash da entrada; conflito e revalidação do acesso | Arquitetura §5; BR18; issue 09 |
| Meta de acessibilidade podia passar com falhas A/AA não bloqueantes | Exigir todos os critérios A/AA aplicáveis; testar 320 CSS px e zoom 400% | UX10; issue 63; revalidação dos fluxos comerciais na 70/72 |
| Uso sem rede prometia mais do que a arquitetura sustentava | Conexão intermitente em aba preparada; abertura com rede; autorização local até 8 horas; rascunho até 7 dias | Experiência §7; arquitetura §9; issue 62 |
| Issue 70 exigia resultados de 71/72 e outros testes antecipavam módulos | Delimitar requisitos até a etapa atual e identificar cenários posteriores; matriz integral apenas no fechamento final | BR36 na 71 e BR38 na 72; integrações explícitas; issue 70 |
| Cobrança na 68 dependia dos termos entregues na 69 | Política mínima e teste técnico na 68; termos na 69; contratação e recebimento de cliente nos pilotos/liberação | BR31/BR33; issues 68-72 |
| Proibição genérica de dados reais contradizia piloto autorizado | Dados fictícios nos testes automatizados e no repositório; dados autorizados e evidências restritas no piloto | AGENTS, LEIAME; issues 71/72 |

## 4. Verificações da versão documental

- 72 etapas consecutivas, com dependência da imediatamente anterior e sem ciclo numérico.
- Identificação e correção de ciclos semânticos, inclusive o fechamento indevido de requisitos futuros na issue 70.
- 192 IDs únicos, com origem, fonte ou decisão, localização, regra privada, issue primária e aceite.
- Correspondência dos registros normativos com a matriz; regras de contratos, orçamento e operação sincronizadas entre Markdown e JSON.
- 30 vetores de referência. A coordenação recalculou independentemente os 25 casos numéricos/de datas; os cinco restantes registram predicados e estados que precisam de teste na implementação.
- Revisores também conferiram os cenários por referências próprias. O resultado esperado não foi apresentado como teste do futuro código Rust.
- Links locais e documentos referenciados conferidos. Indisponibilidades externas conhecidas estão identificadas no catálogo.
- Corpos das issues preservados em arquivos próprios para comparação com a publicação nativa.

## 5. O que continua exigido durante a implementação

As issues permanecem abertas até a entrega real de cada etapa. Versões das dependências e compilação SSR/WASM serão comprovadas na 01. Cada novo módulo precisa aplicar autorização, auditoria, precisão e concorrência com seus dados e fluxos reais. As regras comuns não ficam aprovadas antecipadamente para consumidores futuros.

A validação integrada da 70 não substitui os pilotos da 71 nem a verificação em produção da 72. A liberação final requer duas organizações independentes, termos e atendimento operantes, cobrança autorizada e conciliada, documento fiscal do serviço, restauração demonstrada, metas de experiência e desempenho atendidas e nenhum defeito impeditivo ou diferença monetária sem explicação.

Novas fontes ou falhas descobertas na implementação exigem correção rastreável, atualização dos testes afetados e nova revisão. Esta especificação não usa ausência de código como evidência de ausência de risco; define os critérios que deverão revelar esses riscos antes do lançamento.
