# ADR 009 — Adapters de plataforma de dispositivo

## Status

Aceito em 2026-08-30.

## Contexto

O sistema precisa identificar, acessar e configurar equipamentos com sistemas
diferentes. Antes desta decisão, havia uma lista central de sistemas, uma tabela
paralela de receitas Syslog, regras específicas no parser, palavras de
classificação no discovery, mapas fixos no frontend e um registro independente
de geradores WireGuard. Adicionar uma plataforma exigia encontrar e alterar
todos esses pontos; esquecer um deles produzia uma opção parcialmente funcional.

## Decisão

Adotar Adapter Pattern como fronteira de toda variação por plataforma:

- `DeviceAdapter` expõe metadados, evidências de identificação, meios de acesso,
  dica de classificação e vínculos com recursos especializados;
- `SyslogConfigurationAdapter` encapsula comandos, texto de orientação,
  leitura de identidade, comando de teste e enriquecimento do dialeto recebido;
- `VpnProfileGenerator` permanece como adapter especializado de artefatos
  WireGuard, ligado ao dispositivo pelo registro principal;
- `devices::adapters::registry` é o único ponto de composição e precedência;
- controllers, discovery, parser, provisionamento e frontend consomem contratos
  ou catálogos retornados pela API, nunca listas próprias de plataformas.

As capacidades observadas de um dispositivo (interfaces, métricas, logs e
eventos realmente coletados) continuam separadas do adapter. Plataforma diz o
que é suportado; `devices/capabilities.rs` diz o que já foi comprovado naquele
equipamento. Misturar as duas coisas faria uma flag de cadastro parecer uma
conexão bem-sucedida.

## Consequências

Para adicionar uma plataforma:

1. implementar `DeviceAdapter` e, se aplicável, os adapters especializados;
2. registrar a implementação em `devices::adapters::registry`;
3. implementar/registrar um `VpnProfileGenerator` quando houver artefato VPN;
4. adicionar testes do adapter e amostras reais dos protocolos envolvidos.

O frontend recebe rótulo, ícone e capacidades. Perfis desconhecidos mantêm um
fallback legível para dados históricos, mas não exigem uma nova constante para
aparecer nas telas.

Testes de convenção impedem a volta de catálogos paralelos nos pontos críticos.
Conhecimento específico de protocolos (por exemplo, o framing do MAC-Telnet ou
um script RouterOS) permanece em sua implementação concreta; não é duplicação
de catálogo, é o próprio adapter/protocolo.
