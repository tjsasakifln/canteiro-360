# Decisão 0001: fundação compilável

Data: 11/11/2026. Escopo: issue 1.

## Arquitetura e compilação

Mantidos Rust, Leptos com SSR/hidratação, Axum/Tokio e SQLx/PostgreSQL, conforme a especificação aprovada. O workspace separa domínio, aplicação, persistência, contratos, interface, servidor e processamento. Ferramentas próprias e ensaios de navegador são escritos em Rust.

A compilação usa Cargo e `wasm-bindgen-cli` diretamente, coordenados por `tools/xtask`. Essa escolha evita outro serviço de desenvolvimento e permite que o mesmo procedimento gere os dois alvos, arquivos comprimidos e medidas. O carregador de hidratação é produzido pelo componente oficial `HydrationScripts` do Leptos. Não há script de inicialização próprio.

O perfil do navegador usa otimização de tamanho, LTO e uma unidade de geração. O servidor conserva seu perfil próprio. As dependências e a toolchain são fixadas; não há resolução por `latest` em cada execução.

O orçamento usa o conjunto completo de arquivos da interface. Recurso remoto não inventariado, import de CSS externo, Brotli ausente ou diferente do arquivo original impede o aceite. O script inline entra pelo tamanho bruto como margem conservadora; sua compressão real ocorre junto com o HTML e não é atribuída artificialmente a um arquivo separado.

## Dependência transitiva sem manutenção

O [aviso RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436) classifica `paste 1.0.15` como **INFO Unmaintained**. A fonte não descreve nesse aviso uma vulnerabilidade explorável nem uma versão corrigida. O pacote é uma macro procedural usada na compilação por componentes de Leptos, incluindo `either_of`, `reactive_graph`, `reactive_stores` e `tachys`.

Decisão: aceitar somente esse aviso, com identificação explícita em `deny.toml` e versão registrada no lock. A manutenção deve ser reavaliada na próxima atualização de Leptos e antes da liberação comercial da issue 72. Uma nova vulnerabilidade ou outro aviso continua sujeito ao bloqueio normal.

A alternativa de substituir por `pastey` exige substituir coordenadamente os consumidores ou manter cópias modificadas das bibliotecas. Para a fundação, isso acrescentaria manutenção e risco de divergência sem eliminar um defeito demonstrado de execução. A revisão independente concordou com a exceção restrita. Ela não significa que todas as dependências são livres de risco futuro.

## Licenças

A verificação cobre o grafo de dependências, inclusive as do navegador. As licenças permitidas estão explícitas em `deny.toml`. `BSL-1.0` designa Boost Software License; não é Business Source License. `CC0-1.0` e `CDLA-Permissive-2.0` foram incluídas para os pacotes efetivamente resolvidos. Dependências externas de origem desconhecida e versões curinga externas são recusadas. A permissão de caminhos locais se limita aos crates do próprio workspace.

## Ambientes e limites desta entrega

As configurações de desenvolvimento e homologação contêm apenas valores públicos. O servidor não passa configuração de persistência aos tipos serializáveis ou à interface. Erros de configuração indicam o campo, sem repetir o valor recebido.

Não foram criados cadastros, sessões, migrações, filas, cálculos ou transações financeiras nesta etapa. Os crates correspondentes estabelecem as fronteiras da implementação, e a página comunica o estágio real do produto.

## Referência para ensaios físicos futuros

Selecionado o **Samsung Galaxy A55 5G, variante com 8 GB de RAM e 128 GB**, como Android físico de referência. A capacidade foi conferida na [especificação oficial da Samsung](https://news.samsung.com/br/samsung-galaxy-a55-5g-e-galaxy-a35-5g-inovacoes-incriveis-e-seguranca-projetada-para-todos). Antes do ensaio físico, registrar a unidade usada, processador, versão efetivamente instalada do sistema e navegador, energia e rede. A seleção não representa compra, posse ou ensaio já realizado.

O computador de referência para repetição automatizada é o executor Linux x86_64 identificado no registro da issue 1, com Chrome for Testing fixado. O protocolo completo de experiência continua exigindo o ensaio físico e o cenário de capacidade especificado antes da aceitação das jornadas integradas. Medidas desta fundação são identificadas como medidas do laboratório efetivamente disponível, sem equivaler a visitantes reais ou ao desempenho dos módulos ainda não construídos.
