# LAN Skin Share 0.1.0 — Horizon 26.3 (protótipo)

**Objetivo:** compartilhar a skin atualmente selecionada por cada jogador entre clientes Horizon que participam de um mundo aberto para LAN. O projeto foi pensado para funcionar com perfis offline e contas Microsoft originais, sem usar nickname para localizar ou associar skins.

## Como deve funcionar

1. Cada cliente lê a textura que o próprio Horizon já carregou para o personagem local (skin offline selecionada no launcher ou skin ativa do perfil Microsoft).
2. Opcionalmente, `config/lan_skin_share/skin.png` substitui essa textura.
3. O cliente envia a textura ao servidor LAN pelo canal de mods `horizon:mod_net`.
4. O servidor integrado do anfitrião mantém as texturas em memória e as distribui aos outros clientes Horizon que tenham o mod.
5. Cada cliente associa a textura ao UUID de sessão fornecido pela conexão e aplica a skin à entidade correspondente. O UUID não é usado para procurar perfil, nome ou skin externa.

## Configuração normal

Não é necessário criar arquivos nem configurar nickname. Cada jogador escolhe a sua skin no próprio Horizon/launcher. O mod tenta compartilhar a textura que o Horizon já carregou para o personagem local.

## Substituição manual opcional

Para forçar uma PNG diferente da skin ativa no Horizon, coloque os arquivos ao lado de `options.txt`, dentro de `config/`:

```text
config/
└── lan_skin_share/
    ├── skin.png
    └── model.txt
```

- `skin.png`: PNG 64×64.
- `model.txt`: `slim` para Alex ou `classic` para Steve. Se não existir, assume `classic`.
- A PNG precisa ter até 24.000 bytes para caber com folga no limite do canal nativo.

Se a PNG não existir ou não for válida, o mod tenta compartilhar a skin ativa do Horizon.

## Requisitos

- Horizon Minecraft 26.3, Nightly de 2026-11-13, commit `9f42064c11efe7ef3e5ed2baad7ddc2a3b36eea0`.
- Todos os participantes, inclusive o anfitrião, precisam instalar o mesmo mod e usar builds compatíveis.
- Clientes vanilla não recebem mensagens `horizon:mod_net`; portanto, este mod não compartilha skins com clientes sem Horizon/mod.
- Apenas PNG 64×64 nesta versão. Texturas antigas 64×32 ainda não são convertidas.

## Privacidade e segurança

- Não usa nickname como chave nem consulta serviços de skins.
- Não exige autenticação Microsoft por parte do mod.
- Para contas originais, compartilha a textura que o próprio Horizon já carregou; o mod não faz uma consulta de conta separada.
- Não usa servidor externo: os bytes trafegam pelo canal nativo da sessão LAN e ficam em memória.
- O UUID de conexão só serve para associar a textura à entidade correta enquanto ela participa da sessão.

## Estado e validação

Este pacote é um **protótipo-fonte** fixado à revisão acima. Foram conferidos estaticamente os manifests, o TOML, os caminhos dos helpers, as assinaturas dos hooks e o formato do protocolo. **Não foi compilado com Cargo/horizon-mixins nem testado em uma sessão LAN real.** Este ambiente não contém `cargo`, `rustc` nem o checkout do Horizon. Portanto, não deve ser tratado como versão estável ou como funcionalidade garantida.

O próximo passo é compilar pelo launcher Horizon e corrigir qualquer incompatibilidade indicada no log. Depois disso, ainda será necessário testar com anfitrião + convidado(s), incluindo um perfil offline e uma conta Microsoft original.
