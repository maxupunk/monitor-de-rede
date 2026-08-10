import {
  artifactHeader,
  WG_TUNNEL_NAME,
  type ArtifactVariant,
  type PeerConfigContext,
} from './profile_contract.js'

/**
 * Scripts de terminal que instalam o cliente WireGuard e sobem o túnel sem que
 * o usuário precise abrir a interface gráfica.
 *
 * O `.conf` gerado pelo perfil continua sendo a fonte da verdade: estes scripts
 * apenas o embutem — nada é redigitado aqui, então rotação de chaves e mudança
 * de endpoint se propagam sozinhas.
 */

/** Detecção de gerenciador de pacotes: primeira família encontrada vence. */
interface PackageManager {
  /** Comando testado com `command -v`. */
  probe: string
  /** Distribuições cobertas — vira comentário no script. */
  distros: string
  install: string[]
  /** Pacote SNMP correspondente, quando o nome difere. */
  snmpInstall: string[]
}

const LINUX_PACKAGE_MANAGERS: PackageManager[] = [
  {
    probe: 'apt-get',
    distros: 'Debian, Ubuntu, Mint, Raspberry Pi OS',
    install: ['export DEBIAN_FRONTEND=noninteractive', 'apt-get update -qq', 'apt-get install -y'],
    snmpInstall: ['apt-get install -y snmpd'],
  },
  {
    probe: 'dnf',
    distros: 'Fedora, RHEL 8+, Rocky, AlmaLinux',
    install: ['dnf install -y'],
    snmpInstall: ['dnf install -y net-snmp'],
  },
  {
    probe: 'yum',
    distros: 'CentOS 7, RHEL 7',
    install: ['yum install -y epel-release || true', 'yum install -y'],
    snmpInstall: ['yum install -y net-snmp'],
  },
  {
    probe: 'zypper',
    distros: 'openSUSE, SLES',
    install: ['zypper --non-interactive install'],
    snmpInstall: ['zypper --non-interactive install net-snmp'],
  },
  {
    probe: 'pacman',
    distros: 'Arch, Manjaro',
    install: ['pacman -Sy --noconfirm'],
    snmpInstall: ['pacman -Sy --noconfirm net-snmp'],
  },
  {
    probe: 'apk',
    distros: 'Alpine',
    install: ['apk add --no-cache'],
    snmpInstall: ['apk add --no-cache net-snmp'],
  },
]

/** `if command -v apt-get; then ... elif command -v dnf; then ...` */
function packageManagerBranch(
  command: (manager: PackageManager) => string[],
  indent = '  '
): string[] {
  const lines: string[] = []

  LINUX_PACKAGE_MANAGERS.forEach((manager, index) => {
    const keyword = index === 0 ? 'if' : 'elif'
    lines.push(
      `${keyword} command -v ${manager.probe} >/dev/null 2>&1; then   # ${manager.distros}`
    )
    lines.push(...command(manager).map((line) => `${indent}${line}`))
  })

  return lines
}

function linuxSnmpSection(context: PeerConfigContext, step: (title: string) => string): string[] {
  if (!context.snmpEnabled) return []

  const community = context.snmpCommunity || 'public'

  return [
    '',
    step(`SNMP para o NetMonitor (community "${community}", restrita à faixa da VPN)`),
    ...packageManagerBranch((manager) => manager.snmpInstall),
    'else',
    "  echo 'SNMP: instale o pacote snmpd manualmente.' >&2",
    'fi',
    '',
    '# O snmpd padrão escuta apenas em 127.0.0.1 — a cópia original fica salva ao lado.',
    'install -d -m 755 /etc/snmp',
    'cp -n /etc/snmp/snmpd.conf /etc/snmp/snmpd.conf.netmonitor.bak 2>/dev/null || true',
    "cat > /etc/snmp/snmpd.conf <<'EOF'",
    '# Gerado pelo NetMonitor',
    'agentaddress udp:161',
    `rocommunity ${community} ${context.vpnCidr}`,
    'sysLocation NetMonitor',
    'sysServices 72',
    'EOF',
    '',
    'systemctl restart snmpd 2>/dev/null || rc-service snmpd restart 2>/dev/null || true',
    'systemctl enable snmpd 2>/dev/null || true',
  ]
}

/**
 * Bash único para todas as distribuições: descobre o gerenciador de pacotes,
 * instala o `wireguard-tools`, grava o perfil com permissão 600 e habilita o
 * túnel no boot.
 */
export function createLinuxBashVariant(
  context: PeerConfigContext,
  confContent: string
): ArtifactVariant {
  const iface = WG_TUNNEL_NAME

  // O bloco de SNMP é opcional, então a numeração dos passos acompanha o total.
  const totalSteps = context.snmpEnabled ? 5 : 4
  let currentStep = 0
  const step = (title: string) => `# ${++currentStep}/${totalSteps} · ${title}`

  const lines = [
    '#!/usr/bin/env bash',
    ...artifactHeader(context),
    '# Salve como netmonitor-vpn.sh e execute:  sudo bash netmonitor-vpn.sh',
    'set -euo pipefail',
    '',
    `IFACE='${iface}'`,
    'CONF="/etc/wireguard/${IFACE}.conf"',
    '',
    'if [ "$(id -u)" -ne 0 ]; then',
    "  echo 'Execute como root: sudo bash netmonitor-vpn.sh' >&2",
    '  exit 1',
    'fi',
    '',
    step('Instala o wireguard-tools pelo gerenciador de pacotes da distribuição'),
    ...packageManagerBranch((manager) => [
      ...manager.install.slice(0, -1),
      `${manager.install[manager.install.length - 1]} wireguard-tools`,
    ]),
    'else',
    "  echo 'Gerenciador de pacotes não reconhecido — instale o wireguard-tools manualmente.' >&2",
    '  exit 1',
    'fi',
    '',
    step('Grava o perfil (a chave privada fica legível apenas pelo root)'),
    'install -d -m 700 /etc/wireguard',
    'umask 077',
    'cat > "$CONF" <<\'EOF\'',
    confContent.trimEnd(),
    'EOF',
    'chmod 600 "$CONF"',
    '',
    '# Sem a chave privada o tunel nunca fecha — melhor parar aqui do que deixar',
    '# um wg-quick habilitado no boot tentando subir uma configuracao invalida.',
    'if grep -q CHAVE-PRIVADA-INDISPONIVEL "$CONF"; then',
    '  rm -f "$CONF"',
    '  echo \'A chave privada deste dispositivo ja foi entregue. No NetMonitor, use "Rotacionar chaves" e copie o script novo.\' >&2',
    '  exit 1',
    'fi',
    '',
    step('Sobe o túnel agora e a cada reinício'),
    'if command -v systemctl >/dev/null 2>&1; then',
    '  systemctl enable --now "wg-quick@${IFACE}"',
    'else',
    '  wg-quick up "$IFACE"   # sem systemd (OpenRC): rc-update add wg-quick.${IFACE}',
    'fi',
    '',
    step('Conferência — "latest handshake" deve aparecer em poucos segundos'),
    'wg show "$IFACE"',
    ...linuxSnmpSection(context, step),
    '',
    `echo 'Túnel ${iface} ativo. O dispositivo aparece como conectado no NetMonitor.'`,
  ]

  return {
    id: 'bash',
    label: 'Script Bash (apt / dnf / yum / zypper / pacman / apk)',
    hint: 'Detecta a distribuição automaticamente — Debian, Ubuntu, Fedora, RHEL, SUSE, Arch e Alpine',
    icon: 'mdi-console',
    fileName: 'netmonitor-vpn.sh',
    language: 'shell',
    content: `${lines.join('\n')}\n`,
    instructions: [
      'Copie o script e salve no servidor como netmonitor-vpn.sh.',
      'Execute com: sudo bash netmonitor-vpn.sh.',
      'O túnel sobe na hora e volta sozinho a cada reinício da máquina.',
    ],
  }
}

/** Passo a passo para abrir um PowerShell elevado — vale em qualquer versão e idioma do Windows. */
const OPEN_AS_ADMIN =
  'menu Iniciar > digite "powershell" > botao direito em "Windows PowerShell" > "Executar como administrador"'

/**
 * PowerShell de ponta a ponta: instala o cliente oficial via winget, grava o
 * perfil, registra o túnel como serviço do Windows (sobe antes do logon) e abre
 * o WireGuard já com o túnel na lista — o usuário não digita nada além do bloco.
 *
 * Todo o corpo vive dentro de `& { ... }` por um motivo específico: colado no
 * console, cada linha de nível superior é um comando independente, e
 * `$ErrorActionPreference = 'Stop'` não impede a linha seguinte de rodar. Sem o
 * bloco, uma falha no meio (permissão negada ao gravar o perfil, por exemplo)
 * deixava o script seguir até o fim e produzir meia-instalação silenciosa.
 * Como scriptblock, é um único comando: o primeiro `throw` aborta tudo.
 *
 * O texto é ASCII puro — o console do PowerShell 5.1 roda em codepage legada e
 * embaralha acentos colados, inclusive dentro das mensagens de erro.
 */
export function createWindowsWingetVariant(
  context: PeerConfigContext,
  confContent: string
): ArtifactVariant {
  const tunnel = WG_TUNNEL_NAME

  const lines = [
    ...artifactHeader(context),
    `# 1) Abra o PowerShell como ADMINISTRADOR: ${OPEN_AS_ADMIN}.`,
    '# 2) Cole este bloco inteiro e pressione Enter.',
    '',
    '& {',
    "  $ErrorActionPreference = 'Stop'",
    '',
    `  $Tunnel = '${tunnel}'`,
    "  $WgHome = Join-Path $env:ProgramFiles 'WireGuard'",
    "  $WgExe  = Join-Path $WgHome 'wireguard.exe'",
    '  $ServiceName = "WireGuardTunnel`$$Tunnel"',
    '',
    "  $Conf = @'",
    confContent.trimEnd(),
    "'@",
    '',
    '  # 1/5 - Confere o terreno antes de mexer em qualquer coisa',
    '  $identity = [Security.Principal.WindowsIdentity]::GetCurrent()',
    '  $isAdmin = ([Security.Principal.WindowsPrincipal]$identity).IsInRole(',
    '    [Security.Principal.WindowsBuiltInRole]::Administrator)',
    '  if (-not $isAdmin) {',
    `    throw 'Este bloco precisa de um PowerShell ADMINISTRADOR: ${OPEN_AS_ADMIN}.'`,
    '  }',
    "  if ($Conf -match 'CHAVE-PRIVADA-INDISPONIVEL') {",
    '    throw \'A chave privada deste dispositivo ja foi entregue e nao pode ser exibida outra vez. No NetMonitor, use "Rotacionar chaves" e copie o script novo.\'',
    '  }',
    '  if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {',
    '    throw \'winget nao encontrado. Instale o "Instalador de Aplicativo" pela Microsoft Store ou baixe o cliente em https://www.wireguard.com/install/\'',
    '  }',
    '',
    '  # 2/5 - Instala o cliente oficial WireGuard',
    '  winget install --exact --id WireGuard.WireGuard --silent `',
    '    --accept-source-agreements --accept-package-agreements',
    '  # winget termina em erro quando o pacote ja esta instalado: o que importa e o executavel existir.',
    '  if (-not (Test-Path $WgExe)) {',
    '    throw "WireGuard nao encontrado em $WgHome. Verifique se a instalacao foi concluida."',
    '  }',
    '',
    '  # 3/4 - Desfaz instalacoes anteriores feitas por este script',
    '  # Um servico proprio (/installtunnelservice) e o tunel gerenciado pela',
    '  # janela do WireGuard disputam o mesmo adaptador: os dois nao coexistem.',
    '  if (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue) {',
    '    & $WgExe /uninstalltunnelservice $Tunnel',
    '    Start-Sleep -Seconds 3',
    '  }',
    "  $Legacy = Join-Path $env:ProgramData 'NetMonitor'",
    '  if (Test-Path $Legacy) {',
    '    # Versoes antigas deixavam o perfil aqui com uma ACL de leitura que nao',
    '    # inclui DELETE - nem o administrador apagava sem tomar posse antes.',
    '    $stale = @(Get-ChildItem $Legacy -Force -Recurse -ErrorAction SilentlyContinue |',
    '      ForEach-Object { $_.FullName }) + @($Legacy)',
    '    foreach ($item in $stale) {',
    '      takeown /f $item /a | Out-Null',
    '      icacls $item /reset | Out-Null',
    '    }',
    '    Remove-Item -Path $Legacy -Recurse -Force -ErrorAction SilentlyContinue',
    '  }',
    '',
    '  # 4/4 - Grava o perfil no cofre do proprio WireGuard',
    '  # O gerenciador monitora essa pasta e lista o tunel na hora, sem reiniciar.',
    '  # Ao ativar, ele cria o servico (que sobe no boot) e cifra o perfil como',
    '  # .conf.dpapi - por isso o arquivo nao pode ser dono de servico nenhum aqui.',
    "  $Store = Join-Path $WgHome 'Data\\Configurations'",
    '  if (-not (Test-Path $Store)) {',
    '    New-Item -ItemType Directory -Path $Store -Force | Out-Null',
    '    icacls $Store /inheritance:r /grant:r "*S-1-5-18:(F)" "*S-1-5-32-544:(F)" | Out-Null',
    '  }',
    '',
    '  # O nome do arquivo vira o nome do tunel: precisa ser <tunel>.conf',
    '  $File = Join-Path $Store "$Tunnel.conf"',
    '  # Um .conf.dpapi de importacao anterior tem prioridade sobre o .conf e',
    '  # manteria o perfil velho no lugar do novo.',
    '  Remove-Item -Path "$File.dpapi" -Force -ErrorAction SilentlyContinue',
    '  Remove-Item -Path $File -Force -ErrorAction SilentlyContinue',
    '  $Conf | Set-Content -Path $File -Encoding ascii',
    '  # A chave privada fica em texto puro ate o WireGuard cifrar o perfil: so',
    '  # SYSTEM (que roda o gerenciador) e Administradores enxergam nesse meio tempo.',
    '  # (SIDs no lugar dos nomes, para funcionar em Windows de qualquer idioma.)',
    '  icacls $File /inheritance:r /grant:r "*S-1-5-18:(F)" "*S-1-5-32-544:(F)" | Out-Null',
    '',
    '  # Abre a janela do WireGuard com o tunel ja na lista',
    '  Start-Process -FilePath $WgExe',
    "  Write-Host ''",
    `  Write-Host 'Perfil "${tunnel}" instalado no WireGuard.'`,
    `  Write-Host 'Na janela que abriu, selecione "${tunnel}" e clique em Ativar.'`,
    "  Write-Host 'Dai em diante o WireGuard gerencia o tunel e ele volta sozinho a cada boot.'",
    '}',
    '',
    `# Para remover depois: basta excluir o tunel "${tunnel}" na janela do WireGuard.`,
  ]

  return {
    id: 'winget',
    label: 'PowerShell + winget',
    hint: 'Windows 10/11 e Server 2022+ — instala o cliente e deixa o túnel pronto na janela do WireGuard',
    icon: 'mdi-powershell',
    fileName: 'netmonitor-vpn.ps1',
    language: 'powershell',
    content: `${lines.join('\n')}\n`,
    instructions: [
      'Abra o PowerShell como Administrador: menu Iniciar, digite "powershell", clique com o botão direito e escolha "Executar como administrador".',
      'Cole o bloco completo e pressione Enter — ele para sozinho se algo der errado.',
      'Na janela do WireGuard que abrir, selecione o túnel e clique em "Ativar" — daí em diante ele volta sozinho a cada boot.',
    ],
  }
}
