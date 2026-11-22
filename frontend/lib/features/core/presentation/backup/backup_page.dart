import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/areas/composition_registries.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/file_download.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/core/models/admin_models.dart';
import 'package:netmonitor_app/features/core/providers/backup_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Tela de Backup e Restauração com composição dinâmica via `BackupTabRegistry`
/// (paridade com `BackupPage.vue`: Sistema + abas plugadas por `servers-dev`).
class BackupPage extends ConsumerStatefulWidget {
  const BackupPage({super.key, this.initialTab});

  final String? initialTab;

  @override
  ConsumerState<BackupPage> createState() => _BackupPageState();
}

class _BackupPageState extends ConsumerState<BackupPage> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) ref.read(backupProvider.notifier).fetchPlan();
    });
  }

  @override
  Widget build(BuildContext context) {
    final auth = ref.watch(authProvider);
    if (!auth.isAdmin) {
      return const EmptyStateView(
        title: 'Acesso restrito a administradores',
        description:
            'Backups e restaurações são permitidos apenas para administradores.',
        icon: Icons.lock_outline,
        tone: SemanticTone.error,
      );
    }

    final externalTabs = BackupTabRegistry.resolve(isAdmin: auth.isAdmin);
    final tabs =
        <({String id, String title, IconData icon, WidgetBuilder builder})>[
          (
            id: 'protegido',
            title: 'O que está protegido',
            icon: Icons.shield_outlined,
            builder: (_) => const _SystemBackupTab(),
          ),
          for (final ext in externalTabs)
            (
              id: ext.id,
              title: ext.title,
              icon: ext.icon,
              builder: ext.builder,
            ),
        ];

    int initialIndex = 0;
    if (widget.initialTab != null && widget.initialTab!.isNotEmpty) {
      final exact = tabs.indexWhere((t) => t.id == widget.initialTab);
      if (exact >= 0) {
        initialIndex = exact;
      } else if (widget.initialTab == 'destinos') {
        final storagesIdx = tabs.indexWhere(
          (t) => t.id == 'storages' || t.id == 'destinos',
        );
        if (storagesIdx >= 0) initialIndex = storagesIdx;
      }
    }

    return LayoutBuilder(
      builder: (context, constraints) {
        final isCompact = constraints.maxWidth < 680;
        return DefaultTabController(
          length: tabs.length,
          initialIndex: initialIndex,
          child: NestedScrollView(
            headerSliverBuilder: (context, innerBoxIsScrolled) => <Widget>[
              SliverToBoxAdapter(
                child: Padding(
                  padding: const EdgeInsets.only(bottom: 8),
                  child: SearchFilterHeader(
                    title: 'Backup e Restauração',
                    subtitle: isCompact
                        ? null
                        : 'Cópias agendadas do sistema, exportação completa e destinos de armazenamento',
                  ),
                ),
              ),
              SliverPersistentHeader(
                pinned: true,
                delegate: PinnedSliverHeaderDelegate(
                  height: 60,
                  bottomBorder: true,
                  child: TabBar(
                    isScrollable: isCompact,
                    tabAlignment: isCompact
                        ? TabAlignment.start
                        : TabAlignment.fill,
                    tabs: <Widget>[
                      for (final t in tabs)
                        Tab(icon: Icon(t.icon, size: 18), text: t.title),
                    ],
                  ),
                ),
              ),
            ],
            body: TabBarView(
              children: <Widget>[for (final t in tabs) t.builder(context)],
            ),
          ),
        );
      },
    );
  }
}

class _SystemBackupTab extends ConsumerWidget {
  const _SystemBackupTab();

  Future<void> _openPlanSheet(
    BuildContext context,
    WidgetRef ref,
    BackupState state,
  ) async {
    await showAdaptiveFormSheet<void>(
      context: context,
      title: 'Plano de backup automático',
      subtitle:
          'Escolha o destino, frequência (horas) e retenção (1 a 365 cópias).',
      builder: (_) =>
          _BackupPlanSheetBody(plan: state.plan, storages: state.storages),
    );
  }

  Future<void> _openRestoreCopySheet(
    BuildContext context,
    WidgetRef ref,
    SystemBackupPlan plan,
  ) async {
    final storageId = plan.storageDestinationId;
    if (storageId == null) return;

    await showAdaptiveFormSheet<void>(
      context: context,
      title: 'Restaurar cópia do destino',
      subtitle: plan.storageDestinationName ?? 'Destino configurado',
      maxWidth: 640,
      builder: (sheetCtx) {
        return FutureBuilder<List<SystemBackupCopy>>(
          future: ref
              .read(backupProvider.notifier)
              .listCopies(storageId: storageId),
          builder: (context, snapshot) {
            if (snapshot.connectionState == ConnectionState.waiting) {
              return const LoadingStateView(
                message: 'Listando cópias disponíveis...',
              );
            }
            final copies = snapshot.data ?? const <SystemBackupCopy>[];
            if (copies.isEmpty) {
              return const EmptyStateView(
                title: 'Nenhuma cópia encontrada',
                icon: Icons.cloud_off_outlined,
              );
            }
            return Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                for (final copy in copies) ...<Widget>[
                  ListTile(
                    contentPadding: EdgeInsets.zero,
                    leading: const Icon(Icons.inventory_2_outlined),
                    title: Text(
                      copy.name,
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(fontWeight: FontWeight.w700),
                    ),
                    subtitle: Text(
                      '${formatBytes(copy.size)} · ${formatDateTime(copy.lastModified)}',
                    ),
                    trailing: FilledButton.tonal(
                      onPressed: () async {
                        final counts = await ref
                            .read(backupProvider.notifier)
                            .previewCopy(storageId, copy.key);
                        if (!context.mounted) return;
                        final confirmed = await showConfirmActionDialog(
                          context: context,
                          title: 'Confirmar restauração da cópia?',
                          message:
                              'Serão restauradas ${formatInteger(counts.totalRows)} linhas em ${counts.tables.length} tabela(s).',
                          confirmLabel: 'Restaurar agora',
                          details: BackupCountsSummaryWidget(counts: counts),
                        );
                        if (!confirmed) return;
                        await ref
                            .read(backupProvider.notifier)
                            .restoreCopy(storageId, copy.key);
                        if (sheetCtx.mounted) {
                          Navigator.of(sheetCtx).pop();
                          ScaffoldMessenger.of(context).showSnackBar(
                            const SnackBar(
                              content: Text('Cópia restaurada com sucesso.'),
                            ),
                          );
                        }
                      },
                      child: const Text('Restaurar'),
                    ),
                  ),
                  const Divider(height: 1),
                ],
              ],
            );
          },
        );
      },
    );
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(backupProvider);
    final plan = state.plan;

    return RefreshIndicator(
      onRefresh: () => ref.read(backupProvider.notifier).fetchPlan(),
      child: ListView(
        padding: const EdgeInsets.only(top: 12),
        physics: const AlwaysScrollableScrollPhysics(),
        children: <Widget>[
          if (state.error != null) ...<Widget>[
            NestedPanel(tone: SemanticTone.error, child: Text(state.error!)),
            const SizedBox(height: 12),
          ],
          SectionCard(
            title: 'Backup automático do NetMonitor',
            subtitle: plan?.storageDestinationName != null
                ? 'Destino: ${plan!.storageDestinationName} · a cada ${plan.backupIntervalHours}h (retém ${plan.backupRetention})'
                : 'Configure um destino para ativar cópias periódicas automáticas',
            icon: Icons.backup_outlined,
            iconTone: plan?.lastBackupStatus == 'failed'
                ? SemanticTone.error
                : (plan?.backupEnabled == true
                      ? SemanticTone.success
                      : SemanticTone.info),
            trailing: SemanticChip(
              label: plan?.backupEnabled == true ? 'Ativo' : 'Desativado',
              tone: plan?.backupEnabled == true
                  ? SemanticTone.success
                  : SemanticTone.secondary,
              compact: true,
            ),
            child: state.loading && plan == null
                ? const LoadingStateView(
                    message: 'Carregando plano de backup...',
                  )
                : Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: <Widget>[
                      if (plan != null) ...<Widget>[
                        KeyValueRow(
                          label: 'Último backup',
                          value: plan.lastBackupAt != null
                              ? '${formatDateTime(plan.lastBackupAt)} (${switch (plan.lastBackupStatus) {
                                  'succeeded' || 'ok' => 'concluído',
                                  'failed' || 'error' => 'falhou',
                                  'running' => 'em andamento',
                                  _ => plan.lastBackupStatus ?? '—',
                                }})'
                              : 'Nunca executado',
                        ),
                        KeyValueRow(
                          label: 'Próximo agendado',
                          value: plan.nextBackupAt != null
                              ? formatDateTime(plan.nextBackupAt)
                              : '—',
                        ),
                        if (plan.lastBackupError != null &&
                            plan.lastBackupError!.isNotEmpty) ...<Widget>[
                          const SizedBox(height: 8),
                          NestedPanel(
                            tone: SemanticTone.error,
                            child: Text(plan.lastBackupError!),
                          ),
                        ],
                      ],
                      const SizedBox(height: 12),
                      SymmetricalActionBar(
                        spacing: 8,
                        runSpacing: 8,
                        primaryFullWidthWhenOdd: true,
                        children: <Widget>[
                          FilledButton.icon(
                            onPressed:
                                plan?.storageDestinationId == null ||
                                    state.running
                                ? null
                                : () async {
                                    try {
                                      await ref
                                          .read(backupProvider.notifier)
                                          .runNow();
                                      if (!context.mounted) return;
                                      ScaffoldMessenger.of(
                                        context,
                                      ).showSnackBar(
                                        const SnackBar(
                                          content: Text(
                                            'Backup do sistema concluído.',
                                          ),
                                        ),
                                      );
                                    } catch (err) {
                                      if (!context.mounted) return;
                                      ScaffoldMessenger.of(
                                        context,
                                      ).showSnackBar(
                                        SnackBar(
                                          content: Text(describeError(err)),
                                        ),
                                      );
                                    }
                                  },
                            icon: state.running
                                ? const SizedBox(
                                    width: 16,
                                    height: 16,
                                    child: CircularProgressIndicator(
                                      strokeWidth: 2,
                                    ),
                                  )
                                : const Icon(Icons.play_arrow, size: 18),
                            label: const Text('Fazer backup agora'),
                          ),
                          OutlinedButton.icon(
                            style: OutlinedButton.styleFrom(
                              padding: const EdgeInsets.symmetric(
                                horizontal: 10,
                                vertical: 10,
                              ),
                            ),
                            onPressed: () =>
                                _openPlanSheet(context, ref, state),
                            icon: const Icon(Icons.tune, size: 17),
                            label: const Text(
                              'Configurar plano',
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                            ),
                          ),
                          if (plan?.storageDestinationId != null)
                            OutlinedButton.icon(
                              style: OutlinedButton.styleFrom(
                                padding: const EdgeInsets.symmetric(
                                  horizontal: 10,
                                  vertical: 10,
                                ),
                              ),
                              onPressed: () =>
                                  _openRestoreCopySheet(context, ref, plan!),
                              icon: const Icon(Icons.restore, size: 17),
                              label: const Text(
                                'Restaurar cópia',
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                              ),
                            ),
                        ],
                      ),
                    ],
                  ),
          ),
          const SizedBox(height: 14),
          SectionCard(
            title: 'Exportar arquivo de backup',
            subtitle:
                'Baixe apenas configurações (.json) ou o banco completo com histórico (.json.gz)',
            icon: Icons.download_outlined,
            iconTone: SemanticTone.primary,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                SymmetricalActionBar(
                  spacing: 10,
                  runSpacing: 10,
                  threeColumnBreakpoint: 680,
                  children: <Widget>[
                    FilledButton.tonalIcon(
                      style: FilledButton.styleFrom(
                        padding: const EdgeInsets.symmetric(
                          horizontal: 10,
                          vertical: 10,
                        ),
                      ),
                      onPressed: state.exporting
                          ? null
                          : () async {
                              final res = await ref
                                  .read(backupProvider.notifier)
                                  .exportConfig();
                              if (res == null || !context.mounted) return;
                              await saveOrShareBytes(
                                bytes: res.bytes,
                                filename: res.filename,
                                mimeType: 'application/json',
                                context: context,
                              );
                            },
                      icon: const Icon(Icons.description_outlined, size: 17),
                      label: Text(
                        state.exporting ? 'Exportando...' : 'Config (.json)',
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                    OutlinedButton.icon(
                      style: OutlinedButton.styleFrom(
                        padding: const EdgeInsets.symmetric(
                          horizontal: 10,
                          vertical: 10,
                        ),
                      ),
                      onPressed: state.isDownloadingFull
                          ? null
                          : () async {
                              final res = await ref
                                  .read(backupProvider.notifier)
                                  .downloadFull();
                              if (res == null || !context.mounted) return;
                              await saveOrShareBytes(
                                bytes: res.bytes,
                                filename:
                                    res.filename ??
                                    'netmonitor-backup-completo-${formatFileTimestamp()}.json.gz',
                                mimeType: res.contentType,
                                context: context,
                              );
                            },
                      icon: const Icon(Icons.archive_outlined, size: 17),
                      label: const Text(
                        'Banco (.json.gz)',
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                  ],
                ),
                if (state.isDownloadingFull) ...<Widget>[
                  const SizedBox(height: 12),
                  NestedPanel(
                    tone: SemanticTone.info,
                    child: Row(
                      children: <Widget>[
                        const SizedBox(
                          width: 18,
                          height: 18,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        ),
                        const SizedBox(width: 10),
                        Expanded(
                          child: Text(
                            'Baixando backup completo (${formatBytes(state.fullDownloadReceivedBytes)})...',
                          ),
                        ),
                        TextButton(
                          onPressed: () => ref
                              .read(backupProvider.notifier)
                              .cancelFullDownload(),
                          child: const Text('Cancelar'),
                        ),
                      ],
                    ),
                  ),
                ],
              ],
            ),
          ),
          const SizedBox(height: 14),
          BackupFileRestoreCard(
            onRestored: (res) async {
              if (res.signInAgain) {
                await ref.read(authProvider.notifier).logout();
                if (context.mounted) context.go('/login?restaurado=1');
              } else if (context.mounted) {
                ScaffoldMessenger.of(context).showSnackBar(
                  SnackBar(
                    content: Text(
                      'Restauração concluída (${formatInteger(res.counts.totalRows)} registros).',
                    ),
                  ),
                );
              }
            },
          ),
          const SizedBox(height: 24),
        ],
      ),
    );
  }
}

/// Componente reutilizável de restauração por arquivo (usado em `BackupPage` e no Onboarding).
class BackupFileRestoreCard extends ConsumerWidget {
  const BackupFileRestoreCard({super.key, required this.onRestored});

  final ValueChanged<BackupRestoreResponse> onRestored;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(backupProvider);

    return SectionCard(
      title: 'Restaurar a partir de arquivo',
      subtitle:
          'Envio binário direto (.json ou .json.gz) com prévia obrigatória antes de aplicar',
      icon: Icons.upload_file_outlined,
      iconTone: SemanticTone.warning,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          if (state.upload != null) ...<Widget>[
            LinearProgressIndicator(value: state.upload!.fraction),
            const SizedBox(height: 6),
            Text(
              '${state.upload!.phase == 'preview' ? 'Analisando' : 'Restaurando'}: '
              '${formatBytes(state.upload!.sentBytes)} / ${formatBytes(state.upload!.totalBytes)}',
              style: context.textTheme.bodySmall,
            ),
            const SizedBox(height: 10),
          ],
          if (state.pendingCounts == null)
            FilledButton.tonalIcon(
              onPressed: state.upload != null
                  ? null
                  : () async {
                      final picked = await FilePicker.platform.pickFiles(
                        withData: true,
                        type: FileType.custom,
                        allowedExtensions: <String>['json', 'gz'],
                      );
                      final file = picked?.files.firstOrNull;
                      if (file == null || file.bytes == null) return;
                      await ref
                          .read(backupProvider.notifier)
                          .previewUploadedFile(file.bytes!, file.name);
                    },
              icon: const Icon(Icons.folder_open_outlined, size: 18),
              label: const Text('Escolher arquivo de backup'),
            )
          else ...<Widget>[
            NestedPanel(
              tone: state.pendingCounts!.isFull
                  ? SemanticTone.warning
                  : SemanticTone.info,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Text(
                    'Prévia de ${state.pendingName ?? 'arquivo'}',
                    style: const TextStyle(fontWeight: FontWeight.w700),
                  ),
                  const SizedBox(height: 6),
                  BackupCountsSummaryWidget(counts: state.pendingCounts!),
                  if (state.pendingCounts!.isFull) ...<Widget>[
                    const SizedBox(height: 8),
                    const Text(
                      'Atenção: backup completo substitui usuários, senhas e histórico. '
                      'Segredos cifrados exigem a mesma ENCRYPTION_KEY.',
                      style: TextStyle(fontWeight: FontWeight.w600),
                    ),
                  ],
                ],
              ),
            ),
            const SizedBox(height: 12),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: <Widget>[
                FilledButton.icon(
                  onPressed: state.restoring
                      ? null
                      : () async {
                          final ok = await showConfirmActionDialog(
                            context: context,
                            title: 'Aplicar restauração agora?',
                            message:
                                'Os dados atuais serão substituídos pelos registros listados na prévia.',
                            confirmLabel: 'Confirmar restauração',
                          );
                          if (!ok) return;
                          final res = await ref
                              .read(backupProvider.notifier)
                              .restorePendingFile();
                          if (res != null) onRestored(res);
                        },
                  icon: const Icon(Icons.settings_backup_restore, size: 18),
                  label: Text(
                    state.restoring
                        ? 'Restaurando...'
                        : 'Confirmar restauração',
                  ),
                ),
                TextButton(
                  onPressed: () =>
                      ref.read(backupProvider.notifier).clearPendingFile(),
                  child: const Text('Cancelar arquivo'),
                ),
              ],
            ),
          ],
        ],
      ),
    );
  }
}

class BackupCountsSummaryWidget extends StatelessWidget {
  const BackupCountsSummaryWidget({super.key, required this.counts});

  final BackupCountsResponse counts;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Wrap(
          spacing: 8,
          runSpacing: 6,
          children: <Widget>[
            SemanticChip(
              label: counts.isFull
                  ? 'Escopo: Completo'
                  : 'Escopo: Configuração',
              tone: counts.isFull ? SemanticTone.warning : SemanticTone.info,
              compact: true,
            ),
            SemanticChip(
              label: '${formatInteger(counts.totalRows)} registros',
              tone: SemanticTone.primary,
              compact: true,
            ),
          ],
        ),
        const SizedBox(height: 8),
        for (final t in counts.tables.take(10))
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 2),
            child: Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: <Widget>[
                Expanded(
                  child: Text(
                    t.table,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(
                      fontFamily: 'monospace',
                      fontSize: 12,
                    ),
                  ),
                ),
                const SizedBox(width: 8),
                Text(
                  formatInteger(t.rows),
                  style: const TextStyle(fontWeight: FontWeight.w700),
                ),
              ],
            ),
          ),
        if (counts.tables.length > 10)
          Text(
            '+ ${counts.tables.length - 10} tabela(s) adicional(is)',
            style: context.textTheme.bodySmall,
          ),
      ],
    );
  }
}

class _BackupPlanSheetBody extends ConsumerStatefulWidget {
  const _BackupPlanSheetBody({required this.plan, required this.storages});

  final SystemBackupPlan? plan;
  final List<StorageOptionRef> storages;

  @override
  ConsumerState<_BackupPlanSheetBody> createState() =>
      _BackupPlanSheetBodyState();
}

class _BackupPlanSheetBodyState extends ConsumerState<_BackupPlanSheetBody> {
  int? _storageId;
  bool _enabled = false;
  late final TextEditingController _intervalCtrl;
  late final TextEditingController _retentionCtrl;
  bool _saving = false;

  @override
  void initState() {
    super.initState();
    _storageId = widget.plan?.storageDestinationId;
    _enabled = widget.plan?.backupEnabled ?? false;
    _intervalCtrl = TextEditingController(
      text: '${widget.plan?.backupIntervalHours ?? 24}',
    );
    _retentionCtrl = TextEditingController(
      text: '${widget.plan?.backupRetention ?? 7}',
    );
  }

  @override
  void dispose() {
    _intervalCtrl.dispose();
    _retentionCtrl.dispose();
    super.dispose();
  }

  Future<void> _save() async {
    final interval = (int.tryParse(_intervalCtrl.text.trim()) ?? 24).clamp(
      1,
      720,
    );
    final retention = (int.tryParse(_retentionCtrl.text.trim()) ?? 7).clamp(
      1,
      365,
    );
    setState(() => _saving = true);
    try {
      await ref
          .read(backupProvider.notifier)
          .savePlan(
            storageDestinationId: _storageId,
            backupEnabled: _enabled,
            backupIntervalHours: interval,
            backupRetention: retention,
          );
      if (mounted) Navigator.of(context).pop();
    } catch (err) {
      if (!mounted) return;
      setState(() => _saving = false);
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(describeError(err))));
    }
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        AppDropdownFormField<int?>(
          initialValue: widget.storages.any((s) => s.id == _storageId)
              ? _storageId
              : null,
          decoration: const InputDecoration(
            labelText: 'Destino de armazenamento',
          ),
          items: <DropdownMenuItem<int?>>[
            const DropdownMenuItem<int?>(
              value: null,
              child: Text('Nenhum destino', overflow: TextOverflow.ellipsis),
            ),
            for (final s in widget.storages)
              DropdownMenuItem<int?>(
                value: s.id,
                child: Text(
                  '${s.name} (${s.kind})',
                  overflow: TextOverflow.ellipsis,
                ),
              ),
          ],
          onChanged: (v) => setState(() => _storageId = v),
        ),
        const SizedBox(height: 12),
        ResponsiveFieldRow(
          breakpoint: 480,
          children: <Widget>[
            TextField(
              controller: _intervalCtrl,
              keyboardType: TextInputType.number,
              decoration: const InputDecoration(labelText: 'Intervalo (horas)'),
            ),
            TextField(
              controller: _retentionCtrl,
              keyboardType: TextInputType.number,
              decoration: const InputDecoration(labelText: 'Retenção (1–365)'),
            ),
          ],
        ),
        const SizedBox(height: 8),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          value: _enabled,
          onChanged: (v) => setState(() => _enabled = v),
          title: const Text('Backup automático habilitado'),
        ),
        const SizedBox(height: 14),
        FilledButton.icon(
          onPressed: _saving ? null : _save,
          icon: const Icon(Icons.save_outlined),
          label: const Text('Salvar plano'),
        ),
      ],
    );
  }
}
