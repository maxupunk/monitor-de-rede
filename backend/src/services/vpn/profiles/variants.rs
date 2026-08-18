//! Scripts de terminal que instalam o cliente WireGuard e sobem o túnel sem
//! que o usuário precise abrir a interface gráfica (§8.10.5).
//!
//! O `.conf` gerado pelo perfil continua sendo a fonte da verdade: estes
//! scripts apenas o embutem — nada é redigitado aqui, então rotação de chaves e
//! mudança de endpoint se propagam sozinhas.

use super::contract::{artifact_header, ArtifactVariant, PeerConfigContext, WG_TUNNEL_NAME};

/// Detecção de gerenciador de pacotes: primeira família encontrada vence.
struct PackageManager {
    /// Comando testado com `command -v`.
    probe: &'static str,
    /// Distribuições cobertas — vira comentário no script.
    distros: &'static str,
    install: &'static [&'static str],
    /// Pacote SNMP correspondente, quando o nome difere.
    snmp_install: &'static [&'static str],
}

const LINUX_PACKAGE_MANAGERS: [PackageManager; 6] = [
    PackageManager {
        probe: "apt-get",
        distros: "Debian, Ubuntu, Mint, Raspberry Pi OS",
        install: &[
            "export DEBIAN_FRONTEND=noninteractive",
            "apt-get update -qq",
            "apt-get install -y",
        ],
        snmp_install: &["apt-get install -y snmpd"],
    },
    PackageManager {
        probe: "dnf",
        distros: "Fedora, RHEL 8+, Rocky, AlmaLinux",
        install: &["dnf install -y"],
        snmp_install: &["dnf install -y net-snmp"],
    },
    PackageManager {
        probe: "yum",
        distros: "CentOS 7, RHEL 7",
        install: &["yum install -y epel-release || true", "yum install -y"],
        snmp_install: &["yum install -y net-snmp"],
    },
    PackageManager {
        probe: "zypper",
        distros: "openSUSE, SLES",
        install: &["zypper --non-interactive install"],
        snmp_install: &["zypper --non-interactive install net-snmp"],
    },
    PackageManager {
        probe: "pacman",
        distros: "Arch, Manjaro",
        install: &["pacman -Sy --noconfirm"],
        snmp_install: &["pacman -Sy --noconfirm net-snmp"],
    },
    PackageManager {
        probe: "apk",
        distros: "Alpine",
        install: &["apk add --no-cache"],
        snmp_install: &["apk add --no-cache net-snmp"],
    },
];

/// `if command -v apt-get; then ... elif command -v dnf; then ...`
fn package_manager_branch(command: impl Fn(&PackageManager) -> Vec<String>) -> Vec<String> {
    let mut lines = Vec::new();
    for (index, manager) in LINUX_PACKAGE_MANAGERS.iter().enumerate() {
        let keyword = if index == 0 { "if" } else { "elif" };
        lines.push(format!(
            "{keyword} command -v {} >/dev/null 2>&1; then   # {}",
            manager.probe, manager.distros
        ));
        lines.extend(command(manager).into_iter().map(|line| format!("  {line}")));
    }
    lines
}

fn linux_snmp_section(
    context: &PeerConfigContext,
    step: &mut impl FnMut(&str) -> String,
) -> Vec<String> {
    if !context.snmp_enabled {
        return Vec::new();
    }
    let community = context.sanitized_community();
    let mut lines = vec![
        String::new(),
        step(&format!(
            "SNMP para o NetMonitor (community \"{community}\", restrita à faixa da VPN)"
        )),
    ];
    lines.extend(package_manager_branch(|manager| {
        manager
            .snmp_install
            .iter()
            .map(ToString::to_string)
            .collect()
    }));
    lines.extend([
        "else".to_string(),
        "  echo 'SNMP: instale o pacote snmpd manualmente.' >&2".to_string(),
        "fi".to_string(),
        String::new(),
        "# O snmpd padrão escuta apenas em 127.0.0.1 — a cópia original fica salva ao lado."
            .to_string(),
        "install -d -m 755 /etc/snmp".to_string(),
        "cp -n /etc/snmp/snmpd.conf /etc/snmp/snmpd.conf.netmonitor.bak 2>/dev/null || true"
            .to_string(),
        "cat > /etc/snmp/snmpd.conf <<'EOF'".to_string(),
        "# Gerado pelo NetMonitor".to_string(),
        "agentaddress udp:161".to_string(),
        format!("rocommunity {community} {}", context.vpn_cidr),
        "sysLocation NetMonitor".to_string(),
        "sysServices 72".to_string(),
        "EOF".to_string(),
        String::new(),
        "systemctl restart snmpd 2>/dev/null || rc-service snmpd restart 2>/dev/null || true"
            .to_string(),
        "systemctl enable snmpd 2>/dev/null || true".to_string(),
    ]);
    lines
}

/// Bash único para todas as distribuições: descobre o gerenciador de pacotes,
/// instala o `wireguard-tools`, grava o perfil com permissão 600 e habilita o
/// túnel no boot.
#[must_use]
pub fn linux_bash_variant(context: &PeerConfigContext, conf_content: &str) -> ArtifactVariant {
    let iface = WG_TUNNEL_NAME;
    // O bloco de SNMP é opcional, então a numeração dos passos acompanha o total.
    let total_steps = if context.snmp_enabled { 5 } else { 4 };
    let mut current_step = 0;
    let mut step = move |title: &str| {
        current_step += 1;
        format!("# {current_step}/{total_steps} · {title}")
    };

    let mut lines = vec!["#!/usr/bin/env bash".to_string()];
    lines.extend(artifact_header(context));
    lines.extend([
        "# Salve como netmonitor-vpn.sh e execute:  sudo bash netmonitor-vpn.sh".to_string(),
        "set -euo pipefail".to_string(),
        String::new(),
        format!("IFACE='{iface}'"),
        "CONF=\"/etc/wireguard/${IFACE}.conf\"".to_string(),
        String::new(),
        "if [ \"$(id -u)\" -ne 0 ]; then".to_string(),
        "  echo 'Execute como root: sudo bash netmonitor-vpn.sh' >&2".to_string(),
        "  exit 1".to_string(),
        "fi".to_string(),
        String::new(),
        step("Instala o wireguard-tools pelo gerenciador de pacotes da distribuição"),
    ]);
    lines.extend(package_manager_branch(|manager| {
        let mut commands: Vec<String> = manager.install[..manager.install.len() - 1]
            .iter()
            .map(ToString::to_string)
            .collect();
        commands.push(format!(
            "{} wireguard-tools",
            manager.install[manager.install.len() - 1]
        ));
        commands
    }));
    lines.extend([
        "else".to_string(),
        "  echo 'Gerenciador de pacotes não reconhecido — instale o wireguard-tools manualmente.' >&2"
            .to_string(),
        "  exit 1".to_string(),
        "fi".to_string(),
        String::new(),
        step("Grava o perfil (a chave privada fica legível apenas pelo root)"),
        "install -d -m 700 /etc/wireguard".to_string(),
        "umask 077".to_string(),
        "cat > \"$CONF\" <<'EOF'".to_string(),
        conf_content.trim_end().to_string(),
        "EOF".to_string(),
        "chmod 600 \"$CONF\"".to_string(),
        String::new(),
        "# Sem a chave privada o tunel nunca fecha — melhor parar aqui do que deixar".to_string(),
        "# um wg-quick habilitado no boot tentando subir uma configuracao invalida.".to_string(),
        "if grep -q CHAVE-PRIVADA-INDISPONIVEL \"$CONF\"; then".to_string(),
        "  rm -f \"$CONF\"".to_string(),
        "  echo 'A chave privada deste dispositivo ja foi entregue. No NetMonitor, use \"Rotacionar chaves\" e copie o script novo.' >&2"
            .to_string(),
        "  exit 1".to_string(),
        "fi".to_string(),
        String::new(),
        step("Sobe o túnel agora e a cada reinício"),
        "if command -v systemctl >/dev/null 2>&1; then".to_string(),
        "  systemctl enable --now \"wg-quick@${IFACE}\"".to_string(),
        "else".to_string(),
        "  wg-quick up \"$IFACE\"   # sem systemd (OpenRC): rc-update add wg-quick.${IFACE}"
            .to_string(),
        "fi".to_string(),
        String::new(),
        step("Conferência — \"latest handshake\" deve aparecer em poucos segundos"),
        "wg show \"$IFACE\"".to_string(),
    ]);
    lines.extend(linux_snmp_section(context, &mut step));
    lines.extend([
        String::new(),
        format!("echo 'Túnel {iface} ativo. O dispositivo aparece como conectado no NetMonitor.'"),
    ]);

    ArtifactVariant {
        id: "bash".to_string(),
        label: "Script Bash (apt / dnf / yum / zypper / pacman / apk)".to_string(),
        hint: "Detecta a distribuição automaticamente — Debian, Ubuntu, Fedora, RHEL, SUSE, Arch e Alpine"
            .to_string(),
        icon: "mdi-console".to_string(),
        file_name: "netmonitor-vpn.sh".to_string(),
        language: "shell".to_string(),
        content: format!("{}\n", lines.join("\n")),
        instructions: vec![
            "Copie o script e salve no servidor como netmonitor-vpn.sh.".to_string(),
            "Execute com: sudo bash netmonitor-vpn.sh.".to_string(),
            "O túnel sobe na hora e volta sozinho a cada reinício da máquina.".to_string(),
        ],
    }
}

/// Passo a passo para abrir um PowerShell elevado — vale em qualquer versão e
/// idioma do Windows.
const OPEN_AS_ADMIN: &str = "menu Iniciar > digite \"powershell\" > botao direito em \"Windows PowerShell\" > \"Executar como administrador\"";

/// PowerShell de ponta a ponta: instala o cliente oficial via winget, grava o
/// perfil, registra o túnel no cofre do WireGuard e abre a janela já com o
/// túnel na lista — o usuário não digita nada além do bloco.
///
/// Todo o corpo vive dentro de `& { ... }` por um motivo específico: colado no
/// console, cada linha de nível superior é um comando independente, e
/// `$ErrorActionPreference = 'Stop'` não impede a linha seguinte de rodar. Sem
/// o bloco, uma falha no meio (permissão negada ao gravar o perfil, por
/// exemplo) deixava o script seguir até o fim e produzir meia-instalação
/// silenciosa. Como scriptblock, é um único comando: o primeiro `throw` aborta
/// tudo.
///
/// O texto é ASCII puro — o console do PowerShell 5.1 roda em codepage legada e
/// embaralha acentos colados, inclusive dentro das mensagens de erro.
#[must_use]
pub fn windows_winget_variant(context: &PeerConfigContext, conf_content: &str) -> ArtifactVariant {
    let tunnel = WG_TUNNEL_NAME;
    let mut lines = artifact_header(context);
    lines.extend([
        format!("# 1) Abra o PowerShell como ADMINISTRADOR: {OPEN_AS_ADMIN}."),
        "# 2) Cole este bloco inteiro e pressione Enter.".to_string(),
        String::new(),
        "& {".to_string(),
        "  $ErrorActionPreference = 'Stop'".to_string(),
        String::new(),
        format!("  $Tunnel = '{tunnel}'"),
        "  $WgHome = Join-Path $env:ProgramFiles 'WireGuard'".to_string(),
        "  $WgExe  = Join-Path $WgHome 'wireguard.exe'".to_string(),
        "  $ServiceName = \"WireGuardTunnel`$$Tunnel\"".to_string(),
        String::new(),
        "  $Conf = @'".to_string(),
        conf_content.trim_end().to_string(),
        "'@".to_string(),
        String::new(),
        "  # 1/5 - Confere o terreno antes de mexer em qualquer coisa".to_string(),
        "  $identity = [Security.Principal.WindowsIdentity]::GetCurrent()".to_string(),
        "  $isAdmin = ([Security.Principal.WindowsPrincipal]$identity).IsInRole(".to_string(),
        "    [Security.Principal.WindowsBuiltInRole]::Administrator)".to_string(),
        "  if (-not $isAdmin) {".to_string(),
        format!("    throw 'Este bloco precisa de um PowerShell ADMINISTRADOR: {OPEN_AS_ADMIN}.'"),
        "  }".to_string(),
        "  if ($Conf -match 'CHAVE-PRIVADA-INDISPONIVEL') {".to_string(),
        "    throw 'A chave privada deste dispositivo ja foi entregue e nao pode ser exibida outra vez. No NetMonitor, use \"Rotacionar chaves\" e copie o script novo.'"
            .to_string(),
        "  }".to_string(),
        "  if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {".to_string(),
        "    throw 'winget nao encontrado. Instale o \"Instalador de Aplicativo\" pela Microsoft Store ou baixe o cliente em https://www.wireguard.com/install/'"
            .to_string(),
        "  }".to_string(),
        String::new(),
        "  # 2/5 - Instala o cliente oficial WireGuard".to_string(),
        "  winget install --exact --id WireGuard.WireGuard --silent `".to_string(),
        "    --accept-source-agreements --accept-package-agreements".to_string(),
        "  # winget termina em erro quando o pacote ja esta instalado: o que importa e o executavel existir."
            .to_string(),
        "  if (-not (Test-Path $WgExe)) {".to_string(),
        "    throw \"WireGuard nao encontrado em $WgHome. Verifique se a instalacao foi concluida.\""
            .to_string(),
        "  }".to_string(),
        String::new(),
        "  # 3/4 - Desfaz instalacoes anteriores feitas por este script".to_string(),
        "  # Um servico proprio (/installtunnelservice) e o tunel gerenciado pela".to_string(),
        "  # janela do WireGuard disputam o mesmo adaptador: os dois nao coexistem.".to_string(),
        "  if (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue) {".to_string(),
        "    & $WgExe /uninstalltunnelservice $Tunnel".to_string(),
        "    Start-Sleep -Seconds 3".to_string(),
        "  }".to_string(),
        "  $Legacy = Join-Path $env:ProgramData 'NetMonitor'".to_string(),
        "  if (Test-Path $Legacy) {".to_string(),
        "    # Versoes antigas deixavam o perfil aqui com uma ACL de leitura que nao".to_string(),
        "    # inclui DELETE - nem o administrador apagava sem tomar posse antes.".to_string(),
        "    $stale = @(Get-ChildItem $Legacy -Force -Recurse -ErrorAction SilentlyContinue |"
            .to_string(),
        "      ForEach-Object { $_.FullName }) + @($Legacy)".to_string(),
        "    foreach ($item in $stale) {".to_string(),
        "      takeown /f $item /a | Out-Null".to_string(),
        "      icacls $item /reset | Out-Null".to_string(),
        "    }".to_string(),
        "    Remove-Item -Path $Legacy -Recurse -Force -ErrorAction SilentlyContinue".to_string(),
        "  }".to_string(),
        String::new(),
        "  # 4/4 - Grava o perfil no cofre do proprio WireGuard".to_string(),
        "  # O gerenciador monitora essa pasta e lista o tunel na hora, sem reiniciar.".to_string(),
        "  # Ao ativar, ele cria o servico (que sobe no boot) e cifra o perfil como".to_string(),
        "  # .conf.dpapi - por isso o arquivo nao pode ser dono de servico nenhum aqui.".to_string(),
        "  $Store = Join-Path $WgHome 'Data\\Configurations'".to_string(),
        "  if (-not (Test-Path $Store)) {".to_string(),
        "    New-Item -ItemType Directory -Path $Store -Force | Out-Null".to_string(),
        "    icacls $Store /inheritance:r /grant:r \"*S-1-5-18:(F)\" \"*S-1-5-32-544:(F)\" | Out-Null"
            .to_string(),
        "  }".to_string(),
        String::new(),
        "  # O nome do arquivo vira o nome do tunel: precisa ser <tunel>.conf".to_string(),
        "  $File = Join-Path $Store \"$Tunnel.conf\"".to_string(),
        "  # Um .conf.dpapi de importacao anterior tem prioridade sobre o .conf e".to_string(),
        "  # manteria o perfil velho no lugar do novo.".to_string(),
        "  Remove-Item -Path \"$File.dpapi\" -Force -ErrorAction SilentlyContinue".to_string(),
        "  Remove-Item -Path $File -Force -ErrorAction SilentlyContinue".to_string(),
        "  $Conf | Set-Content -Path $File -Encoding ascii".to_string(),
        "  # A chave privada fica em texto puro ate o WireGuard cifrar o perfil: so".to_string(),
        "  # SYSTEM (que roda o gerenciador) e Administradores enxergam nesse meio tempo."
            .to_string(),
        "  # (SIDs no lugar dos nomes, para funcionar em Windows de qualquer idioma.)".to_string(),
        "  icacls $File /inheritance:r /grant:r \"*S-1-5-18:(F)\" \"*S-1-5-32-544:(F)\" | Out-Null"
            .to_string(),
        String::new(),
        "  # Abre a janela do WireGuard com o tunel ja na lista".to_string(),
        "  Start-Process -FilePath $WgExe".to_string(),
        "  Write-Host ''".to_string(),
        format!("  Write-Host 'Perfil \"{tunnel}\" instalado no WireGuard.'"),
        format!("  Write-Host 'Na janela que abriu, selecione \"{tunnel}\" e clique em Ativar.'"),
        "  Write-Host 'Dai em diante o WireGuard gerencia o tunel e ele volta sozinho a cada boot.'"
            .to_string(),
        "}".to_string(),
        String::new(),
        format!("# Para remover depois: basta excluir o tunel \"{tunnel}\" na janela do WireGuard."),
    ]);

    ArtifactVariant {
        id: "winget".to_string(),
        label: "PowerShell + winget".to_string(),
        hint: "Windows 10/11 e Server 2022+ — instala o cliente e deixa o túnel pronto na janela do WireGuard"
            .to_string(),
        icon: "mdi-powershell".to_string(),
        file_name: "netmonitor-vpn.ps1".to_string(),
        language: "powershell".to_string(),
        content: format!("{}\n", lines.join("\n")),
        instructions: vec![
            "Abra o PowerShell como Administrador: menu Iniciar, digite \"powershell\", clique com o botão direito e escolha \"Executar como administrador\"."
                .to_string(),
            "Cole o bloco completo e pressione Enter — ele para sozinho se algo der errado."
                .to_string(),
            "Na janela do WireGuard que abrir, selecione o túnel e clique em \"Ativar\" — daí em diante ele volta sozinho a cada boot."
                .to_string(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::super::contract::{tests::contexto, PRIVATE_KEY_UNAVAILABLE};
    use super::*;

    const CONF: &str = "[Interface]\nPrivateKey = CHAVE\nAddress = 10.8.0.11/24\n";

    #[test]
    fn o_bash_bate_com_o_esperado() {
        insta::assert_snapshot!(linux_bash_variant(&contexto(), CONF).content);
    }

    #[test]
    fn o_powershell_bate_com_o_esperado() {
        insta::assert_snapshot!(windows_winget_variant(&contexto(), CONF).content);
    }

    #[test]
    fn o_conf_e_embutido_sem_ser_redigitado() {
        let bash = linux_bash_variant(&contexto(), CONF);
        assert!(bash.content.contains("PrivateKey = CHAVE"));
        let ps = windows_winget_variant(&contexto(), CONF);
        assert!(ps.content.contains("PrivateKey = CHAVE"));
    }

    #[test]
    fn a_numeracao_dos_passos_acompanha_o_snmp() {
        let sem = linux_bash_variant(&contexto(), CONF);
        assert!(sem.content.contains("# 1/4 ·"));
        assert!(!sem.content.contains("/5 ·"));

        let mut context = contexto();
        context.snmp_enabled = true;
        let com = linux_bash_variant(&context, CONF);
        assert!(com.content.contains("# 1/5 ·"));
        assert!(com.content.contains("# 5/5 ·"));
        assert!(com.content.contains("rocommunity public 10.8.0.0/24"));
    }

    #[test]
    fn o_bash_aborta_quando_a_chave_ja_foi_consumida() {
        let mut context = contexto();
        context.client_private_key = PRIVATE_KEY_UNAVAILABLE.into();
        let variant = linux_bash_variant(
            &context,
            "[Interface]\nPrivateKey = <CHAVE-PRIVADA-INDISPONIVEL-ROTACIONE-AS-CHAVES>\n",
        );
        // O guarda em runtime existe justamente para não deixar um wg-quick
        // habilitado no boot com configuração inválida.
        assert!(variant
            .content
            .contains("grep -q CHAVE-PRIVADA-INDISPONIVEL"));
        assert!(variant.content.contains("exit 1"));
    }

    #[test]
    fn o_powershell_e_ascii_puro() {
        // O console do PowerShell 5.1 embaralha acento colado — inclusive nas
        // mensagens de erro, que é justamente quando o texto precisa ser lido.
        let content = windows_winget_variant(&contexto(), CONF).content;
        assert!(content.is_ascii(), "o script do Windows precisa ser ASCII");
    }

    #[test]
    fn o_powershell_roda_dentro_de_um_scriptblock() {
        let content = windows_winget_variant(&contexto(), CONF).content;
        assert!(content.contains("& {"));
        assert!(content.contains("$ErrorActionPreference = 'Stop'"));
    }
}
