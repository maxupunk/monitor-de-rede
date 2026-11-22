import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
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
  bool _testing = false;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) ref.read(coreSettingsProvider.notifier).fetchPushStatus();
    });
  }

  @override
  Widget build(BuildContext context) {
    final settings = ref.watch(coreSettingsProvider);
    final push = settings.pushStatus;

    return SectionCard(
      title: 'Notificações Push e Alertas Locais',
      subtitle:
          'Receba alertas críticos, quedas de monitor e falhas de backup em tempo real',
      icon: Icons.notifications_active_outlined,
      iconTone: push?.configured == true
          ? SemanticTone.success
          : SemanticTone.info,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            value: settings.localNotificationsEnabled,
            onChanged: (v) => ref
                .read(coreSettingsProvider.notifier)
                .setLocalNotificationsEnabled(v),
            title: const Text('Ativar notificações neste dispositivo'),
            subtitle: const Text(
              'Dispara avisos para alert:triggered, monitor down, device offline e backup:failed.',
            ),
          ),
          const Divider(height: 16),
          KeyValueRow(
            label: 'Servidor Web Push (VAPID)',
            value: push == null
                ? 'Consultando...'
                : (push.configured ? 'Configurado' : 'Não configurado'),
          ),
          KeyValueRow(
            label: 'Assinaturas ativas',
            value: formatInteger(push?.subscriptionCount ?? 0),
          ),
          const SizedBox(height: 12),
          Align(
            alignment: Alignment.centerLeft,
            child: FilledButton.tonalIcon(
              onPressed: _testing
                  ? null
                  : () async {
                      setState(() => _testing = true);
                      final msg = await ref
                          .read(coreSettingsProvider.notifier)
                          .sendTestPush();
                      if (!context.mounted) return;
                      setState(() => _testing = false);
                      ScaffoldMessenger.of(
                        context,
                      ).showSnackBar(SnackBar(content: Text(msg)));
                    },
              icon: _testing
                  ? const SizedBox(
                      width: 16,
                      height: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(Icons.send_outlined, size: 18),
              label: const Text('Enviar notificação de teste'),
            ),
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
