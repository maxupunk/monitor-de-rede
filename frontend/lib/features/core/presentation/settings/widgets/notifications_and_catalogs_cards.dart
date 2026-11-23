import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/notifications/notifications_service.dart';
import 'package:netmonitor_app/core/notifications/web_notifications_interop.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/core/models/settings_models.dart';
import 'package:netmonitor_app/features/core/providers/backup_provider.dart';
import 'package:netmonitor_app/features/core/providers/settings_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

// 6. NotificationsCard (Web Push + preferências locais)
class NotificationsCard extends ConsumerStatefulWidget {
  const NotificationsCard({super.key});

  @override
  ConsumerState<NotificationsCard> createState() => _NotificationsCardState();
}

class _NotificationsCardState extends ConsumerState<NotificationsCard> {
  bool _testingPush = false;
  bool _requestingPermission = false;
  String? _feedbackText;
  SemanticTone _feedbackTone = SemanticTone.info;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) {
        ref.read(notificationsProvider.notifier).refreshStatus();
      }
    });
  }

  void _setFeedback(String message, SemanticTone tone) {
    setState(() {
      _feedbackText = message;
      _feedbackTone = tone;
    });
  }

  Future<void> _handleRequestPermission() async {
    setState(() {
      _requestingPermission = true;
      _feedbackText = null;
    });
    final granted = await ref
        .read(notificationsProvider.notifier)
        .requestPermission();
    if (!mounted) return;
    setState(() => _requestingPermission = false);
    final current = ref.read(notificationsProvider);
    if (granted) {
      _setFeedback(
        current.isSubscribed
            ? 'Permissão concedida e dispositivo inscrito para notificações Web Push!'
            : 'Permissão concedida! Notificações locais habilitadas.',
        SemanticTone.success,
      );
    } else if (current.permissionState == NotificationPermissionState.denied) {
      _setFeedback(
        'Permissão bloqueada pelo navegador. Libere as notificações nas configurações do site.',
        SemanticTone.error,
      );
    } else {
      _setFeedback(
        'A permissão de notificações não foi concedida.',
        SemanticTone.warning,
      );
    }
  }

  Future<void> _handleToggleWebPush(bool enable) async {
    setState(() => _feedbackText = null);
    final ok = await ref
        .read(notificationsProvider.notifier)
        .toggleWebPush(enable);
    if (!mounted) return;
    if (ok) {
      _setFeedback(
        enable
            ? 'Dispositivo inscrito com sucesso para notificações Web Push!'
            : 'Inscrição Web Push desativada neste dispositivo.',
        SemanticTone.success,
      );
    } else {
      final err = ref.read(notificationsProvider).error;
      _setFeedback(
        err ?? 'Não foi possível alterar a inscrição de Web Push no navegador.',
        SemanticTone.error,
      );
    }
  }

  Future<void> _handleSendTestPush() async {
    setState(() {
      _testingPush = true;
      _feedbackText = null;
    });
    try {
      final res = await ref.read(notificationsProvider.notifier).sendTestPush();
      if (!mounted) return;
      setState(() => _testingPush = false);
      _setFeedback(
        res.message,
        res.success ? SemanticTone.success : SemanticTone.warning,
      );
    } catch (err) {
      if (!mounted) return;
      setState(() => _testingPush = false);
      _setFeedback(describeError(err), SemanticTone.error);
    }
  }

  Future<void> _handleLocalTest() async {
    final shown = await ref
        .read(notificationsProvider.notifier)
        .sendLocalNotification(
          'Notificação de Teste PWA',
          body:
              'Este é um teste de funcionamento das notificações em tempo real do NetMonitor.',
          tag: 'netmonitor-local-test',
          url: '/settings',
        );
    if (!mounted) return;
    if (shown) {
      _setFeedback(
        'Notificação local disparada na aba aberta.',
        SemanticTone.success,
      );
    } else {
      _setFeedback(
        'Ative as notificações visuais na aba aberta e garanta a permissão concedida.',
        SemanticTone.warning,
      );
    }
  }

  (String subtitle, String chipLabel, SemanticTone tone) _permissionMeta(
    NotificationPermissionState perm,
  ) {
    return switch (perm) {
      NotificationPermissionState.granted => (
        'Permissão concedida',
        'PERMITIDO',
        SemanticTone.success,
      ),
      NotificationPermissionState.denied => (
        'Bloqueado no navegador',
        'BLOQUEADO',
        SemanticTone.error,
      ),
      NotificationPermissionState.unsupported => (
        'Não suportado',
        'INDISPONÍVEL',
        SemanticTone.secondary,
      ),
      NotificationPermissionState.defaultState => (
        'Ainda não solicitado',
        'PENDENTE',
        SemanticTone.warning,
      ),
    };
  }

  (String subtitle, String chipLabel, SemanticTone tone) _webPushMeta(
    NotificationsState notifs,
  ) {
    final countLabel =
        '${formatInteger(notifs.subscriptionCount)} assinatura(s) ativa(s)';
    if (!notifs.isWebPushSupported) {
      return (
        'Não suportado neste navegador/contexto',
        'INDISPONÍVEL',
        SemanticTone.secondary,
      );
    }
    if (notifs.isSubscribed) {
      return (
        'Dispositivo inscrito · $countLabel',
        'ATIVO',
        SemanticTone.success,
      );
    }
    return (
      'Não inscrito neste dispositivo · $countLabel',
      'INATIVO',
      SemanticTone.warning,
    );
  }

  @override
  Widget build(BuildContext context) {
    final notifs = ref.watch(notificationsProvider);
    final (permSubtitle, permChip, permTone) = _permissionMeta(
      notifs.permissionState,
    );
    final (pushSubtitle, pushChip, pushTone) = _webPushMeta(notifs);
    final isGranted =
        notifs.permissionState == NotificationPermissionState.granted;

    return SectionCard(
      title: 'Notificações PWA & Web Push',
      subtitle:
          'Configure alertas críticos, quedas de monitor e falhas de backup mesmo com o navegador ou aplicativo fechados',
      icon: Icons.notifications_active_outlined,
      iconTone: notifs.isSubscribed
          ? SemanticTone.success
          : (notifs.permissionState == NotificationPermissionState.denied
                ? SemanticTone.error
                : SemanticTone.warning),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          ResponsiveFieldRow(
            breakpoint: 560,
            spacing: 12,
            runSpacing: 10,
            children: <Widget>[
              NestedPanel(
                padding: const EdgeInsets.all(12),
                child: Row(
                  children: <Widget>[
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          Text(
                            'Permissão do Navegador',
                            style: context.textTheme.titleSmall?.copyWith(
                              fontWeight: FontWeight.w700,
                            ),
                          ),
                          const SizedBox(height: 2),
                          Text(
                            permSubtitle,
                            style: context.textTheme.bodySmall,
                          ),
                        ],
                      ),
                    ),
                    const SizedBox(width: 8),
                    SemanticChip(
                      label: permChip,
                      tone: permTone,
                      compact: true,
                    ),
                  ],
                ),
              ),
              NestedPanel(
                padding: const EdgeInsets.all(12),
                child: Row(
                  children: <Widget>[
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          Text(
                            'Web Push em 2º Plano',
                            style: context.textTheme.titleSmall?.copyWith(
                              fontWeight: FontWeight.w700,
                            ),
                          ),
                          const SizedBox(height: 2),
                          Text(
                            pushSubtitle,
                            style: context.textTheme.bodySmall,
                          ),
                        ],
                      ),
                    ),
                    const SizedBox(width: 8),
                    SemanticChip(
                      label: pushChip,
                      tone: pushTone,
                      compact: true,
                    ),
                  ],
                ),
              ),
            ],
          ),
          if (!notifs.isWebPushSupported) ...<Widget>[
            const SizedBox(height: 10),
            NestedPanel(
              tone: SemanticTone.info,
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Icon(
                    Icons.lock_outline,
                    size: 18,
                    color: context.semanticColors
                        .resolve(SemanticTone.info, context.colorScheme)
                        .onContainer,
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Text(
                      'Web Push em segundo plano exige acesso via HTTPS (contexto seguro) ou localhost em um navegador compatível com Service Worker e PushManager.',
                      style: context.textTheme.bodySmall?.copyWith(
                        fontWeight: FontWeight.w600,
                        color: context.semanticColors
                            .resolve(SemanticTone.info, context.colorScheme)
                            .onContainer,
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ],
          const SizedBox(height: 10),
          if (notifs.isWebPushSupported)
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              value: notifs.isSubscribed,
              onChanged:
                  notifs.isSubscribing ||
                      notifs.permissionState ==
                          NotificationPermissionState.denied
                  ? null
                  : _handleToggleWebPush,
              title: const Text('Receber alertas em segundo plano (Web Push)'),
              subtitle: Text(
                notifs.isSubscribing
                    ? 'Atualizando assinatura Web Push no servidor...'
                    : 'Entrega alertas críticos mesmo com a aba ou o aplicativo fechados.',
              ),
            ),
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            value: notifs.localNotificationsEnabled,
            onChanged: isGranted
                ? (v) => ref
                      .read(notificationsProvider.notifier)
                      .setLocalNotificationsEnabled(v)
                : null,
            title: const Text('Notificações visuais na aba aberta'),
            subtitle: const Text(
              'Dispara avisos em tempo real via SSE para alert:triggered, monitor down, device offline e backup:failed.',
            ),
          ),
          if (_feedbackText != null) ...<Widget>[
            const SizedBox(height: 8),
            NestedPanel(
              tone: _feedbackTone,
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
              child: Row(
                children: <Widget>[
                  Expanded(
                    child: Text(
                      _feedbackText!,
                      style: context.textTheme.bodySmall?.copyWith(
                        fontWeight: FontWeight.w600,
                        color: context.semanticColors
                            .resolve(_feedbackTone, context.colorScheme)
                            .onContainer,
                      ),
                    ),
                  ),
                  IconButton(
                    tooltip: 'Fechar aviso',
                    visualDensity: VisualDensity.compact,
                    icon: Icon(
                      Icons.close,
                      size: 16,
                      color: context.semanticColors
                          .resolve(_feedbackTone, context.colorScheme)
                          .onContainer,
                    ),
                    onPressed: () => setState(() => _feedbackText = null),
                  ),
                ],
              ),
            ),
          ],
          const SizedBox(height: 12),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: <Widget>[
              if (!isGranted)
                FilledButton.icon(
                  onPressed:
                      notifs.permissionState ==
                              NotificationPermissionState.unsupported ||
                          _requestingPermission
                      ? null
                      : _handleRequestPermission,
                  icon: _requestingPermission
                      ? const SizedBox(
                          width: 16,
                          height: 16,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        )
                      : const Icon(Icons.notifications_active, size: 18),
                  label: const Text('Solicitar permissão'),
                )
              else ...<Widget>[
                FilledButton.icon(
                  onPressed: !notifs.isSubscribed || _testingPush
                      ? null
                      : _handleSendTestPush,
                  icon: _testingPush
                      ? const SizedBox(
                          width: 16,
                          height: 16,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        )
                      : const Icon(Icons.send_outlined, size: 18),
                  label: const Text('Testar Web Push (2º plano)'),
                ),
                FilledButton.tonalIcon(
                  onPressed: _handleLocalTest,
                  icon: const Icon(Icons.notifications_outlined, size: 18),
                  label: const Text('Teste local (aba aberta)'),
                ),
              ],
            ],
          ),
        ],
      ),
    );
  }
}

// 7. BackupShortcutCard
class BackupShortcutCard extends ConsumerStatefulWidget {
  const BackupShortcutCard({super.key});

  @override
  ConsumerState<BackupShortcutCard> createState() => _BackupShortcutCardState();
}

class _BackupShortcutCardState extends ConsumerState<BackupShortcutCard> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && ref.read(authProvider).isAdmin) {
        ref.read(backupProvider.notifier).fetchPlan();
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final auth = ref.watch(authProvider);
    final plan = ref.watch(backupProvider).plan;
    if (!auth.isAdmin) return const SizedBox.shrink();

    return SectionCard(
      title: 'Backup e Restauração',
      subtitle: plan?.backupEnabled == true
          ? 'Backup automático ativo (${plan!.storageDestinationName ?? 'destino'})'
          : 'Gerencie cópias automáticas, bancos de dados e destinos de armazenamento',
      icon: Icons.settings_backup_restore,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          KeyValueRow(
            label: 'Último backup do sistema',
            value: plan?.lastBackupAt != null
                ? formatDateTime(plan!.lastBackupAt)
                : 'Nenhum registro',
          ),
          const SizedBox(height: 12),
          Align(
            alignment: Alignment.centerLeft,
            child: FilledButton.tonalIcon(
              onPressed: () => context.go('/backup'),
              icon: const Icon(Icons.open_in_new, size: 18),
              label: const Text('Abrir Backup'),
            ),
          ),
        ],
      ),
    );
  }
}

// 8. CoreCatalogsCard (Perfis SNMP e Sistemas Operacionais — base MAC OUI vive em NetworkArea)
class CoreCatalogsCard extends ConsumerStatefulWidget {
  const CoreCatalogsCard({super.key});

  @override
  ConsumerState<CoreCatalogsCard> createState() => _CoreCatalogsCardState();
}

class _CoreCatalogsCardState extends ConsumerState<CoreCatalogsCard> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      final n = ref.read(coreSettingsProvider.notifier);
      n.fetchSnmpProfiles();
      n.fetchOperatingSystems();
    });
  }

  Future<void> _openCreateSnmpProfile() async {
    await showAdaptiveFormSheet<void>(
      context: context,
      title: 'Novo perfil SNMP',
      subtitle:
          'Cadastre OIDs personalizados para leitura automática de sensores.',
      builder: (_) => const _CreateSnmpProfileSheetBody(),
    );
  }

  @override
  Widget build(BuildContext context) {
    final settings = ref.watch(coreSettingsProvider);
    final auth = ref.watch(authProvider);

    return SectionCard(
      title: 'Catálogos do Núcleo (SNMP e Sistemas)',
      subtitle: 'Perfis SNMP reutilizáveis e sistemas operacionais suportados',
      icon: Icons.library_books_outlined,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Wrap(
            spacing: 8,
            runSpacing: 6,
            alignment: WrapAlignment.spaceBetween,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: <Widget>[
              Text(
                'Perfis SNMP (${settings.snmpProfiles.length})',
                style: context.textTheme.titleSmall?.copyWith(
                  fontWeight: FontWeight.w700,
                ),
              ),
              if (auth.canWrite)
                TextButton.icon(
                  onPressed: _openCreateSnmpProfile,
                  icon: const Icon(Icons.add, size: 18),
                  label: const Text('Novo perfil'),
                ),
            ],
          ),
          for (final p in settings.snmpProfiles.take(6))
            ListTile(
              dense: true,
              contentPadding: EdgeInsets.zero,
              title: Text(
                p.name,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(fontWeight: FontWeight.w600),
              ),
              subtitle: Text('${p.vendor} · ${p.sensors.length} sensor(es)'),
              trailing: !p.isBuiltin && auth.canWrite
                  ? IconButton(
                      icon: Icon(
                        Icons.delete_outline,
                        color: context.colorScheme.error,
                      ),
                      onPressed: () => ref
                          .read(coreSettingsProvider.notifier)
                          .deleteSnmpProfile(p.id),
                    )
                  : const SemanticChip(
                      label: 'Nativo',
                      tone: SemanticTone.secondary,
                      compact: true,
                    ),
            ),
          const Divider(height: 20),
          Text(
            'Sistemas operacionais cadastrados (${settings.operatingSystems.length})',
            style: context.textTheme.titleSmall?.copyWith(
              fontWeight: FontWeight.w700,
            ),
          ),
          const SizedBox(height: 8),
          Wrap(
            spacing: 6,
            runSpacing: 6,
            children: <Widget>[
              for (final os in settings.operatingSystems)
                SemanticChip(
                  label: os.label,
                  tone: os.supportsSyslog
                      ? SemanticTone.info
                      : SemanticTone.secondary,
                  compact: true,
                ),
            ],
          ),
        ],
      ),
    );
  }
}

class _CreateSnmpProfileSheetBody extends ConsumerStatefulWidget {
  const _CreateSnmpProfileSheetBody();

  @override
  ConsumerState<_CreateSnmpProfileSheetBody> createState() =>
      _CreateSnmpProfileSheetBodyState();
}

class _CreateSnmpProfileSheetBodyState
    extends ConsumerState<_CreateSnmpProfileSheetBody> {
  final TextEditingController _idCtrl = TextEditingController();
  final TextEditingController _nameCtrl = TextEditingController();
  final TextEditingController _vendorCtrl = TextEditingController();
  final TextEditingController _catCtrl = TextEditingController(text: 'router');
  final TextEditingController _oidCtrl = TextEditingController();
  final TextEditingController _sensorKeyCtrl = TextEditingController(
    text: 'temp',
  );
  final TextEditingController _sensorLabelCtrl = TextEditingController(
    text: 'Temperatura',
  );
  final TextEditingController _sensorOidCtrl = TextEditingController();
  bool _saving = false;

  @override
  void dispose() {
    _idCtrl.dispose();
    _nameCtrl.dispose();
    _vendorCtrl.dispose();
    _catCtrl.dispose();
    _oidCtrl.dispose();
    _sensorKeyCtrl.dispose();
    _sensorLabelCtrl.dispose();
    _sensorOidCtrl.dispose();
    super.dispose();
  }

  Future<void> _save() async {
    if (_idCtrl.text.trim().isEmpty || _nameCtrl.text.trim().isEmpty) {
      return;
    }
    setState(() => _saving = true);
    final profile = SnmpDeviceProfile(
      id: _idCtrl.text.trim(),
      name: _nameCtrl.text.trim(),
      vendor: _vendorCtrl.text.trim(),
      category: _catCtrl.text.trim(),
      sysObjectIdPrefix: _oidCtrl.text.trim().isEmpty
          ? null
          : _oidCtrl.text.trim(),
      sensors: _sensorOidCtrl.text.trim().isEmpty
          ? const <SnmpProfileSensor>[]
          : <SnmpProfileSensor>[
              SnmpProfileSensor(
                key: _sensorKeyCtrl.text.trim(),
                label: _sensorLabelCtrl.text.trim(),
                oid: _sensorOidCtrl.text.trim(),
              ),
            ],
    );
    final ok = await ref
        .read(coreSettingsProvider.notifier)
        .saveSnmpProfile(profile);
    if (!mounted) return;
    if (ok) {
      Navigator.of(context).pop();
    } else {
      setState(() => _saving = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        TextField(
          controller: _idCtrl,
          decoration: const InputDecoration(labelText: 'ID único'),
        ),
        const SizedBox(height: 10),
        TextField(
          controller: _nameCtrl,
          decoration: const InputDecoration(labelText: 'Nome'),
        ),
        const SizedBox(height: 10),
        ResponsiveFieldRow(
          breakpoint: 480,
          spacing: 8,
          runSpacing: 10,
          children: <Widget>[
            TextField(
              controller: _vendorCtrl,
              decoration: const InputDecoration(labelText: 'Fabricante'),
            ),
            TextField(
              controller: _catCtrl,
              decoration: const InputDecoration(labelText: 'Categoria'),
            ),
          ],
        ),
        const SizedBox(height: 10),
        TextField(
          controller: _oidCtrl,
          decoration: const InputDecoration(
            labelText: 'Prefixo sysObjectID (opcional)',
          ),
        ),
        const SizedBox(height: 10),
        TextField(
          controller: _sensorLabelCtrl,
          decoration: const InputDecoration(
            labelText: 'Sensor inicial — Rótulo',
          ),
        ),
        const SizedBox(height: 10),
        TextField(
          controller: _sensorOidCtrl,
          style: const TextStyle(fontFamily: 'monospace'),
          decoration: const InputDecoration(labelText: 'Sensor inicial — OID'),
        ),
        const SizedBox(height: 14),
        FilledButton(
          onPressed: _saving ? null : _save,
          child: const Text('Salvar perfil SNMP'),
        ),
      ],
    );
  }
}
