# ADR 010 — Gerenciamento pela Docker Engine API

## Status

Aceito em 2026-08-30.

## Contexto

O NetMonitor precisa observar containers, volumes, redes e imagens do host em
que está instalado e oferecer operações de administração. Executar o CLI
`docker` criaria dependência de binário, parsing de texto, processos filhos e
uma superfície adicional de injeção. A Docker Engine já oferece um contrato HTTP
estruturado pelo socket local.

O socket não é uma permissão comum de arquivo: quem pode escrever nele consegue
criar containers privilegiados e controlar o host. Executar a aplicação sem
capabilities Linux não reduz essa autoridade.

## Decisão

- usar `bollard` como única fronteira com a Docker Engine;
- não instalar, executar nem interpretar saída do CLI `docker`;
- montar `/var/run/docker.sock` no compose e resolver seu GID no entrypoint,
  mantendo o processo da API como usuário `app` e sem `chmod 666`;
- permitir inventário, detalhes, logs e métricas a qualquer perfil autenticado;
- restringir mutações e exportação de volumes ao perfil `admin`;
- registrar mutações na trilha de auditoria;
- limpar logs reais somente para containers com driver `json-file`: a operação
  valida o `LogPath`, interrompe brevemente o alvo, trunca o arquivo por um
  helper efêmero sem rede/capabilities e restaura o estado anterior; containers
  `auto-remove`, pausados ou o próprio NetMonitor são recusados;
- ocultar no backend valores de ambiente com nomes associados a senhas,
  tokens, segredos, chaves privadas e credenciais;
- usar timeouts em chamadas da Engine, limite de linhas de log, coleta de
  métricas concorrente limitada e cache de 10 segundos;
- degradar listagens e métricas explicitamente quando a Engine estiver ausente,
  sem comprometer os demais módulos;
- permitir desligamento por `DOCKER_ENABLED=false`; isolamento efetivo exige
  também remover a montagem do socket.

## Consequências

A instalação padrão ganha administração do host Docker, mas o container do
NetMonitor passa a ser parte da fronteira de confiança da Engine. Uma falha que
contorne autenticação/autorização pode ter impacto no host; atualizações e
revisões de segurança do backend tornam-se obrigatórias.

Exportar um volume cria temporariamente um container Alpine somente-leitura e
transmite o arquivo compactado sem carregá-lo inteiro em memória. Se a imagem
não existir, a Engine faz pull; portanto a primeira exportação pode depender de
acesso ao registry. O container auxiliar é removido ao encerrar ou interromper o
stream.

A Engine não oferece endpoint para apagar logs. A limpeza de `json-file` é,
portanto, uma operação Linux específica e deliberadamente restrita. Drivers
como `journald`, `local`, `syslog` e backends remotos devem ser limpos pela
ferramenta responsável por seu armazenamento.
