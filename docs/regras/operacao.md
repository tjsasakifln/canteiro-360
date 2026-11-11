# Operação, manutenção e projetos

Regras complementares às matrizes de contratos e orçamentos. Datas de acesso e integridade das fontes estão no [catálogo](../fontes/README.md). Regras de projeto sem manual público detalhado localizado são decisões próprias, identificadas abaixo.

## OP01 | Adaptada | Issue 11

Consultar identificador antes de cadastrar contratado ou profissional.

**Fonte:** [SIC-036](https://www.sicop.sc.gov.br/download/sicop/Pacote_de_alteracao_Sicop_2019_04_Cadastro_De_Novos_Contratados_e_Profissionais.pdf) | pp.1-3

**Canteiro 360:** Deduplicar CPF/CNPJ dentro da organização, preservando perfis e acesso; consultar não revela cadastros de outro cliente.

**Aceite:** Cadastro duplicado na mesma organização orienta reutilizar; mesma identidade em outra organização não expõe seus dados.

## OP02 | Adaptada | Issue 12

Obra vincula bem ou via e localização geográfica.

**Fonte:** [SIC-044](https://www.sicop.sc.gov.br/download/sicop/Pacote_de_alteracoes_05_2016_Bem_Publico.pdf) | pp.1-2; SIC-043 p.1

**Canteiro 360:** Ativo privado próprio, locado ou de terceiro; obra linear admite trechos e coordenadas, com mapa privado.

**Aceite:** Duas obras no mesmo ativo permanecem distintas; coordenada inválida é rejeitada e convidado não visualiza obra sem vínculo.

## OP03 | Adaptada | Issue 07

Solicitação de acesso identifica pessoa, organização, perfis e decisão.

**Fonte:** [SIC-030](https://www.sicop.sc.gov.br/download/sicop/Solicitacao_Acesso_Usuario_01_2020.pdf) | pp.1-6

**Canteiro 360:** Convite ou solicitação verificada, com escopo e validade; administrador não autoriza acesso à organização alheia.

**Aceite:** Convite expirado ou direcionado a outro destinatário não concede acesso e não informa existência de contratos.

## OP04 | Adaptada | Issue 06

Recuperação/desbloqueio de credencial possui fluxo próprio.

**Fonte:** [SIC-023](https://www.sicop.sc.gov.br/download/sicop/Pacote_de_alteracoes_02_2021_Troca_ou_Desbloqueio_de_Senha.pdf) | pp.1-3; SIC-047 pp.1-2

**Canteiro 360:** Token aleatório criptográfico com pelo menos 256 bits, expiração de até 15 minutos, hash persistido e uso único; controle de tentativas e revogação, sem enviar senha nem contornar MFA.

**Aceite:** Reusar token falha; mesmo retorno público para e-mail cadastrado ou ausente; senha não aparece no e-mail nem no log.

## OP05 | Adaptada | Issue 08

Consulta e edição têm perfis e vínculos distintos.

**Fonte:** [SIC-047](https://www.sicop.sc.gov.br/download/sicop/Pacote_de_alteracoes_02_2016_Seguranca.pdf) | pp.2-5

**Canteiro 360:** Permissão explícita por organização e escopo; consulta ampliada somente por concessão deliberada, sem herança de transparência pública.

**Aceite:** Leitor vê registros autorizados e não consegue alterar por API; gestor de um contrato não administra outro por trocar ID.

## OP06 | Adaptada | Issue 03

Indicador de etapas mostra contexto e conclusão; a ordem visual não exige executar todos os menus em sequência.

**Fonte:** [SIC-029](https://www.sicop.sc.gov.br/download/sicop/Etapas_do_Modulo_de_Obras_Publicas_02_2020.pdf) | pp.1-2

**Canteiro 360:** Jornada orientada à tarefa, navegação livre entre seções permitidas; bloquear apenas dependência real do ato.

**Aceite:** Usuário abre documentos antes de concluir cadastro; concluir medição continua bloqueado enquanto faltar comissão válida.

## OP07 | Adaptada | Issue 52

Usuários de manutenção precisam de vínculo com o bem.

**Fonte:** [SIC-022](https://www.sicop.sc.gov.br/download/sicop/Manual_de_Manutencao_Coordenador_SED.pdf) | pp.4-8; SIC-025 pp.4-8; SIC-026 pp.4-8

**Canteiro 360:** Vínculo temporal por ativo e função: solicitante, coordenador, fiscal, executor e aprovador.

**Aceite:** Usuário vinculado ao ativo A não pode cadastrar ou aprovar pedido do ativo B por URL ou API.

## OP08 | Adaptada | Issue 52

Pedido registra data, descrição, tipo, fotos, parecer e encaminhamento; exclusão antes do encaminhamento.

**Fonte:** [SIC-026](https://www.sicop.sc.gov.br/download/sicop/PACOTE_DE_ALTERACAO_SICOP_05_2020_Manual_de_Manutencao-Diretor.pdf) | pp.10-15

**Canteiro 360:** Rascunho pode ser descartado com auditoria; após envio, cancelamento justificado preserva a demanda e seus anexos.

**Aceite:** Pedido encaminhado não desaparece por exclusão; sem tipo ou dados exigidos permanece rascunho com indicação do campo.

## OP09 | Adaptada | Issue 52

Coordenador analisa pedido, cancela com parecer ou autoriza elaborar OS.

**Fonte:** [SIC-022](https://www.sicop.sc.gov.br/download/sicop/Manual_de_Manutencao_Coordenador_SED.pdf) | pp.12-13

**Canteiro 360:** Coordenador privado decide dentro da alçada; análise da necessidade precede aprovação do escopo e preço da OS.

**Aceite:** Aprovar a demanda não autoriza fornecedor a executar; recusa registra data, autor e motivo.

## OP10 | Adaptada | Issue 53

Fiscal seleciona ata, lote, grupos, serviços e quantidades e encaminha OS.

**Fonte:** [SIC-025](https://www.sicop.sc.gov.br/download/sicop/PACOTE_DE_ALTERACAO_SICOP_07_2020_Manual_de_Manutencao-Fiscal.pdf) | pp.10-16

**Canteiro 360:** Acordo quadro mantém catálogo, limite, consumido e reserva remanescente. Envio reserva em transação; disponível = limite - consumido - reserva. Cancelamento libera apenas reserva não consumida.

**Aceite:** Pedidos de 20 mil e 15 mil concorrendo por 30 mil não reservam 35 mil. Limite 100, OS 80, execução confirmada 50 e cancelamento do remanescente 30 deixam disponível 50, nunca 100; reenvio não repete a liberação.

## OP11 | Adaptada | Issue 53

Coordenador cancela OS, emite para execução ou solicita adequações.

**Fonte:** [SIC-022](https://www.sicop.sc.gov.br/download/sicop/Manual_de_Manutencao_Coordenador_SED.pdf) | pp.15-17

**Canteiro 360:** Decisão sobre versão exata com valor, prazo e escopo; adequação retorna ao fiscal e invalida decisão antiga.

**Aceite:** Fornecedor não inicia antes de OS autorizada; alteração após aprovação exige nova versão e decisão.

## OP12 | Adaptada | Issue 54

Fiscal registra quantidades executadas e finaliza OS com parecer.

**Fonte:** [SIC-025](https://www.sicop.sc.gov.br/download/sicop/PACOTE_DE_ALTERACAO_SICOP_07_2020_Manual_de_Manutencao-Fiscal.pdf) | pp.17-18

**Canteiro 360:** Confirmação técnica da execução transfere reserva para consumido uma única vez; divergência mantém reserva protegida. Concluir OS documenta execução/aceite, sem gerar pagamento; medição e fatura não consomem o acordo novamente.

**Aceite:** OS concluída com quantidade 10 não permite medir 11; concluir duas vezes não duplica consumo do acordo.

## OP13 | Adaptada | Issue 54

Medição de manutenção permanece em conferência até validar com contratada; documentos e NF são conferidos antes do pagamento.

**Fonte:** [SIC-017](https://www.sicop.sc.gov.br/download/sicop/Pedido_de_manutencao_04.03.2021.svg) | Fluxo 04/03/2021: fiscalização, validação e documentos

**Canteiro 360:** Usar confirmação registrada, divergências e versão aceita; faturamento e caixa próprios substituem SGPe/SIGEF.

**Aceite:** Divergência da contratada mantém medição em revisão; upload de NF não baixa título nem aprova serviço.

## OP14 | Adaptada | Issue 59

Fluxo de manutenção reúne pedidos, medições, totalização e favoritos.

**Fonte:** [SIC-024](https://www.sicop.sc.gov.br/download/sicop/PACOTE_DE_ALTERACOES_01_2021_FLUXO_E_FAVORITOS.pdf) | pp.2-5

**Canteiro 360:** Somar pedidos sem duplicar OS já medidas; salvar favorito conserva contexto e obedece autorização atual.

**Aceite:** Pedido revogado some dos favoritos; soma gerencial não conta ao mesmo tempo valor da OS e sua medição como gastos distintos.

## OP15 | Adaptada | Issue 29

Advertência possui dados, fotos, relatório e histórico de envios.

**Fonte:** [SIC-028](https://www.sicop.sc.gov.br/download/sicop/PACOTE_DE_ALTERACAO_SICOP_2020_03_Historico_e_fotos_de_advertencia.pdf) | pp.1-4

**Canteiro 360:** Ocorrência/notificação contratual com versões e destinatário confirmado; reenviar informa versão e não substitui prova anterior.

**Aceite:** Modificar advertência após envio cria revisão; comprovante anterior mantém hash, destinatário e data.

## OP16 | Adaptada | Issue 28

Fotografia pode preencher latitude/longitude quando presentes no arquivo.

**Fonte:** [SIC-028](https://www.sicop.sc.gov.br/download/sicop/PACOTE_DE_ALTERACAO_SICOP_2020_03_Historico_e_fotos_de_advertencia.pdf) | p.2

**Canteiro 360:** Guardar origem da localização, preservar original e permitir versão de divulgação sem metadados sensíveis.

**Aceite:** Imagem sem coordenada não recebe localização inventada; documento enviado ao cliente usa política de exposição definida.

## OP17 | Adaptada | Issue 62

Aplicativo recebe contratos explicitamente selecionados e identifica importação.

**Fonte:** [SIC-019](https://www.sicop.sc.gov.br/download/sicop/Manual_Aplicativo_de_Fiscalizacao_de_Obras.pdf) | pp.4-12

**Canteiro 360:** Conjunto de campo autorizado por usuário, dispositivo, validade e versão; minimizar armazenamento local.

**Aceite:** Somente contratos selecionados e autorizados ficam disponíveis; revogação bloqueia sincronização de alterações.

## OP18 | Adaptada | Issue 34

Avaliação de medição distingue serviços aprovados, reprovados e não avaliados; reprovação tem observação.

**Fonte:** [SIC-019](https://www.sicop.sc.gov.br/download/sicop/Manual_Aplicativo_de_Fiscalizacao_de_Obras.pdf) | pp.13-19

**Canteiro 360:** Botões acessíveis substituem gesto obrigatório; avaliação de item é distinta da aprovação financeira de toda medição.

**Aceite:** Reprovar sem motivo é rejeitado; confirmar item não libera pagamento; itens pendentes continuam visíveis no resumo.

## OP19 | Adaptada | Issue 62

Envio pode informar avaliações faltantes; novo ciclo exige nova exportação.

**Fonte:** [SIC-019](https://www.sicop.sc.gov.br/download/sicop/Manual_Aplicativo_de_Fiscalizacao_de_Obras.pdf) | pp.20-21

**Canteiro 360:** Sincronizar avaliação parcial como rascunho identificado, com alerta; nenhuma falta vira aprovação implícita.

**Aceite:** Reenvio do pacote não duplica fotos; conteúdo parcial não permite consolidar medição com requisito faltante.

## OP20 | Adaptada | Issue 59

Busca por palavra-chave encontra menus.

**Fonte:** [SIC-038](https://www.sicop.sc.gov.br/download/sicop/Pacote_de_alteracoes_02_2019_Menu_de_Busca.pdf) | pp.1-2

**Canteiro 360:** Busca global por tarefa e registro autorizado, navegação por teclado e resultado com contexto.

**Aceite:** Resultado por nome de obra não revela entidade não autorizada nem em contagem, sugestão ou trecho.

## OP21 | Adaptada | Issue 69

Suporte pode receber chamado, anexos e comentários e informar estado.

**Fonte:** [SIC-031](https://www.sicop.sc.gov.br/download/sicop/Guia_SICOP_Abrir_Chamado_Suporte_01_2020.pdf) | pp.1-12; SIC-035 pp.1-7

**Canteiro 360:** Atendimento com protocolo, prioridade, histórico e acesso limitado; captura de tela é opt-in e redigível.

**Aceite:** Chamado é rastreável até conclusão; anexos não incluem captura automática de dados privados sem conferência.

## OP22 | Adaptada | Issue 61

Integrações públicas associam processo e remetem documentos/valores.

**Fonte:** [SIC-037](https://www.sicop.sc.gov.br/download/sicop/Pacote_de_alteracao_Sicop_2019_03_Integracao_Entre_SICOP_e_SGPe.pdf) | pp.1-2; SIC-014 e SIC-004

**Canteiro 360:** Não conectar contas públicas; manter referência externa e API privada com exportação assinada pelo contexto do cliente.

**Aceite:** Integração externa não pede senha SICOP/SGPe; destino não autorizado não recebe dados por webhook.

## OP23 | Decisão | Issue 13

Manuais públicos localizados não especificam integralmente o módulo Gestão de Projetos.

**Fonte:** [PRODUTO](../../PRD.md) | PRD: projetos; issue 13

**Canteiro 360:** Implementar escopo, entregas, responsáveis e aprovação como capacidade própria, ligada à obra e contrato.

**Aceite:** Projeto pode existir antes da obra; entrega aceita referencia revisão e responsável e não cria medição sozinha.

## OP24 | Decisão | Issue 55

Revisões de projeto precisam governar a informação liberada para obra.

**Fonte:** [PRODUTO](../../PRD.md) | PRD: projetos; issues 55-57

**Canteiro 360:** Estados elaboração, revisão, devolvido, aprovado, emitido e substituído; pedido de esclarecimento e distribuição rastreável.

**Aceite:** Equipe identifica revisão vigente; link anterior mostra substituição e aprovação antiga não autentica revisão nova.

## OP25 | Decisão | Issue 56

Planejamento precisa permitir dependências verificáveis.

**Fonte:** [PRODUTO](../../PRD.md) | PRD: projetos; issue 56

**Canteiro 360:** Tarefas com início/fim, duração, predecessoras, calendário, responsáveis e marcos; rejeitar ciclos e mostrar caminho crítico calculado.

**Aceite:** A precede B e B precede A é recusado; atraso de predecessora atualiza previsão sem mudar a linha de base aprovada.

## OP26 | Decisão | Issue 44

Controle privado exige trilha financeira independente dos estados técnicos.

**Fonte:** [PRODUTO](../../PRD.md) | PRD: financeiro; issues 44-47

**Canteiro 360:** Origem financeira tipada distingue serviço/reajuste liberado, compromisso direto aprovado, autorização de adiantamento, retenção liberada e ajuste. Fatura, título, alocação, liquidação e estorno têm registros próprios e guardas por natureza.

**Aceite:** Um saldo de medição de 1000 suporta duas faturas 600+400, nunca uma terceira; não gera caixa sem liquidação.

## OP27 | Decisão | Issue 48

Encerramento técnico com valor financeiro zero deve ser possível sem falsificar execução.

**Fonte:** [PRODUTO](../../PRD.md) | PRD: encerramento; issues 48-50

**Canteiro 360:** Registro técnico de encerramento guarda contrato, competência, data de conclusão, comissão, saldo, documento e última medição financeira opcional. Recebimento aponta para referência discriminada de final financeira ou encerramento técnico; data não antecede sua conclusão e não cria título fictício.

**Aceite:** Contrato 100% medido no período anterior pode encerrar tecnicamente com zero novo a faturar e pendências de pagamento visíveis.

## OP28 | Decisão | Issue 47

Orçamento estimado, compromisso, medição, fatura e caixa não são parcelas somáveis de custo.

**Fonte:** [PRODUTO](../../PRD.md) | PRD: quadro de gestão; issue 47

**Canteiro 360:** Relatórios exibem estágios e origem; compras diretas e subcontratos usam identificador de compromisso para impedir dupla contagem.

**Aceite:** Subcontrato 1000 medido 600 faturado 600 pago 400 mostra cada estágio separado, não custo total 2600.
