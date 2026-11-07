# Correção 2026-11-07-01 — Endereço de rede e outras sub-redes como host na descoberta

- **Data:** 2026-11-07
- **Severidade:** média (a descoberta ofereceu para cadastro um "aparelho" que não existe)
- **Arquivos:** `backend/crates/core/src/shared/cidr.rs`,
  `backend/crates/network/src/discovery/pipeline.rs`

## O relato

A varredura de `10.0.0.0/24` mostrou um cartão **"IoT / automação" em `10.0.0.0`**, MAC
`00:12:42:51:2b:b0`, fabricante *Millennial Net*, confiança 56 %, motivo *Fabricante
menciona "millennial"*, com o botão **Adicionar**.

## Diagnóstico

São duas perguntas, com respostas diferentes.

### Por que `10.0.0.0` virou host — o defeito

`10.0.0.0` é o **endereço de rede** da faixa: nenhum aparelho mora nele. A expansão da faixa
(`expand_cidr_batch`) já o exclui, e por isso nem o ping nem o ARP local o tocam. Mas a
descoberta tem fontes que **listam vizinhos por conta própria** e entravam no resultado sem
passar pelo mesmo critério:

| Fonte | Filtro antes |
|---|---|
| Ping (ICMP) e ARP local | só os endereços expandidos — correto |
| mDNS e SSDP | `is_ip_in_cidr`, que **aceita** o endereço de rede e o de broadcast |
| Tabela ARP do roteador, lida por SNMP (`scanner: "snmp_arp"`) | **nenhum** |

A identificação consulta o SNMP de cada host vivo; quando o host é um roteador, a resposta
traz a tabela ARP inteira dele — outras sub-redes, e às vezes uma entrada para o próprio
endereço de rede (resto de um aparelho mal configurado ou de uma varredura que o roteador
resolveu). Tudo isso era fundido ao resultado como se fosse desta varredura. O resultado
gravado mostra a origem em `details.scanner = "snmp_arp"` e `details.sourceRouter`.

### Por que "IoT" — não é erro de identificação nem da IA

- O tipo saiu da **heurística determinística** (`discovery/fingerprints.rs`), não do Decisor:
  o OUI `00:12:42` é registrado no IEEE para a **Millennial Net**, fabricante de rádios de
  redes de sensores sem fio (MeshScape) — IoT é a leitura correta desse fabricante. A regra
  `prefix("millennial", IOT, 40)` e o teste `fabricantes_comuns_de_iot_sao_classificados_corretamente`
  existem de propósito.
- A IA (`discovery/decider_identity.rs`) só dá palpite quando a heurística termina em
  `unknown`/`web_device` ou sem sistema identificado. Aqui o tipo já tinha saído como `iot`,
  então o Decisor nem foi consultado. **Não há parâmetro de IA a criar** para este caso:
  o que estava errado era o endereço entrar na lista, e com a correção ele não chega à IA.

## O que foi feito

- **Um critério só para "host desta varredura":** `netmonitor_core::shared::cidr::is_scan_host`
  — dentro da faixa e, em IPv4 abaixo de /31, nem rede nem broadcast (RFC 3021 nas /31 e
  /32); IPv6 por pertença ao prefixo. Ele divide com `expand_cidr_batch` a mesma função de
  limites (`ipv4_host_bounds`), então a expansão e o filtro não divergem.
- **Toda fonte passa por ele** (`pipeline::in_scope`): mDNS/SSDP (no lugar do
  `is_ip_in_cidr`) e o resultado da identificação, onde chega a ARP do roteador.

## Validação

- `cidr::tests`: rede, broadcast e outra sub-rede ficam de fora; /31, /32 e IPv6; e o
  critério aceita exatamente os endereços que a expansão gera.
- `pipeline::tests::arp_do_roteador_fora_da_faixa_ou_no_endereco_de_rede_fica_de_fora`.

## O que fica para o usuário

Resultados de varreduras **anteriores** continuam gravados com o `10.0.0.0`; basta varrer de
novo. Se o aparelho chegou a ser cadastrado, exclua-o na lista de dispositivos.
