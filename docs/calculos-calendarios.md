# Cálculos, unidades e calendários

Este guia descreve a implementação da [issue 5](issues/005.md), os pontos de uso do motor e os limites das prévias. A regra é a [C360-DECIMAL-1](regras/calculos.md), uma decisão versionada do Canteiro 360. As referências do SICOP e as adaptações para contratos privados continuam registradas na [matriz de rastreabilidade](MATRIZ-RASTREABILIDADE.md).

O domínio calcula valores e datas. A autorização, o vínculo com a organização, o bloqueio transacional, a versão do registro e a persistência definitiva pertencem aos respectivos consumidores. Uma prévia correta não aprova uma medição, libera uma fatura nem modifica o prazo formal.

## 1. Onde usar e onde localizar

| Necessidade | Local |
|---|---|
| Conferir parcelas e cronograma mensal na interface | Rota `/conferencia`, também acessível pela página inicial |
| Recalcular o rateio no servidor | `POST /api/prototipo/calculos/rateio` |
| Recalcular o cronograma no servidor | `POST /api/prototipo/calculos/calendario` |
| Operações decimais e tipos com limites | [crates/domain/src/decimal.rs](../crates/domain/src/decimal.rs) |
| Quantidade associada à unidade | [crates/domain/src/units.rs](../crates/domain/src/units.rs) |
| Cadências, execução, pausas e fusos | [crates/domain/src/calendar.rs](../crates/domain/src/calendar.rs) |
| Pedidos e resultados das prévias | [crates/contracts/src/calculations.rs](../crates/contracts/src/calculations.rs) |
| Comparação com resultados independentes | [tools/calculation-verification](../tools/calculation-verification/) |

A interface apresenta exemplos fictícios, a interpretação das entradas, a regra aplicada e o destino do residual. O exemplo inicial de rateio usa R$ 100,00 em três parcelas. O exemplo de calendário vai de 31/01/2027 a 31/03/2027. Os exemplos e os valores iniciais dos campos são renderizados pelo servidor; depois da hidratação, as alterações são recalculadas nesta aba com o domínio compartilhado em Rust/WASM.

As prévias não gravam contratos ou pagamentos. Os limites de apresentação são **100 parcelas** e **36.600 dias** por consulta de calendário. A página oferece cronograma mensal; a API também aceita as demais cadências documentadas abaixo. Erros conservam o texto informado para permitir a correção.

## 2. Representação decimal e limites

O valor do domínio é `rust_decimal::Decimal`, encapsulado em tipos que validam a entrada e o resultado. `ExactDecimal` admite até 28 dígitos significativos na representação normalizada e escala de até 28 casas. Não há conversão financeira para `f32` ou `f64`, saturação nem redução automática de escala para fazer uma operação caber.

`BigInt` é usado somente como inteiro temporário para verificar a exatidão, calcular quocientes e comparar restos. Não amplia os valores financeiros admitidos, não é a representação persistida e não substitui os limites de `NUMERIC`. Um produto pode precisar de muitos dígitos temporários e ainda ter um resultado exato representável depois de remover zeros decimais; o motor verifica esse resultado antes de construí-lo.

| Tipo do domínio | Limite compatível com persistência | Escala máxima de entrada | Sinal e significado |
|---|---|---:|---|
| `Quantity` | `NUMERIC(20,6)` | 6 | Quantidade não negativa; usar com unidade |
| `UnitPrice` | `NUMERIC(20,6)` | 6 | Preço unitário não negativo; exibição do tipo preserva seis casas |
| `Money` | `NUMERIC(20,2)` | 2 | Valor em centavos; o tipo admite valores positivos ou negativos para ajustes, mas a operação ordinária valida o sinal permitido |
| `Percent` | `NUMERIC(12,6)` | 6 | Percentual não negativo; a operação impõe sua faixa específica |
| `IndexValue` | `NUMERIC(20,10)` | 10 | Índice estritamente positivo |
| `Factor` | `NUMERIC(20,10)` | 10 | Fator não negativo, inclusive zero produzido pelo arredondamento autorizado |

Esses limites correspondem aos tipos e à política de armazenamento. A issue 5 não cria registros comerciais nem declara implementadas as tabelas de todos os módulos consumidores.

Na API, os decimais são **textos com ponto**, como `"1234.56"`. Número JSON, expoente, separador de milhar, sinal `+`, espaços, zeros inteiros à esquerda e zero negativo são recusados. Zeros decimais finais são aceitos dentro da escala do tipo. Uma entrada com sete casas em `Quantity` é inválida mesmo quando a sétima casa é zero.

A interface interpreta `1.234,56` como `1234.56` pela convenção brasileira. Um ponto isolado, como `1.234`, exige esclarecimento pela própria entrada: `1.234,00` para milhar ou `1,234` para três casas decimais. A normalização do transporte não deve ser usada para adivinhar a convenção de um arquivo importado.

### Operações básicas

| Operação | Comportamento |
|---|---|
| `checked_add`, `checked_sub`, `checked_mul` | Resultado exato ou erro explícito de faixa/precisão |
| `checked_div_exact` | Aceita somente divisão decimal finita que caiba na política; `1 / 3` é erro |
| `divide_rounded(divisor, escala)` | Divisão com arredondamento explícito ao mais próximo, empate afastando de zero |
| `truncate(escala)` | Descarta casas em direção a zero |
| `round_away(escala)` | Arredonda ao mais próximo, empate afastando de zero |
| `fixed_text(escala)` | Formata sem arredondar; erro se a escala pedida descartaria informação |

Exemplo: `-0.335` truncado para duas casas produz `-0.33`; arredondado para duas casas produz `-0.34`. A existência dessas operações básicas não autoriza o consumidor a escolher outra regra monetária. Os pontos de arredondamento permanecem fixados na política.

## 3. Preços, medições e rateios

`negotiated_unit_price` recebe custo direto, BDI e desconto separados. Aplica uma vez a fórmula `custo × (1 + BDI/100) × (1 - desconto/100)` e arredonda o preço final para seis casas, com empate afastando de zero. BDI admite de 0% a 1.000%; desconto admite de 0% a 100%. Esse teto de BDI é uma decisão do produto, sem significado de limite legal.

Custo de R$ 100, BDI de 20% e desconto de 10% produzem `108.000000`. Desconto de 100% produz preço zero identificado. O consumidor precisa classificar a origem do custo: preço de venda já negociado não deve receber uma segunda incidência de BDI.

### Medição por acumulados

Para cada segmento econômico com preço fixado:

```text
acumulado monetário = truncar2(quantidade acumulada × preço do segmento)
valor do período = acumulado monetário atual - valor anterior reconhecido
```

`unit_measurement` fornece a prévia com quantidade contratada, acumulados anterior/atual e preço. `unit_measurement_with_previous` recebe também o valor anterior reconhecido e recusa divergência entre esse valor, a quantidade anterior e o preço do segmento. O consumidor definitivo deve obter esse histórico da versão protegida na mesma transação que consolidará a operação.

| Entrada | Resultado |
|---|---|
| PU `0.335`, acumulados 1, 2 e 3 | Parcelas `0.33`, `0.34`, `0.33`; total `1.00` |
| PU `10`, anterior 20, atual 35 | Quantidade do período 15; valor `150.00` |
| Contratado 100, proposto 101 | `InsufficientBalance` |
| Anterior 20, proposto 15 | `Decreasing`; exige o fluxo próprio de correção |
| Anterior 20, PU 10, valor anterior reconhecido `199.99` | `InconsistentPrevious` |

O total é calculado pela diferença dos valores acumulados. Truncar cada quantidade incremental isoladamente perderia centavos. `global_measurement` aplica a mesma diferença de acumulados sobre o valor da etapa e percentuais entre 0% e 100%. Uma etapa de `100000.00`, de 20% para 45%, gera `25000.00` no período; a 100%, o acumulado fecha o valor-base.

`adjustment` calcula o quociente dos índices com dez casas, empate afastando de zero, e trunca o reajuste monetário para centavos. Índices devem ser positivos. Um fator arredondado pode ser zero; por isso `Factor` é distinto de `IndexValue`. Seleção da competência, permissão contratual de variação negativa e tratamento de índice provisório ou ausente pertencem ao consumidor de reajuste.

### Maiores restos

`allocate_money` distribui os centavos de um total. `allocate_percentages` distribui `100.00` pontos percentuais para apresentação. A API de domínio `allocate` permite uma escala explícita, sem arredondar o total para fazê-lo caber.

Cada parcela recebe identificador, posição estável e peso não negativo. O domínio admite de uma a 10.000 parcelas, exige soma de pesos positiva e recusa identificadores duplicados. Depois das quotas inteiras, os resíduos são destinados aos maiores restos; o desempate usa posição crescente e depois identificador. A ordem incidental de uma consulta não define quem recebe o centavo.

R$ 100,00 em três pesos iguais produz R$ 33,34, R$ 33,33 e R$ 33,33. Os percentuais exibidos ficam em 33,34%, 33,33% e 33,33%. O resultado identifica a parcela que recebeu cada residual. Percentuais são uma reconciliação informativa, sem criar outra cobrança a somar às parcelas.

## 4. Quantidade e unidade

`QuantityWithUnit` associa `Quantity` a `MeasureUnit`. As unidades canônicas disponíveis são `un`, `m`, `m²`, `m³`, `kg`, `t`, `L` e `h`.

Somar ou subtrair exige a mesma unidade. `m` e `m²` não são compatíveis; `kg` e `t` também não são convertidos automaticamente. Uma conversão futura precisa de regra explícita, origem e resultado verificáveis. A subtração que produzir quantidade negativa é recusada. A desserialização exige quantidade textual e unidade reconhecida.

O formulário pode oferecer aliases de apresentação, como `m2`, e convertê-los explicitamente para `m²`. O contrato canônico do domínio continua exigindo o símbolo registrado. O cálculo de uma medição deve receber quantidades da mesma unidade e do mesmo segmento econômico.

## 5. Calendários, execução e paralisação

Os cálculos usam `CivilDate`, entre os anos 0001 e 9999, sem horário nem fuso. Uma data de contrato não deve ser convertida em um instante à meia-noite.

| Cadência | `DIAS_FIXOS_SICOP` | `CALENDARIO_CIVIL` |
|---|---:|---|
| `SEMANAL` | 7 dias | 7 dias |
| `QUINZENAL` | 15 dias | 15 dias |
| `MENSAL` | 30 dias | 1 mês desde a âncora original |
| `BIMESTRAL` | 60 dias | 2 meses desde a âncora original |
| `TRIMESTRAL` | 90 dias | 3 meses desde a âncora original |
| `SEMESTRAL` | 180 dias | 6 meses desde a âncora original |
| `ANUAL` | 365 dias | 12 meses desde a âncora original |

`advance` e `civil_months_after` calculam sempre a partir da âncora original. De 31/01/2027, um e dois meses produzem 28/02 e 31/03. Em 2028, o primeiro término é 29/02. O ajuste de fevereiro não transforma o dia 28 ou 29 em uma nova âncora para março.

`schedule` produz períodos contíguos e encerra exatamente no término formal informado. Se a próxima cadência ultrapassar esse término, marca o último período como residual. Assim, o intervalo de 31/01/2027 a 28/02/2027 conserva 28 dias mesmo quando a cadência selecionada é fixa de 30 dias.

`execution_end` conta dias corridos a partir do dia seguinte à OS: início em 01/03 mais dez dias produz término em 11/03. `execution_window` também confere se início e término estão dentro da vigência. Quando a execução excede a vigência, a data calculada continua disponível e `within_validity` fica falso. O consumidor deve bloquear a consolidação até regularização do instrumento.

### Pausas

`PauseInterval` representa `[início, retomada)`. Parada em 10/03 e retomada em 13/03 soma três dias. Intervalos adjacentes podem coexistir; sobreposição na mesma frente é recusada. Um intervalo vazio soma zero dias. A validação examina o conjunto inteiro antes de apurar o total.

`pause_days` exige a frente a apurar. Pausas de frentes diferentes não são somadas como se todas prorrogassem a obra inteira. Pausa aberta exige `as_of`, igual ou posterior ao início, e produz uma projeção identificada.

`project_execution` conserva `formal_end` e `valid_to`, calcula `projected_end` e informa se a projeção cabe na vigência. Enquanto houver pausa aberta na frente escolhida, `provisional` é verdadeiro. Nenhuma dessas funções altera o instrumento. O consumidor precisa selecionar somente os intervalos ainda não aplicados ao prazo daquela revisão e registrar um ajuste aprovado uma única vez. Autorização, idempotência, impedimento de execução na frente parada e efeitos sobre cronograma pertencem aos fluxos de [paralisação](issues/030.md), [aditivos](issues/040.md) e [reprogramação](issues/042.md).

## 6. Instantes e fusos regionais

`UtcInstant` representa o instante auditável em UTC, com precisão de até microssegundos. `TimeZoneId` valida o nome IANA, como `America/Sao_Paulo`. As regras regionais são fornecidas pela dependência fixada no projeto; uma atualização da base de fusos exige revisão dos casos afetados.

`local_to_utc` recebe `YYYY-MM-DDTHH:MM:SS[.ffffff]` e um fuso. Horário inexistente em uma mudança de horário produz `NonexistentLocalDateTime`; horário repetido produz `AmbiguousLocalDateTime`. O motor não escolhe silenciosamente um dos dois instantes. Quando há offset explícito e válido, o consumidor pode usar a entrada de `UtcInstant`.

`utc_to_local` apresenta o instante com offset. Alguns offsets históricos possuem segundos, que a representação RFC3339 não comporta. Nesses casos, a função retorna **`SubminuteOffset`**. O chamador pode manter a apresentação em UTC, acompanhada do fuso, em vez de exibir um offset arredondado que apontaria para outro instante. A conversão histórica de hora local para UTC continua exata quando a data e o horário são válidos.

## 7. Contratos HTTP das prévias

Os dois endpoints recebem JSON, têm limite de corpo de 4.096 bytes e exigem `policy: "C360-DECIMAL-1"`. Campos desconhecidos, versão ausente ou incompatível e entradas inválidas são recusados com HTTP 422 e erro legível. A versão não é inferida a partir da ausência do campo. Não existe campo para escolher arredondamento por lançamento.

### Rateio

```http
POST /api/prototipo/calculos/rateio
Content-Type: application/json

{"policy":"C360-DECIMAL-1","total":"100.00","shares":3}
```

Resposta de exemplo:

```json
{
  "policy": "C360-DECIMAL-1",
  "total": "100.00",
  "shares": [
    {"position":1,"amount":"33.34","percent":"33.34","received_cent":true,"received_percent_residual":true},
    {"position":2,"amount":"33.33","percent":"33.33","received_cent":false,"received_percent_residual":false},
    {"position":3,"amount":"33.33","percent":"33.33","received_cent":false,"received_percent_residual":false}
  ]
}
```

Essa prévia usa pesos iguais e aceita de uma a 100 parcelas. `total` precisa ser não negativo e respeitar `Money`. A distribuição por pesos próprios é uma capacidade do domínio, ainda sem campo neste endpoint.

### Calendário

```http
POST /api/prototipo/calculos/calendario
Content-Type: application/json

{
  "policy":"C360-DECIMAL-1",
  "calendar_policy":"CALENDARIO_CIVIL",
  "cadence":"MENSAL",
  "anchor":"2027-01-31",
  "formal_end":"2027-03-31"
}
```

A resposta conserva as políticas, a âncora e o término, informa `duration_days: 59` e retorna dois períodos: 28 e 31 dias, terminando em 28/02 e 31/03. Cada período contém `start`, `end`, `days` e `residual`. O intervalo máximo da prévia é 36.600 dias; início posterior ao término é recusado.

## 8. Uso seguro pelos próximos módulos

Entradas decimais tipadas e unidades possuem desserialização validada. Os resultados derivados do domínio, como `MeasurementDelta`, `GlobalMeasurement`, `Adjustment`, `Allocation` e as projeções de calendário, oferecem serialização de saída, sem `Deserialize`. Os contratos de apresentação que precisam receber uma resposta HTTP usam seus próprios tipos.

Um consumidor definitivo deve conservar a política, os dados de entrada, a versão protegida e a origem econômica. Precisa recalcular no servidor, conferir autorização, organização, saldo e estado e gravar o resultado com a decisão correspondente. O resultado enviado pelo navegador não é prova de aprovação. Reexecução de uma função pura produz o mesmo valor, mas não garante que um comando persistente consuma saldo uma única vez.

A divisão entre domínio, aplicação e persistência segue a [arquitetura](arquitetura-rust.md). Os exemplos e as prévias desta etapa não declaram concluídos os fluxos financeiros ou a prontidão comercial.

## 9. Verificação e reprodução

Na raiz de um checkout com a toolchain fixada, os testes numéricos e de calendário podem ser executados sem banco:

```sh
cargo test --locked -p canteiro-domain
cargo test --locked -p canteiro-contracts
cargo test --locked -p canteiro-api --test calculations
cargo test --locked -p canteiro-calculation-verification --test reference
cargo clippy --locked -p canteiro-domain --all-targets -- -D warnings
```

Os resultados esperados estão no arquivo versionado [reference-vectors.json](validacao/issue-005/reference-vectors.json). Sua referência independente usa aritmética decimal, frações exatas e calendário externos ao produto. O teste `reference` compara os resultados calculados com os valores esperados e verifica que esses valores esperados não podem fornecer a resposta ao adaptador.

O executável abaixo produz as respostas nativas para inspeção. A comparação com os resultados esperados é feita pelo teste, não pelo simples fato de o executável imprimir JSON:

```sh
cargo run --locked -p canteiro-calculation-verification -- docs/validacao/issue-005/reference-vectors.json
```

Para executar o mesmo domínio em WASM real, prepare as ferramentas e os artefatos conforme o [guia de desenvolvimento](desenvolvimento.md). Depois de `cargo xtask build`, gere o módulo de verificação separado:

```sh
cargo build --locked -p canteiro-calculation-verification --lib --features wasm --target wasm32-unknown-unknown --profile wasm-release
wasm-bindgen --target web --no-typescript --out-name calculation-verification --out-dir target/site/pkg/verification target/wasm32-unknown-unknown/wasm-release/canteiro_calculation_verification.wasm
cargo test --locked -p canteiro-e2e --test calculations -- --ignored --nocapture
```

O ensaio de navegador exige o servidor compilado, o banco de desenvolvimento migrado, a conexão de execução e Chrome/ChromeDriver configurados como no guia. O adaptador de verificação não é ligado à aplicação de produção. No navegador, somente as entradas são entregues ao WASM; os resultados esperados permanecem no teste externo. A comparação confere referência, Rust nativo e WASM executado, além da representação canônica. Compilar para WASM, isoladamente, não equivale a executar essa comparação.

As propriedades verificadas incluem conservação de centavos e duração, telescopia de acumulados, independência da ordem de consulta e monotonicidade do acumulado para preço fixo não negativo. Não se presume monotonicidade individual de cada parcela quando o total de um rateio por maiores restos muda.

Este guia descreve o procedimento e o alcance da implementação. O resultado de uma execução precisa estar associado à revisão efetivamente testada; não constitui, por si, declaração de aprovação da CI, do navegador ou da integração das issues anteriores.

## 10. Cobertura dos vetores originais e consumidores futuros

Os 30 [vetores originais de especificação](validacao/vetores-calculo.json) permanecem preservados. A fundação oferece seus componentes numéricos e temporais; os casos que dependem de estado exigem os módulos correspondentes. A [issue 70](issues/070.md) deve reconciliar essas partes em jornadas completas, inclusive concorrência, correção, documentos e banco.

| Vetores originais | Alcance da fundação e conclusão exigida do consumidor |
|---|---|
| V01, V02, V03, V04, V05, V06, V10, V13, V14, V15, V16, V27, V28 | Operações numéricas, rateio, cadências e duração cobertos pelos tipos, funções e casos desta etapa. Origem dos valores, aprovação e persistência seguem o módulo consumidor |
| V11 | A prévia recusa quantidade acima do contratado. O contrato vigente e alterações formais precisam ser conferidos nas issues [24](issues/024.md), [32](issues/032.md) e [40](issues/040.md) |
| V12, V22 | Seleção de competência, índice ausente, uso provisório e ciclo fixo pertencem a [reajustes](issues/035.md) e ao [cálculo definitivo](issues/036.md) |
| V07, V08, V21, V23 | Supressão, rejeição, reabertura da cadeia e reversão do ciclo original exigem as issues [32](issues/032.md), [36](issues/036.md), [41](issues/041.md) e a conciliação dos efeitos financeiros |
| V09, V20, V26 | Retenções, adiantamentos, liquidação, crédito remanescente e encerramento técnico exigem as issues [44](issues/044.md), [45](issues/045.md), [46](issues/046.md) e [48](issues/048.md) a [50](issues/050.md) |
| V17, V25 | Modelo do acordo e seu limite, concorrência, reserva, confirmação e cancelamento parcial exigem as issues [9](issues/009.md), [51](issues/051.md), [53](issues/053.md) e [54](issues/054.md) |
| V18 | Decisão de alçada para acréscimo ou alteração mista pertence às issues [38](issues/038.md), [39](issues/039.md) e [40](issues/040.md) |
| V19 | CNPJ textual e validação por vetores oficiais pertencem aos [cadastros](issues/011.md) |
| V24 | Incidência percentual exige a seleção versionada de títulos-base, exclusões e ausência de ciclos em [composições](issues/017.md) e [medição global e percentual](issues/033.md) |
| V29 | `execution_window` identifica a execução fora da vigência. O bloqueio definitivo e o ajuste formal são aplicados por [contratos](issues/023.md), [paralisação](issues/030.md) e [aditivos](issues/040.md) |
| V30 | Mudança de denominador sem reescrever execução depende das issues [33](issues/033.md), [40](issues/040.md) e [42](issues/042.md) |

Essa separação preserva o aceite próprio da issue 5 e mantém os cenários compostos sob responsabilidade explícita da homologação integrada. Um teste de primitivo não substitui o aceite do fluxo completo.
