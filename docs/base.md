# Sistema de Monitoramento de Redes

## 1. Visão geral

O projeto consiste em uma plataforma simples de monitoramento de redes residenciais e de pequenas empresas.

O sistema permitirá descobrir equipamentos conectados à rede, monitorar disponibilidade, acompanhar tráfego, identificar relações entre dispositivos e visualizar a topologia da rede em formato gráfico.

A proposta é oferecer uma experiência mais simples que soluções corporativas tradicionais, mantendo recursos importantes como:

* Monitoramento por ping, HTTP, HTTPS, TCP e DNS.
* Descoberta automática de equipamentos.
* Identificação por IP, hostname, mDNS, MAC e SNMP.
* Monitoramento de tráfego por interface.
* Construção de mapas de rede.
* Traceroute visual.
* Alertas de indisponibilidade.
* Histórico de métricas e eventos.

## 2. Público-alvo

O sistema será direcionado principalmente para:

* Residências com vários dispositivos conectados.
* Pequenos escritórios.
* Pequenas empresas.
* Prestadores de suporte técnico.
* Administradores de redes locais.
* Empresas com filiais pequenas.
* Ambientes com roteadores, switches, access points, câmeras e servidores.

## 3. Objetivos

### 3.1 Objetivo principal

Centralizar o inventário, monitoramento e visualização de uma rede local em uma interface simples e intuitiva.

### 3.2 Objetivos específicos

* Detectar equipamentos conectados à rede.
* Informar quais equipamentos estão online ou offline.
* Monitorar latência e perda de conectividade.
* Acompanhar serviços HTTP, HTTPS e TCP.
* Organizar dispositivos por rede e localização.
* Permitir relacionar equipamentos entre si.
* Exibir a topologia da rede graficamente.
* Consultar informações de equipamentos via SNMP.
* Monitorar tráfego de entrada e saída.
* Registrar histórico de disponibilidade e métricas.
* Gerar alertas quando houver falhas.

## 4. Conceitos principais

### 4.1 Site

Representa um local físico ou ambiente monitorado.

Exemplos:

* Residência.
* Escritório principal.
* Loja.
* Filial.
* Data center pequeno.

Um site poderá possuir uma ou mais redes.

### 4.2 Rede

Representa uma sub-rede monitorada.

Exemplos:

* `10.0.0.0/24`
* `192.168.1.0/24`
* `192.168.10.0/24`
* Rede administrativa.
* Rede de câmeras.
* Rede Wi-Fi para clientes.

### 4.3 Probe

Aplicação responsável por executar verificações dentro da rede monitorada.

O probe poderá executar:

* Ping.
* Requisições HTTP e HTTPS.
* Testes TCP.
* Resolução DNS.
* Scan de IPs.
* Descoberta mDNS.
* Descoberta SSDP.
* Leitura de ARP e NDP.
* Consultas SNMP.
* Traceroute.

O sistema poderá operar em três modos:

* Servidor e probe na mesma instalação.
* Servidor central com probes remotos.
* Instalação standalone.

### 4.4 Dispositivo

Representa um equipamento físico ou virtual identificado na rede.

Exemplos:

* Roteador.
* Switch.
* Access point.
* Computador.
* Servidor.
* Impressora.
* Câmera.
* Televisão.
* Smartphone.
* Dispositivo IoT.

Um dispositivo poderá possuir:

* Um ou mais endereços IP.
* Um ou mais endereços MAC.
* Várias interfaces de rede.
* Vários serviços.
* Vários monitores.
* Relações com outros dispositivos.

### 4.5 Interface

Representa uma interface de rede pertencente a um dispositivo.

Exemplos:

* Porta Ethernet.
* Interface WAN.
* Interface LAN.
* Interface Wi-Fi.
* VLAN.
* Interface virtual.
* Túnel.

Uma interface poderá armazenar:

* Nome.
* Descrição.
* Índice SNMP.
* Endereço MAC.
* Velocidade.
* Estado administrativo.
* Estado operacional.
* Tráfego de entrada.
* Tráfego de saída.
* Pacotes.
* Erros.
* Descartes.

### 4.6 Ligação

Representa uma relação entre dois dispositivos ou interfaces.

Exemplos:

* Roteador conectado ao switch.
* Switch conectado ao access point.
* Access point associado a um cliente Wi-Fi.
* Equipamento localizado atrás de outro roteador.

A ligação poderá ter diferentes origens:

* Manual.
* Descoberta por LLDP.
* Descoberta por CDP.
* Inferida por SNMP.
* Inferida por tabela MAC.
* Identificada por traceroute.

### 4.7 Monitor

Representa uma verificação executada periodicamente.

Um dispositivo poderá possuir vários monitores.

Exemplo:

```text
Roteador principal
├── Ping
├── HTTPS
├── Porta TCP 8291
├── SNMP
└── Traceroute
```

## 5. Funcionalidades

## 5.1 Gestão de sites

O sistema permitirá:

* Criar sites.
* Editar sites.
* Desativar sites.
* Definir nome e descrição.
* Informar localização.
* Associar redes.
* Associar probes.
* Visualizar o estado geral do site.

## 5.2 Gestão de redes

Cada rede poderá conter:

* Nome.
* Faixa CIDR.
* Gateway.
* VLAN.
* DNS.
* Probe responsável.
* Configurações de scan.
* Intervalo de descoberta.
* Lista de dispositivos.

Exemplo:

```text
Nome: Rede principal
CIDR: 192.168.1.0/24
Gateway: 192.168.1.1
VLAN: 1
```

## 5.3 Descoberta automática

O sistema poderá realizar scans manuais ou programados.

A descoberta poderá utilizar:

* Ping ICMP.
* Tabela ARP.
* NDP para IPv6.
* DNS reverso.
* mDNS.
* SSDP.
* Portas conhecidas.
* SNMP.
* Fabricante pelo prefixo MAC.

Para cada equipamento descoberto, o sistema poderá apresentar:

* Endereço IP.
* Endereço MAC.
* Hostname.
* Nome mDNS.
* Fabricante.
* Serviços encontrados.
* Possível tipo de equipamento.
* Data da primeira descoberta.
* Data da última atividade.
* Nível de confiança da identificação.

O usuário poderá:

* Adicionar o dispositivo ao inventário.
* Definir seu tipo.
* Informar atrás de qual roteador ou switch ele está.
* Ativar monitores automaticamente.

## 5.4 Inventário de dispositivos

O cadastro de dispositivo poderá conter:

* Nome amigável.
* Tipo.
* Fabricante.
* Modelo.
* Número de série.
* Sistema operacional.
* Descrição.
* Endereços IP.
* Endereços MAC.
* Interfaces.
* Credenciais associadas.
* Localização.
* Tags.
* Observações.
* Data da última atividade.

Tipos iniciais:

* Roteador.
* Switch.
* Access point.
* Servidor.
* Computador.
* Smartphone.
* Impressora.
* Câmera.
* Televisão.
* IoT.
* Equipamento desconhecido.

## 5.5 Monitoramento por ping

O monitor de ping permitirá:

* Definir endereço ou hostname.
* Definir intervalo.
* Definir timeout.
* Definir quantidade de tentativas.
* Medir latência.
* Detectar indisponibilidade.
* Registrar perda de resposta.
* Gerar alertas.

Métricas:

* Estado online ou offline.
* Latência atual.
* Latência mínima.
* Latência média.
* Latência máxima.
* Percentual de disponibilidade.

## 5.6 Monitoramento HTTP e HTTPS

O monitor HTTP permitirá:

* Definir URL.
* Definir método HTTP.
* Definir headers.
* Definir timeout.
* Validar código de resposta.
* Validar conteúdo.
* Ignorar ou validar certificado.
* Monitorar expiração do certificado.
* Medir tempo de resposta.
* Seguir redirecionamentos.

Resultados armazenados:

* Código HTTP.
* Tempo de resposta.
* Estado.
* Mensagem de erro.
* Informações do certificado.
* Data da verificação.

## 5.7 Monitoramento TCP

O monitor TCP permitirá testar se uma porta está acessível.

Exemplos:

* SSH.
* RDP.
* Banco de dados.
* Painel administrativo.
* Serviço personalizado.

Configurações:

* Host.
* Porta.
* Timeout.
* Intervalo.
* Tentativas.

## 5.8 Monitoramento DNS

O monitor DNS permitirá:

* Resolver registros.
* Validar respostas.
* Definir servidor DNS.
* Monitorar tipos específicos de registro.
* Medir tempo de resposta.

Tipos iniciais:

* A.
* AAAA.
* CNAME.
* MX.
* TXT.
* NS.

## 5.9 SNMP

O SNMP será utilizado para inventário e coleta de métricas.

Versões previstas:

* SNMPv1.
* SNMPv2c.
* SNMPv3.

Para SNMPv3, o sistema deverá suportar:

* `noAuthNoPriv`.
* `authNoPriv`.
* `authPriv`.

Informações básicas:

* `sysName`.
* `sysDescr`.
* `sysObjectID`.
* `sysUpTime`.
* Localização.
* Contato.
* Fabricante.
* Modelo.

Informações de interfaces:

* `ifIndex`.
* `ifName`.
* `ifDescr`.
* `ifAlias`.
* `ifType`.
* `ifAdminStatus`.
* `ifOperStatus`.
* `ifSpeed`.
* `ifHighSpeed`.
* `ifHCInOctets`.
* `ifHCOutOctets`.
* Erros.
* Descartes.

O sistema deverá utilizar `getBulk` quando disponível para reduzir a quantidade de consultas.

## 5.10 Monitoramento de tráfego

O tráfego será calculado com base na diferença entre contadores SNMP.

Métricas:

* Download em bits por segundo.
* Upload em bits por segundo.
* Total recebido.
* Total enviado.
* Utilização percentual.
* Erros por interface.
* Descartes por interface.
* Estado da interface.

Períodos de visualização:

* Última hora.
* Últimas 6 horas.
* Últimas 24 horas.
* Últimos 7 dias.
* Últimos 30 dias.
* Intervalo personalizado.

## 5.11 Topologia de rede

O sistema exibirá os dispositivos em um gráfico interativo.

Exemplo:

```text
Internet
   │
Roteador principal
   ├── Switch
   │   ├── Servidor
   │   ├── Impressora
   │   └── Access point
   │       ├── Notebook
   │       └── Smartphone
   └── Câmera
```

O usuário poderá:

* Arrastar dispositivos.
* Aproximar ou afastar o gráfico.
* Reorganizar os elementos.
* Criar ligações manualmente.
* Remover ligações.
* Visualizar estado online ou offline.
* Visualizar tráfego nas ligações.
* Abrir os detalhes de um dispositivo.
* Salvar posições do gráfico.

Tipos de ligação visual:

* Ligação manual confirmada.
* Ligação descoberta.
* Ligação inferida.
* Caminho de traceroute.

## 5.12 Relação “atrás de”

Ao adicionar um dispositivo, o usuário poderá informar que ele está atrás de outro equipamento.

Exemplo:

```text
Dispositivo: Computador administrativo
Atrás de: Roteador secundário
```

Essa informação poderá ser utilizada na construção inicial da topologia, mesmo quando não houver LLDP ou SNMP disponível.

## 5.13 LLDP e CDP

Quando suportado pelo equipamento, o sistema poderá descobrir vizinhos usando:

* LLDP.
* CDP.
* LLDP-MED.

Informações possíveis:

* Equipamento vizinho.
* Porta local.
* Porta remota.
* Nome do sistema.
* Endereço de gerenciamento.
* Tipo de equipamento.

## 5.14 Traceroute

O sistema permitirá executar traceroute a partir de um probe.

O resultado poderá ser exibido como:

* Lista de saltos.
* Latência por salto.
* Endereço IP.
* Hostname.
* Perda de resposta.
* Gráfico de rota.

O caminho de traceroute será tratado como rota observada, não como ligação física confirmada.

## 5.15 Alertas

O sistema permitirá criar alertas para:

* Dispositivo offline.
* Serviço indisponível.
* Latência alta.
* Perda de pacotes.
* Interface inativa.
* Alto uso de banda.
* Erros na interface.
* Certificado próximo da expiração.
* Probe desconectado.
* Falha em uma descoberta.
* Equipamento novo na rede.

Os alertas poderão possuir:

* Severidade.
* Tempo mínimo para disparo.
* Quantidade de falhas consecutivas.
* Horário de silêncio.
* Canais de notificação.
* Regras de recuperação.

## 5.16 Notificações

Canais iniciais:

* E-mail.
* Telegram.
* Discord.
* Webhook.
* Notificação no navegador.

Canais futuros:

* WhatsApp.
* Microsoft Teams.
* Slack.
* Gotify.
* Pushover.

## 5.17 Eventos

O sistema registrará eventos importantes:

* Dispositivo ficou offline.
* Dispositivo voltou a responder.
* Novo equipamento foi descoberto.
* Endereço IP mudou.
* Interface foi desativada.
* Tráfego ultrapassou o limite.
* Credencial SNMP falhou.
* Probe se desconectou.
* Configuração foi alterada.

## 5.18 Dashboard

O dashboard principal poderá apresentar:

* Total de dispositivos.
* Dispositivos online.
* Dispositivos offline.
* Serviços indisponíveis.
* Alertas ativos.
* Probes conectados.
* Equipamentos recém-descobertos.
* Latência média.
* Tráfego atual.
* Sites com problema.
* Últimos eventos.

## 6. Interface frontend

## 6.1 Tecnologias

O frontend será desenvolvido com:

* Vue 3.
* TypeScript.
* Vite.
* Vuetify.
* Vue Router.
* Pinia.
* PWA.
* SSE para atualizações em tempo real.

## 6.2 Diretrizes da interface

A interface deverá ser:

* Simples.
* Responsiva.
* Compatível com desktop, tablet e celular.
* Adequada para uso como PWA.
* Rápida em redes locais.
* Organizada por contexto.
* Fácil para usuários não especialistas.

## 6.3 Estrutura principal

```text
Aplicação
├── Dashboard
├── Sites
├── Redes
├── Dispositivos
├── Topologia
├── Descoberta
├── Monitores
├── Alertas
├── Eventos
├── Probes
└── Configurações
```

## 6.4 Páginas

### Dashboard

Resumo geral do ambiente monitorado.

### Sites

Listagem e gerenciamento dos locais.

### Redes

Configuração das sub-redes e scans.

### Dispositivos

Inventário completo de equipamentos.

### Detalhes do dispositivo

A página de um dispositivo poderá possuir as abas:

```text
Visão geral
Monitoramento
Interfaces
Tráfego
Serviços
Topologia
SNMP
Eventos
Configurações
```

### Topologia

Mapa gráfico dos equipamentos e suas ligações.

### Descoberta

Resultados dos scans e equipamentos pendentes de confirmação.

### Monitores

Listagem de verificações configuradas.

### Alertas

Alertas ativos, resolvidos e silenciados.

### Eventos

Linha do tempo das atividades do sistema.

### Probes

Estado e configuração dos agentes de rede.

### Configurações

Usuários, notificações, credenciais e retenção de dados.

## 6.5 Componentes Vuetify

Componentes principais:

* `VApp`.
* `VNavigationDrawer`.
* `VAppBar`.
* `VCard`.
* `VDataTable`.
* `VTreeview`.
* `VDialog`.
* `VForm`.
* `VSelect`.
* `VAutocomplete`.
* `VChip`.
* `VAlert`.
* `VSnackbar`.
* `VTabs`.
* `VMenu`.
* `VTooltip`.
* `VProgressLinear`.
* `VSkeletonLoader`.

O gráfico de topologia poderá utilizar uma biblioteca especializada integrada ao Vue, mantendo o restante da interface em Vuetify.

## 6.6 Estados visuais

Estados principais:

* Online.
* Offline.
* Instável.
* Desconhecido.
* Em manutenção.
* Descoberto.
* Ignorado.
* Desativado.

As cores deverão ser acompanhadas de ícones e textos, evitando depender somente da cor.

## 7. Backend

## 7.1 Tecnologias

O backend principal será desenvolvido com:

* Node.js.
* TypeScript.
* AdonisJS.
* PostgreSQL.
* Lucid ORM.
* SSE.
* Sistema de filas.
* Processos de worker separados.

## 7.2 Responsabilidades do AdonisJS

O AdonisJS será responsável por:

* API.
* Autenticação.
* Autorização.
* Usuários.
* Configurações.
* Validação.
* Persistência.
* Migrations.
* Controle dos sites.
* Controle das redes.
* Controle dos dispositivos.
* Controle dos monitores.
* Alertas.
* Comunicação com o frontend.

## 7.3 Motor de monitoramento

O motor de monitoramento será independente do framework.

Responsabilidades:

* Executar checks.
* Controlar intervalos.
* Controlar tentativas.
* Registrar resultados.
* Calcular disponibilidade.
* Disparar mudanças de estado.
* Coletar métricas.
* Controlar timeout.
* Distribuir tarefas entre probes.

## 7.4 Processos

```text
API
├── Recebe requisições
├── Gerencia configurações
├── Autentica usuários
└── Atualiza frontend

Worker
├── Agenda checks
├── Executa monitoramento
├── Processa métricas
├── Avalia alertas
└── Envia notificações

Probe
├── Executa tarefas na rede local
├── Descobre equipamentos
├── Consulta SNMP
├── Executa traceroute
└── Retorna resultados ao servidor
```

## 8. Estrutura do projeto

```text
network-monitor/
├── apps/
│   ├── server/
│   ├── worker/
│   ├── probe/
│   └── frontend/
│
├── packages/
│   ├── domain/
│   ├── contracts/
│   ├── checks/
│   ├── discovery/
│   ├── snmp/
│   ├── topology/
│   ├── alerts/
│   └── persistence/
│
├── docker/
├── docs/
└── package.json
```

## 9. Modelo de dados inicial

Entidades principais:

```text
User
Site
Network
Probe
Device
DeviceAddress
DeviceMac
DeviceInterface
DeviceService
DeviceCredential
Link
Check
CheckResult
MetricDefinition
MetricSample
DiscoveryRun
DiscoveryResult
RouteObservation
AlertRule
AlertEvent
NotificationChannel
SystemEvent
```

## 10. Segurança

O sistema deverá:

* Armazenar senhas com hash seguro.
* Criptografar credenciais SNMP.
* Não retornar credenciais ao frontend.
* Utilizar HTTPS.
* Validar mensagens enviadas pelos probes.
* Autenticar cada probe individualmente.
* Limitar permissões por usuário.
* Registrar alterações administrativas.
* Controlar tentativas de login.
* Permitir revogar probes.
* Impedir execução arbitrária de comandos.

Os scans deverão ocorrer somente em redes explicitamente cadastradas e autorizadas.

## 11. Armazenamento de métricas

O PostgreSQL poderá ser utilizado inicialmente para resultados e métricas.

Tipos de dados:

* Heartbeats.
* Estado dos dispositivos.
* Latência.
* Tráfego.
* Erros.
* Eventos.
* Resultados de descoberta.

Para reduzir crescimento excessivo, o sistema deverá aplicar retenção e agregação.

Exemplo:

```text
Dados brutos de 1 minuto: 7 dias
Dados agregados de 5 minutos: 30 dias
Dados agregados de 1 hora: 1 ano
```

## 12. MVP

A primeira versão deverá conter:

* Cadastro de usuário.
* Cadastro de site.
* Cadastro de rede.
* Cadastro manual de dispositivo.
* Scan de IPv4.
* Descoberta por ping.
* Resolução de hostname.
* Descoberta mDNS.
* Monitoramento por ping.
* Monitoramento HTTP e HTTPS.
* Monitoramento TCP.
* Dashboard.
* Histórico de disponibilidade.
* Alertas básicos.
* Notificação por Telegram ou e-mail.
* Relação manual entre dispositivos.
* Topologia inicial.
* Instalação standalone por Docker.

## 13. Segunda etapa

Após o MVP:

* Probe remoto.
* SNMP v1, v2c e v3.
* Inventário SNMP.
* Interfaces de rede.
* Monitoramento de tráfego.
* Traceroute.
* Descoberta LLDP.
* Regras avançadas de alertas.
* Relatórios.
* Múltiplos usuários e permissões.
* Retenção configurável.

## 14. Funcionalidades futuras

* Monitoramento IPv6 completo.
* NetFlow.
* sFlow.
* IPFIX.
* Integração com UniFi.
* Integração com MikroTik.
* Integração com Omada.
* Templates de equipamentos.
* Backup e restauração.
* Aplicativo móvel nativo.
* Descoberta de vulnerabilidades conhecidas.
* Monitoramento de qualidade Wi-Fi.
* Mapa físico dos ambientes.
* Comparação de alterações na rede.
* API pública.
* Plugins de monitoramento.

## 15. Princípios do projeto

O desenvolvimento deverá seguir os seguintes princípios:

* Interface simples.
* Configuração progressiva.
* Baixo consumo de recursos.
* Instalação fácil.
* Arquitetura modular.
* Independência entre API e coletores.
* Segurança das credenciais.
* Histórico compreensível.
* Compatibilidade com redes pequenas.
* Possibilidade de expansão futura.

## 16. Resumo técnico

```text
Frontend
Vue 3 + TypeScript + Vuetify + PWA

Backend
AdonisJS + TypeScript

Banco
PostgreSQL

Tempo real
SSE

Monitoramento
Workers Node.js separados

Probe
Node.js e TypeScript sem dependência direta do AdonisJS

Protocolos
ICMP, HTTP, HTTPS, TCP, DNS, SNMP, mDNS, SSDP e LLDP

Implantação
Docker Compose e modo standalone
```

## 17. Definição final

O projeto será uma plataforma de monitoramento e descoberta de redes focada em simplicidade.

Diferentemente de uma ferramenta voltada somente para disponibilidade, o sistema tratará separadamente:

* Equipamentos.
* Endereços.
* Interfaces.
* Serviços.
* Monitores.
* Métricas.
* Ligações.
* Rotas.
* Alertas.

Essa separação permitirá representar redes reais de maneira clara, sem transformar cada teste ou OID SNMP em um equipamento diferente.
