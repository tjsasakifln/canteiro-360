# Cadastros e carteira de obras

## Cadastros de partes, issue 11

Abra **Cadastros** depois de selecionar a empresa. A busca considera nome, CPF e CNPJ; as páginas têm até 50 registros. O cadastro pode reunir as funções de cliente, fornecedor e profissional. Os dados da pessoa ficam separados da identidade usada para entrar no sistema.

Membros vigentes podem consultar a base da empresa. Criar, importar, atualizar e inativar exige administração e verificação em duas etapas. CPF e CNPJ são opcionais no cadastro inicial; se informados, precisam passar pela validação de formato e dígitos verificadores no servidor. Profissionais somente podem ser pessoas. Conselho, registro, UF e início da atuação são necessários para que o perfil seja considerado vigente em uma designação.

O identificador fiscal fica em texto, com zeros iniciais preservados e letras normalizadas para maiúsculas. Formatação brasileira completa e texto sem máscara são aceitos; máscara incompleta, caracteres desconhecidos e dígitos incorretos são rejeitados. Isso verifica a estrutura do documento, sem consultar a situação cadastral na Receita Federal.

Na mesma empresa, um CPF ou CNPJ já existente é localizado e reutilizado. O comando de criação não sobrescreve esse cadastro com os dados recém-digitados. Em outra empresa, o identificador não revela nem reutiliza registros da primeira.

### Contatos, importação e inativação

Cada cadastro admite até dez contatos. A importação recebe até cem linhas no formato `tipo;nome;CPF ou CNPJ;funções`, sem cabeçalho. Tipos: `pessoa` ou `empresa`. Funções: `cliente`, `fornecedor`, `profissional`, separadas por vírgula quando houver mais de uma. Todos os registros são validados antes de qualquer gravação. Uma linha inválida impede a importação inteira.

Inativar conserva a identidade, o histórico e os relacionamentos. Um profissional inativo ou fora de seu período não deve ser usado em uma nova designação. Seu cadastro não cria usuário, convite, participação ou permissão.

O arquivo de exportação usa JSON para preservar os identificadores como texto, inclusive CNPJ alfanumérico e zeros iniciais. Aplica a busca corrente e revalida a empresa e o vínculo no momento da transferência. Conta bancária e suas versões não integram essa exportação. Consultas acima de dez mil cadastros exigem refinar a busca.

### Dados bancários

Somente administradores com verificação em duas etapas consultam ou alteram contas de fornecedores. Alterações exigem que o fornecedor esteja ativo; a inativação preserva a consulta às versões anteriores. A restrição de leitura também está na política de linhas do PostgreSQL. Nova conta exige banco, agência, número, titular, documento do titular, motivo e versão atual do cadastro.

Cada alteração cria uma versão bancária imutável. O histórico permite comparar a conta anterior e a atual, com autor, motivo e data. O cadastro recebe uma nova versão na mesma transação. O consumidor financeiro deverá referenciar a versão bancária usada no ato, sem reconstruir comprovantes antigos a partir da conta atual; essa integração pertence aos módulos financeiros e à homologação da issue 70.

### Fontes e vetores

O vetor `12.ABC.345/01DE-35` e a regra de ASCII menos 48, pesos cíclicos de 2 a 9 e módulo 11 são conferidos no [Manual de Cálculo do DV da Receita Federal](https://www.gov.br/receitafederal/pt-br/centrais-de-conteudo/publicacoes/documentos-tecnicos/cnpj/manual-dv-cnpj.pdf/@@download/file), páginas 3 e 4, publicado em 05/11/2024. O vetor numérico `12.345.678/0001-95` também aparece na pergunta 17 de [Perguntas e Respostas do CNPJ alfanumérico](https://www.gov.br/receitafederal/pt-br/centrais-de-conteudo/publicacoes/perguntas-e-respostas/cnpj/cnpj-alfanumerico.pdf). Ambos são exercitados no domínio e no serviço de aplicação; a exportação mantém os valores canônicos.

Rastreabilidade: OP01, BR01 e BR02 da matriz. A autorização administrativa e os limites de página/importação são decisões do produto, sem atribuição ao SICOP.

## Carteira, issue 12

Abra **Carteira**. A referência de obra e as concessões são administradas em **Permissões**. Cada referência do tipo obra pode originar uma única entrada na carteira. Conceda explicitamente **Consultar** e, para edição, **Gerir carteira** ao papel de gestor. Ser administrador ou membro da empresa não concede essas ações automaticamente.

O registro reúne empreendimento, título, ativo, relação com o ativo (próprio, locado ou de terceiro), centro de custo, identificação externa, situação, endereço, trecho, dimensão, unidade e municípios. A situação da carteira é uma classificação de acompanhamento. Marcar uma obra como concluída não registra aceite técnico, medição, fatura ou recebimento.

### Rateio e localização

O rateio recebe linhas `município;UF;percentual;dimensão`, com vírgula decimal na interface. Município e UF não podem se repetir. Percentuais e dimensões têm até seis casas decimais. O conjunto precisa fechar exatamente 100% e a dimensão total; o formulário apresenta a diferença faltante ou excedente. Os cálculos usam o motor decimal exato compartilhado com o servidor.

Latitude e longitude são opcionais em conjunto. Se informadas, seus limites são respectivamente -90 a 90 e -180 a 180, com até oito casas decimais. Zero é uma coordenada válida e não representa ausência. Campos vazios permanecem vazios.

O mapa é uma grade privada de coordenadas em SVG. Seus pontos vêm exclusivamente da lista autorizada, sem consultar mapas externos ou enviar localização a terceiros. A lista textual permite escolher e editar obras pelo teclado; nenhuma operação exige acertar um ponto no mapa.

### Contratos e acesso

Uma obra admite múltiplos vínculos de contratos, inclusive contratos relacionados originalmente a outra obra da mesma empresa. O relacionamento não copia a obra e não cria orçamento, contrato definitivo ou custo. Cada par obra/contrato é único.

Busca, filtro de situação, contagem, mapa, consulta direta e histórico aplicam **Consultar** ao mesmo escopo. Vínculos de contratos sem acesso não são revelados. Para editar uma obra que já contém vínculos, o gestor também precisa consultar todos esses contratos, evitando que um formulário parcial remova relacionamentos que ele não pode ver.

Rastreabilidade: OP02 e ORC04 da matriz. O mapa privado e a classificação administrativa são decisões próprias desta implementação.

## Gravação e revisão

As duas áreas validam sessão, empresa, participação vigente, versão da seleção e autorização dentro da transação que grava. A identidade da tentativa e o conteúdo ficam registrados: repetir a mesma tentativa devolve o mesmo recibo; reaproveitá-la com outro conteúdo gera conflito. Uma versão antiga não sobrescreve o registro atual. Atualizar a versão da empresa ou de sua seleção não muda a identidade da tentativa: a mesma intenção, com contexto vigente e autorização revalidada, recupera o recibo anterior.

O usuário conserva os campos quando ocorre erro. A interface não transfere silenciosamente um rascunho para outra empresa depois de uma troca de contexto. Respostas de histórico ou consulta bancária só preenchem o registro e a empresa aos quais a consulta se referia. Ao sair da tela, respostas pendentes são descartadas. Vínculos e concessões são conferidos novamente depois de leituras que possam aguardar bloqueios; um prazo expirado durante a espera impede liberar os dados.

As revisões e os recibos usam tabelas protegidas contra atualização, remoção e truncamento. A consulta de histórico deriva a autoria da identidade autenticada, sem aceitar autoria informada pelo navegador.
