import 'package:flutter/material.dart';
import 'package:netmonitor_app/features/core/models/log_models.dart';
import 'package:netmonitor_app/features/core/models/settings_models.dart'
    show ServerAddressEntry;
import 'package:netmonitor_app/shared/device_access/device_access_models.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Modo de configuração do painel "Receber logs (Syslog)".
enum SyslogSetupMode { automatic, manual }

/// Método de autorização escolhido para a configuração automática de Syslog.
enum SyslogAccessMethod { dedicatedSshKey, oneTimePassword }

/// Painel coeso "Receber logs (Syslog)" no cadastro/edição de dispositivo.
class DeviceSyslogSetupPanel extends StatelessWidget {
  const DeviceSyslogSetupPanel({
    super.key,
    required this.enabled,
    required this.loading,
    required this.onToggle,
    this.guide,
    this.mode = SyslogSetupMode.automatic,
    this.accessMethod = SyslogAccessMethod.dedicatedSshKey,
    this.isExistingDevice = false,
    this.resolvedServerAddress,
    this.serverPort = 514,
    this.usableAddresses = const <ServerAddressEntry>[],
    this.serverAddressController,
    this.existingCredential,
    this.reuseExistingAccess = false,
    this.sshAdminUserController,
    this.sshAdminPasswordController,
    this.sshDedicatedPortController,
    this.oneTimeProtocol = 'ssh',
    this.oneTimeUserController,
    this.oneTimePasswordController,
    this.oneTimePortController,
    this.provisioning = false,
    this.provisionStep,
    this.provisionError,
    this.provisionResult,
    this.operatingSystemId,
    this.operatingSystemLabel,
    this.vendorHint,
    this.manualSnippetIndex,
    this.onModeChanged,
    this.onAccessMethodChanged,
    this.onReuseExistingAccessChanged,
    this.onOneTimeProtocolChanged,
    this.onSelectServerAddress,
    this.onSelectSnippetIndex,
    this.onRetryLoad,
    this.onSshAdminUserChanged,
    this.onOneTimeUserChanged,
    this.onProvisionNow,
  });

  static const String unknownServerPlaceholder = '<IP-DO-SERVIDOR>';

  final bool enabled;
  final bool loading;
  final SetupGuide? guide;
  final SyslogSetupMode mode;
  final SyslogAccessMethod accessMethod;
  final bool isExistingDevice;
  final String? resolvedServerAddress;
  final int serverPort;
  final List<ServerAddressEntry> usableAddresses;
  final TextEditingController? serverAddressController;
  final DeviceCredentialView? existingCredential;
  final bool reuseExistingAccess;
  final TextEditingController? sshAdminUserController;
  final TextEditingController? sshAdminPasswordController;
  final TextEditingController? sshDedicatedPortController;
  final String oneTimeProtocol;
  final TextEditingController? oneTimeUserController;
  final TextEditingController? oneTimePasswordController;
  final TextEditingController? oneTimePortController;
  final bool provisioning;
  final String? provisionStep;
  final String? provisionError;
  final ProvisionLoggingResponse? provisionResult;
  final String? operatingSystemId;
  final String? operatingSystemLabel;
  final String? vendorHint;
  final int? manualSnippetIndex;
  final ValueChanged<bool> onToggle;
  final ValueChanged<SyslogSetupMode>? onModeChanged;
  final ValueChanged<SyslogAccessMethod>? onAccessMethodChanged;
  final ValueChanged<bool>? onReuseExistingAccessChanged;
  final ValueChanged<String>? onOneTimeProtocolChanged;
  final ValueChanged<String?>? onSelectServerAddress;
  final ValueChanged<int?>? onSelectSnippetIndex;
  final VoidCallback? onRetryLoad;
  final ValueChanged<String>? onSshAdminUserChanged;
  final ValueChanged<String>? onOneTimeUserChanged;
  final VoidCallback? onProvisionNow;

  static String? resolveBestServerAddress({
    String? selectedAddress,
    String? hintedAddress,
    List<ServerAddressEntry> usableAddresses = const <ServerAddressEntry>[],
    String? preferredAddressId,
    String? guideAddress,
  }) {
    bool isReal(String? value) {
      final clean = value?.trim() ?? '';
      return clean.isNotEmpty && clean != unknownServerPlaceholder;
    }

    if (isReal(selectedAddress)) return selectedAddress!.trim();
    if (isReal(hintedAddress)) return hintedAddress!.trim();
    if (preferredAddressId != null && preferredAddressId.isNotEmpty) {
      for (final entry in usableAddresses) {
        if (entry.id == preferredAddressId && isReal(entry.value)) {
          return entry.value!.trim();
        }
      }
    }
    for (final entry in usableAddresses) {
      if (isReal(entry.value)) return entry.value!.trim();
    }
    if (isReal(guideAddress)) return guideAddress!.trim();
    return null;
  }

  static String? inferDedicatedPlatform({
    String? osId,
    String? vendorHint,
    String? accessPlatform,
  }) {
    final explicit = accessPlatform?.trim().toLowerCase() ?? '';
    if (explicit == 'mikrotik' || explicit == 'openwrt') return explicit;
    final combined = '${osId ?? ''} ${vendorHint ?? ''}'.toLowerCase();
    if (combined.contains('openwrt') || combined.contains('lede')) {
      return 'openwrt';
    }
    if (combined.contains('routeros') || combined.contains('mikrotik')) {
      return 'mikrotik';
    }
    return null;
  }

  static int? resolveSnippetIndex(
    List<SetupSnippet> snippets, {
    String? osId,
    String? osLabel,
    String? vendorHint,
  }) {
    if (snippets.isEmpty) return null;

    final idNeedle = osId?.trim().toLowerCase() ?? '';
    final labelNeedle = osLabel?.trim().toLowerCase() ?? '';
    final vendorNeedle = vendorHint?.trim().toLowerCase() ?? '';

    if (idNeedle.isNotEmpty) {
      for (var i = 0; i < snippets.length; i++) {
        if (snippets[i].id.trim().toLowerCase() == idNeedle) return i;
      }
      for (var i = 0; i < snippets.length; i++) {
        final combined = '${snippets[i].id} ${snippets[i].label}'.toLowerCase();
        if (combined.contains(idNeedle) ||
            _matchesOsAlias(idNeedle, combined)) {
          return i;
        }
      }
    }

    if (labelNeedle.isNotEmpty) {
      for (var i = 0; i < snippets.length; i++) {
        final label = snippets[i].label.toLowerCase();
        if (label.contains(labelNeedle) || labelNeedle.contains(label)) {
          return i;
        }
      }
    }

    if (vendorNeedle.isNotEmpty) {
      for (var i = 0; i < snippets.length; i++) {
        final combined = '${snippets[i].id} ${snippets[i].label}'.toLowerCase();
        if (combined.contains(vendorNeedle) ||
            _matchesOsAlias(vendorNeedle, combined)) {
          return i;
        }
      }
    }

    return 0;
  }

  static bool _matchesOsAlias(String needle, String haystack) {
    const aliasGroups = <List<String>>[
      <String>['mikrotik', 'routeros'],
      <String>['ubiquiti', 'unifi', 'edgeos', 'edgerouter', 'airos'],
      <String>['cisco', 'ios', 'nxos'],
      <String>['openwrt', 'lede'],
      <String>['linux', 'rsyslog', 'debian', 'ubuntu', 'systemd'],
      <String>['pfsense', 'opnsense', 'freebsd'],
      <String>['fortinet', 'fortigate', 'fortios'],
      <String>['huawei', 'vrp'],
      <String>['juniper', 'junos'],
    ];
    for (final group in aliasGroups) {
      final needleInGroup = group.any(needle.contains);
      if (needleInGroup && group.any(haystack.contains)) {
        return true;
      }
    }
    return false;
  }

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final snippets = guide?.snippets ?? const <SetupSnippet>[];
    final autoIdx = resolveSnippetIndex(
      snippets,
      osId: operatingSystemId,
      osLabel: operatingSystemLabel,
      vendorHint: vendorHint,
    );
    final selectedIdx =
        (manualSnippetIndex != null &&
            manualSnippetIndex! >= 0 &&
            manualSnippetIndex! < snippets.length)
        ? manualSnippetIndex!
        : autoIdx;
    final activeSnippet = (selectedIdx != null && selectedIdx < snippets.length)
        ? snippets[selectedIdx]
        : null;

    return NestedPanel(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Material(
            type: MaterialType.transparency,
            child: SwitchListTile(
              contentPadding: EdgeInsets.zero,
              title: const Text('Receber logs (Syslog)'),
              subtitle: const Text(
                'Configuração automática pelo servidor ou comandos manuais por SO',
              ),
              value: enabled,
              onChanged: provisioning ? null : onToggle,
            ),
          ),
          if (enabled) ...<Widget>[
            const SizedBox(height: 8),
            SegmentedButton<SyslogSetupMode>(
              showSelectedIcon: false,
              segments: const <ButtonSegment<SyslogSetupMode>>[
                ButtonSegment<SyslogSetupMode>(
                  value: SyslogSetupMode.automatic,
                  icon: Icon(Icons.auto_fix_high, size: 16),
                  label: Text(
                    'Automático',
                    maxLines: 1,
                    softWrap: false,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                ButtonSegment<SyslogSetupMode>(
                  value: SyslogSetupMode.manual,
                  icon: Icon(Icons.terminal, size: 16),
                  label: Text(
                    'Manual',
                    maxLines: 1,
                    softWrap: false,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              ],
              selected: <SyslogSetupMode>{mode},
              onSelectionChanged: provisioning
                  ? null
                  : (sel) => onModeChanged?.call(sel.first),
            ),
            const SizedBox(height: 12),
            _SyslogTargetAddressSection(
              resolvedServerAddress: resolvedServerAddress,
              serverPort: serverPort,
              usableAddresses: usableAddresses,
              serverAddressController: serverAddressController,
              loading: loading,
              onSelectServerAddress: onSelectServerAddress,
            ),
            const SizedBox(height: 12),
            if (mode == SyslogSetupMode.automatic)
              _SyslogAutomaticSetupSection(
                isExistingDevice: isExistingDevice,
                accessMethod: accessMethod,
                existingCredential: existingCredential,
                reuseExistingAccess: reuseExistingAccess,
                sshAdminUserController: sshAdminUserController,
                sshAdminPasswordController: sshAdminPasswordController,
                sshDedicatedPortController: sshDedicatedPortController,
                oneTimeProtocol: oneTimeProtocol,
                oneTimeUserController: oneTimeUserController,
                oneTimePasswordController: oneTimePasswordController,
                oneTimePortController: oneTimePortController,
                provisioning: provisioning,
                provisionStep: provisionStep,
                provisionError: provisionError,
                provisionResult: provisionResult,
                onAccessMethodChanged: onAccessMethodChanged,
                onReuseExistingAccessChanged: onReuseExistingAccessChanged,
                onOneTimeProtocolChanged: onOneTimeProtocolChanged,
                onSshAdminUserChanged: onSshAdminUserChanged,
                onOneTimeUserChanged: onOneTimeUserChanged,
                onProvisionNow: onProvisionNow,
                onSwitchToManual: () =>
                    onModeChanged?.call(SyslogSetupMode.manual),
              )
            else
              _SyslogManualSetupSection(
                loading: loading,
                guide: guide,
                resolvedServerAddress: resolvedServerAddress,
                serverPort: serverPort,
                snippets: snippets,
                selectedIdx: selectedIdx,
                activeSnippet: activeSnippet,
                onSelectSnippetIndex: onSelectSnippetIndex,
                onRetryLoad: onRetryLoad,
                onSurfaceVariant: scheme.onSurfaceVariant,
              ),
          ],
        ],
      ),
    );
  }
}

class _SyslogTargetAddressSection extends StatelessWidget {
  const _SyslogTargetAddressSection({
    required this.resolvedServerAddress,
    required this.serverPort,
    required this.usableAddresses,
    required this.serverAddressController,
    required this.loading,
    required this.onSelectServerAddress,
  });

  final String? resolvedServerAddress;
  final int serverPort;
  final List<ServerAddressEntry> usableAddresses;
  final TextEditingController? serverAddressController;
  final bool loading;
  final ValueChanged<String?>? onSelectServerAddress;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final hasRealAddress =
        resolvedServerAddress != null && resolvedServerAddress!.isNotEmpty;
    final displayTarget = hasRealAddress
        ? '$resolvedServerAddress:$serverPort'
        : '${DeviceSyslogSetupPanel.unknownServerPlaceholder}:$serverPort';
    final uniqueAddresses = <ServerAddressEntry>[];
    final seenValues = <String>{};
    for (final entry in usableAddresses) {
      final val = entry.value?.trim() ?? '';
      if (val.isNotEmpty && seenValues.add(val)) {
        uniqueAddresses.add(entry);
      }
    }
    final dropdownMatches =
        hasRealAddress &&
        uniqueAddresses.any((e) => e.value?.trim() == resolvedServerAddress);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        KeyValueRow(
          label: 'Destino Syslog (UDP)',
          value: displayTarget,
          monospace: true,
          copyable: hasRealAddress,
        ),
        if (uniqueAddresses.length > 1) ...<Widget>[
          const SizedBox(height: 8),
          AppDropdownFormField<String>(
            key: ValueKey<String>(
              'syslog-server-addr-${dropdownMatches ? resolvedServerAddress : 'first'}-${uniqueAddresses.length}',
            ),
            initialValue: dropdownMatches
                ? resolvedServerAddress!
                : uniqueAddresses.first.value!.trim(),
            decoration: const InputDecoration(
              labelText: 'Endereço deste servidor (alvo Syslog)',
              isDense: true,
            ),
            items: <DropdownMenuItem<String>>[
              for (final entry in uniqueAddresses)
                DropdownMenuItem<String>(
                  value: entry.value!.trim(),
                  child: Text(
                    '${entry.label} (${entry.value!.trim()})',
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
            ],
            onChanged: (v) => onSelectServerAddress?.call(v),
          ),
        ] else if (!hasRealAddress &&
            !loading &&
            serverAddressController != null) ...<Widget>[
          const SizedBox(height: 8),
          TextFormField(
            controller: serverAddressController,
            decoration: const InputDecoration(
              labelText: 'Endereço deste servidor (opcional)',
              hintText: 'Ex.: 192.168.1.10',
              helperText:
                  'IP pelo qual o equipamento alcança o NetMonitor na rede.',
            ),
            onFieldSubmitted: (v) => onSelectServerAddress?.call(v),
          ),
        ],
        if (loading && !hasRealAddress) ...<Widget>[
          const SizedBox(height: 6),
          Row(
            children: <Widget>[
              const SizedBox(
                width: 12,
                height: 12,
                child: CircularProgressIndicator(strokeWidth: 2),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  'Detectando endereço do servidor Syslog...',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: context.textTheme.bodySmall?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
              ),
            ],
          ),
        ],
      ],
    );
  }
}

class _SyslogAutomaticSetupSection extends StatelessWidget {
  const _SyslogAutomaticSetupSection({
    required this.isExistingDevice,
    required this.accessMethod,
    required this.existingCredential,
    required this.reuseExistingAccess,
    required this.sshAdminUserController,
    required this.sshAdminPasswordController,
    required this.sshDedicatedPortController,
    required this.oneTimeProtocol,
    required this.oneTimeUserController,
    required this.oneTimePasswordController,
    required this.oneTimePortController,
    required this.provisioning,
    required this.provisionStep,
    required this.provisionError,
    required this.provisionResult,
    required this.onAccessMethodChanged,
    required this.onReuseExistingAccessChanged,
    required this.onOneTimeProtocolChanged,
    this.onSshAdminUserChanged,
    this.onOneTimeUserChanged,
    required this.onProvisionNow,
    required this.onSwitchToManual,
  });

  final bool isExistingDevice;
  final SyslogAccessMethod accessMethod;
  final DeviceCredentialView? existingCredential;
  final bool reuseExistingAccess;
  final TextEditingController? sshAdminUserController;
  final TextEditingController? sshAdminPasswordController;
  final TextEditingController? sshDedicatedPortController;
  final String oneTimeProtocol;
  final TextEditingController? oneTimeUserController;
  final TextEditingController? oneTimePasswordController;
  final TextEditingController? oneTimePortController;
  final bool provisioning;
  final String? provisionStep;
  final String? provisionError;
  final ProvisionLoggingResponse? provisionResult;
  final ValueChanged<SyslogAccessMethod>? onAccessMethodChanged;
  final ValueChanged<bool>? onReuseExistingAccessChanged;
  final ValueChanged<String>? onOneTimeProtocolChanged;
  final ValueChanged<String>? onSshAdminUserChanged;
  final ValueChanged<String>? onOneTimeUserChanged;
  final VoidCallback? onProvisionNow;
  final VoidCallback onSwitchToManual;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final hasReusable = existingCredential != null;
    final usingExisting = hasReusable && reuseExistingAccess;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (hasReusable) ...<Widget>[
          _SyslogExistingAccessBanner(
            credential: existingCredential!,
            reusing: usingExisting,
            busy: provisioning,
            onToggleReuse: (v) => onReuseExistingAccessChanged?.call(v),
          ),
          const SizedBox(height: 12),
        ],
        if (!usingExisting) ...<Widget>[
          Text(
            'Como liberar o acesso do NetMonitor ao equipamento?',
            style: context.textTheme.titleSmall?.copyWith(
              fontWeight: FontWeight.w700,
              color: scheme.onSurface,
            ),
          ),
          const SizedBox(height: 8),
          ChoiceCard(
            title: 'Chave SSH dedicada',
            subtitle:
                'Cria conta dedicada por chave SSH sem salvar a senha administrativa.',
            icon: Icons.vpn_key_outlined,
            tone: SemanticTone.plugin,
            badge: 'Recomendado',
            badgeTone: SemanticTone.plugin,
            selected: accessMethod == SyslogAccessMethod.dedicatedSshKey,
            enabled: !provisioning,
            onTap: () =>
                onAccessMethodChanged?.call(SyslogAccessMethod.dedicatedSshKey),
          ),
          const SizedBox(height: 8),
          ChoiceCard(
            title: 'Login e senha (uso único)',
            subtitle: 'Usado apenas nesta configuração — não fica salvo.',
            icon: Icons.password_outlined,
            tone: SemanticTone.primary,
            selected: accessMethod == SyslogAccessMethod.oneTimePassword,
            enabled: !provisioning,
            onTap: () =>
                onAccessMethodChanged?.call(SyslogAccessMethod.oneTimePassword),
          ),
          const SizedBox(height: 12),
          if (accessMethod == SyslogAccessMethod.dedicatedSshKey)
            _SyslogDedicatedSshFields(
              userController: sshAdminUserController,
              passwordController: sshAdminPasswordController,
              portController: sshDedicatedPortController,
              enabled: !provisioning,
              onUserChanged: onSshAdminUserChanged,
            )
          else
            _SyslogOneTimeLoginFields(
              protocol: oneTimeProtocol,
              userController: oneTimeUserController,
              passwordController: oneTimePasswordController,
              portController: oneTimePortController,
              enabled: !provisioning,
              onProtocolChanged: onOneTimeProtocolChanged,
              onUserChanged: onOneTimeUserChanged,
            ),
        ],
        if (provisioning && provisionStep != null) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(
            tone: SemanticTone.info,
            padding: const EdgeInsets.all(10),
            child: Row(
              children: <Widget>[
                const SizedBox(
                  width: 16,
                  height: 16,
                  child: CircularProgressIndicator(strokeWidth: 2),
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: Text(
                    provisionStep!,
                    style: context.textTheme.bodySmall?.copyWith(
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                ),
              ],
            ),
          ),
        ],
        if (provisionError != null) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(
            tone: SemanticTone.error,
            padding: const EdgeInsets.all(12),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Text(provisionError!, style: context.textTheme.bodySmall),
                const SizedBox(height: 6),
                Align(
                  alignment: AlignmentDirectional.centerStart,
                  child: TextButton.icon(
                    onPressed: provisioning ? null : onSwitchToManual,
                    style: TextButton.styleFrom(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 8,
                        vertical: 4,
                      ),
                      minimumSize: const Size(0, 32),
                      tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                    ),
                    icon: const Icon(Icons.terminal, size: 16),
                    label: const Text('Ver comandos manuais'),
                  ),
                ),
              ],
            ),
          ),
        ],
        if (provisionResult != null) ...<Widget>[
          const SizedBox(height: 12),
          _SyslogProvisionResultBanner(result: provisionResult!),
        ],
        const SizedBox(height: 12),
        if (isExistingDevice && onProvisionNow != null)
          Align(
            alignment: AlignmentDirectional.centerEnd,
            child: FilledButton.tonalIcon(
              onPressed: provisioning ? null : onProvisionNow,
              icon: provisioning
                  ? const SizedBox(
                      width: 16,
                      height: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(Icons.play_arrow_rounded, size: 18),
              label: Text(
                provisionResult != null
                    ? 'Reaplicar Syslog agora'
                    : 'Configurar Syslog agora',
              ),
            ),
          )
        else
          NestedPanel(
            tone: SemanticTone.info,
            padding: const EdgeInsets.all(10),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Icon(
                  Icons.info_outline,
                  size: 18,
                  color: context.semanticColors
                      .resolve(SemanticTone.info, scheme)
                      .onContainer,
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    'Ao cadastrar, o NetMonitor autorizará o acesso e configurará o Syslog automaticamente.',
                    style: context.textTheme.bodySmall,
                  ),
                ),
              ],
            ),
          ),
      ],
    );
  }
}

class _SyslogExistingAccessBanner extends StatelessWidget {
  const _SyslogExistingAccessBanner({
    required this.credential,
    required this.reusing,
    required this.busy,
    required this.onToggleReuse,
  });

  final DeviceCredentialView credential;
  final bool reusing;
  final bool busy;
  final ValueChanged<bool> onToggleReuse;

  @override
  Widget build(BuildContext context) {
    final isKey = credential.dedicated || credential.storage == 'key';
    final userLabel = credential.username.isNotEmpty
        ? credential.username
        : '—';
    final summary =
        '${isKey ? 'Chave SSH dedicada' : credential.kind.toUpperCase()} · '
        '$userLabel'
        '${credential.port != null ? ' (porta ${credential.port})' : ''}';

    return NestedPanel(
      tone: SemanticTone.success,
      padding: const EdgeInsets.all(12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Wrap(
            spacing: 8,
            runSpacing: 6,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: <Widget>[
              const SemanticChip(
                label: 'Acesso já configurado',
                tone: SemanticTone.success,
                compact: true,
                icon: Icons.verified_user_outlined,
              ),
              Text(
                summary,
                style: context.textTheme.bodySmall?.copyWith(
                  fontWeight: FontWeight.w600,
                ),
              ),
            ],
          ),
          const SizedBox(height: 8),
          Wrap(
            spacing: 8,
            runSpacing: 6,
            children: <Widget>[
              if (reusing)
                OutlinedButton.icon(
                  onPressed: busy ? null : () => onToggleReuse(false),
                  icon: const Icon(Icons.tune, size: 16),
                  label: const Text('Reconfigurar acesso'),
                )
              else
                FilledButton.tonalIcon(
                  onPressed: busy ? null : () => onToggleReuse(true),
                  icon: const Icon(Icons.check, size: 16),
                  label: const Text('Reusar acesso configurado'),
                ),
            ],
          ),
        ],
      ),
    );
  }
}

class _SyslogDedicatedSshFields extends StatelessWidget {
  const _SyslogDedicatedSshFields({
    required this.userController,
    required this.passwordController,
    required this.portController,
    required this.enabled,
    this.onUserChanged,
  });

  final TextEditingController? userController;
  final TextEditingController? passwordController;
  final TextEditingController? portController;
  final bool enabled;
  final ValueChanged<String>? onUserChanged;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        ResponsiveFieldRow(
          breakpoint: 480,
          children: <Widget>[
            TextFormField(
              controller: userController,
              enabled: enabled,
              onChanged: onUserChanged,
              decoration: const InputDecoration(
                labelText: 'Usuário admin atual',
                hintText: 'Ex.: root ou admin',
              ),
            ),
            TextFormField(
              controller: portController,
              enabled: enabled,
              keyboardType: TextInputType.number,
              decoration: const InputDecoration(labelText: 'Porta SSH'),
            ),
          ],
        ),
        const SizedBox(height: 10),
        TextFormField(
          controller: passwordController,
          enabled: enabled,
          obscureText: true,
          decoration: const InputDecoration(
            labelText: 'Senha admin (uso único)',
            helperText:
                'Usada uma única vez para instalar a chave SSH dedicada e descartada em seguida.',
          ),
        ),
      ],
    );
  }
}

class _SyslogOneTimeLoginFields extends StatelessWidget {
  const _SyslogOneTimeLoginFields({
    required this.protocol,
    required this.userController,
    required this.passwordController,
    required this.portController,
    required this.enabled,
    required this.onProtocolChanged,
    this.onUserChanged,
  });

  final String protocol;
  final TextEditingController? userController;
  final TextEditingController? passwordController;
  final TextEditingController? portController;
  final bool enabled;
  final ValueChanged<String>? onProtocolChanged;
  final ValueChanged<String>? onUserChanged;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        ResponsiveFieldRow(
          breakpoint: 480,
          children: <Widget>[
            AppDropdownFormField<String>(
              key: ValueKey<String>('syslog-onetime-proto-$protocol'),
              initialValue: protocol,
              decoration: const InputDecoration(labelText: 'Protocolo'),
              items: const <DropdownMenuItem<String>>[
                DropdownMenuItem<String>(
                  value: 'ssh',
                  child: Text('SSH', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem<String>(
                  value: 'telnet',
                  child: Text('Telnet', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem<String>(
                  value: 'mactelnet',
                  child: Text('MAC-Telnet', overflow: TextOverflow.ellipsis),
                ),
              ],
              onChanged: enabled
                  ? (v) {
                      if (v != null) onProtocolChanged?.call(v);
                    }
                  : null,
            ),
            TextFormField(
              controller: portController,
              enabled: enabled,
              keyboardType: TextInputType.number,
              decoration: const InputDecoration(labelText: 'Porta'),
            ),
          ],
        ),
        const SizedBox(height: 10),
        ResponsiveFieldRow(
          breakpoint: 480,
          children: <Widget>[
            TextFormField(
              controller: userController,
              enabled: enabled,
              onChanged: onUserChanged,
              decoration: const InputDecoration(
                labelText: 'Usuário',
                hintText: 'Ex.: root ou admin',
              ),
            ),
            TextFormField(
              controller: passwordController,
              enabled: enabled,
              obscureText: true,
              decoration: const InputDecoration(
                labelText: 'Senha (uso único)',
                helperText: 'Usada apenas agora e descartada.',
              ),
            ),
          ],
        ),
      ],
    );
  }
}

class _SyslogProvisionResultBanner extends StatelessWidget {
  const _SyslogProvisionResultBanner({required this.result});

  final ProvisionLoggingResponse result;

  @override
  Widget build(BuildContext context) {
    final confirmed = result.confirmed == true;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        NestedPanel(
          tone: confirmed ? SemanticTone.success : SemanticTone.info,
          padding: const EdgeInsets.all(12),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Text(
                confirmed
                    ? 'Syslog configurado e confirmado!'
                    : 'Comandos Syslog aplicados no equipamento.',
                style: context.textTheme.titleSmall?.copyWith(
                  fontWeight: FontWeight.w700,
                ),
              ),
              if (result.identifiedHostname != null &&
                  result.identifiedHostname!.isNotEmpty) ...<Widget>[
                const SizedBox(height: 4),
                Text(
                  'Identidade detectada: ${result.identifiedHostname}',
                  style: context.textTheme.bodySmall,
                ),
              ],
              if (result.persistenceWarning != null &&
                  result.persistenceWarning!.isNotEmpty) ...<Widget>[
                const SizedBox(height: 4),
                Text(
                  result.persistenceWarning!,
                  style: context.textTheme.bodySmall,
                ),
              ],
            ],
          ),
        ),
        if (result.transcript.isNotEmpty) ...<Widget>[
          const SizedBox(height: 8),
          CodeViewer(
            title: 'Transcrição do equipamento',
            code: result.transcript,
            maxHeight: 160,
          ),
        ],
      ],
    );
  }
}

class _SyslogManualSetupSection extends StatelessWidget {
  const _SyslogManualSetupSection({
    required this.loading,
    required this.guide,
    required this.resolvedServerAddress,
    required this.serverPort,
    required this.snippets,
    required this.selectedIdx,
    required this.activeSnippet,
    required this.onSelectSnippetIndex,
    required this.onRetryLoad,
    required this.onSurfaceVariant,
  });

  final bool loading;
  final SetupGuide? guide;
  final String? resolvedServerAddress;
  final int serverPort;
  final List<SetupSnippet> snippets;
  final int? selectedIdx;
  final SetupSnippet? activeSnippet;
  final ValueChanged<int?>? onSelectSnippetIndex;
  final VoidCallback? onRetryLoad;
  final Color onSurfaceVariant;

  @override
  Widget build(BuildContext context) {
    if (loading && guide == null) {
      return Row(
        children: <Widget>[
          const SizedBox(
            width: 14,
            height: 14,
            child: CircularProgressIndicator(strokeWidth: 2),
          ),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              'Carregando comandos Syslog...',
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: context.textTheme.bodySmall?.copyWith(
                color: onSurfaceVariant,
              ),
            ),
          ),
        ],
      );
    }

    if (guide == null) {
      return Wrap(
        spacing: 8,
        runSpacing: 8,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: <Widget>[
          Text(
            'Não foi possível carregar o guia Syslog do servidor.',
            style: context.textTheme.bodySmall?.copyWith(
              color: onSurfaceVariant,
            ),
          ),
          if (onRetryLoad != null)
            OutlinedButton.icon(
              onPressed: onRetryLoad,
              icon: const Icon(Icons.refresh, size: 16),
              label: const Text('Tentar novamente'),
            ),
        ],
      );
    }

    final targetDisplay =
        resolvedServerAddress ??
        guide?.serverAddress ??
        DeviceSyslogSetupPanel.unknownServerPlaceholder;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (snippets.length > 1) ...<Widget>[
          AppDropdownFormField<int>(
            key: ValueKey<String>(
              'syslog-recipe-$selectedIdx-${snippets.length}',
            ),
            initialValue: selectedIdx ?? 0,
            decoration: const InputDecoration(
              labelText: 'Receita de configuração (SO)',
            ),
            items: <DropdownMenuItem<int>>[
              for (var i = 0; i < snippets.length; i++)
                DropdownMenuItem<int>(
                  value: i,
                  child: Text(
                    snippets[i].label,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
            ],
            onChanged: (idx) => onSelectSnippetIndex?.call(idx),
          ),
          const SizedBox(height: 10),
        ],
        if (activeSnippet != null) ...<Widget>[
          CodeViewer(
            title: activeSnippet!.label,
            code: activeSnippet!.commands,
            maxHeight: 200,
          ),
          if (activeSnippet!.notes != null &&
              activeSnippet!.notes!.isNotEmpty) ...<Widget>[
            const SizedBox(height: 6),
            Text(
              activeSnippet!.notes!,
              style: context.textTheme.bodySmall?.copyWith(
                color: onSurfaceVariant,
              ),
            ),
          ],
        ] else
          Text(
            'Configure o equipamento para enviar Syslog UDP para '
            '$targetDisplay:$serverPort.',
            style: context.textTheme.bodySmall?.copyWith(
              color: onSurfaceVariant,
            ),
          ),
      ],
    );
  }
}
