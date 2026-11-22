import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/core/models/admin_models.dart';
import 'package:netmonitor_app/features/core/providers/audit_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Trilha de Auditoria (paridade com `AuditPage.vue`, somente administradores).
class AuditPage extends ConsumerStatefulWidget {
  const AuditPage({super.key});

  @override
  ConsumerState<AuditPage> createState() => _AuditPageState();
}

class _AuditPageState extends ConsumerState<AuditPage> {
  final TextEditingController _searchCtrl = TextEditingController();

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) ref.read(auditProvider.notifier).fetchPage(page: 1);
    });
  }

  @override
  void dispose() {
    _searchCtrl.dispose();
    super.dispose();
  }

  SemanticTone _actionTone(String action) => switch (action) {
    'create' => SemanticTone.success,
    'update' => SemanticTone.warning,
    'delete' || 'execute' => SemanticTone.error,
    'login' => SemanticTone.primary,
    _ => SemanticTone.secondary,
  };

  int _activeFilterCount(AuditFilters f) =>
      (f.action != null ? 1 : 0) + (f.resourceType != null ? 1 : 0);

  void _clearAllFilters() {
    _searchCtrl.clear();
    ref.read(auditProvider.notifier).clearFilters();
  }

  Future<void> _openDetail(AuditLogEntry entry) async {
    await showAdaptiveFormSheet<void>(
      context: context,
      title:
          '${auditActionLabel(entry.action)} · ${auditResourceLabel(entry.resourceType)}',
      subtitle: formatDateTime(entry.createdAt),
      maxWidth: 640,
      builder: (_) => Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          KeyValueRow(label: 'Usuário', value: entry.userEmail ?? 'Sistema'),
          KeyValueRow(label: 'Ação', value: auditActionLabel(entry.action)),
          KeyValueRow(
            label: 'Recurso',
            value:
                '${auditResourceLabel(entry.resourceType)}${entry.resourceLabel != null ? ' — ${entry.resourceLabel}' : ''}',
          ),
          if (entry.ipAddress != null)
            KeyValueRow(
              label: 'IP de origem',
              value: entry.ipAddress!,
              monospace: true,
            ),
          if (entry.userAgent != null && entry.userAgent!.isNotEmpty)
            KeyValueRow(label: 'User-Agent', value: entry.userAgent!),
          if (entry.description != null && entry.description!.isNotEmpty)
            KeyValueRow(label: 'Descrição', value: entry.description!),
          if (entry.changes != null) ...<Widget>[
            const SizedBox(height: 12),
            CodeViewer(
              title: 'Alterações registradas',
              code: const JsonEncoder.withIndent('  ').convert(entry.changes),
              maxHeight: 320,
            ),
          ],
        ],
      ),
    );
  }

  List<ActiveFilterChipData> _buildActiveChips(AuditState state) {
    final f = state.filters;
    final chips = <ActiveFilterChipData>[];
    if (f.action != null) {
      chips.add(
        ActiveFilterChipData(
          icon: Icons.bolt_outlined,
          label: auditActionLabel(f.action!),
          onDeleted: () => ref
              .read(auditProvider.notifier)
              .applyFilters(f.copyWith(clearAction: true)),
        ),
      );
    }
    if (f.resourceType != null) {
      chips.add(
        ActiveFilterChipData(
          icon: Icons.category_outlined,
          label: auditResourceLabel(f.resourceType!),
          onDeleted: () => ref
              .read(auditProvider.notifier)
              .applyFilters(f.copyWith(clearResource: true)),
        ),
      );
    }
    return chips;
  }

  @override
  Widget build(BuildContext context) {
    final auth = ref.watch(authProvider);
    final state = ref.watch(auditProvider);
    final scheme = context.colorScheme;

    if (!auth.isAdmin) {
      return const EmptyStateView(
        title: 'Acesso restrito a administradores',
        icon: Icons.lock_outline,
        tone: SemanticTone.error,
      );
    }

    final activeCount = _activeFilterCount(state.filters);

    return LayoutBuilder(
      builder: (context, constraints) {
        final isCompact = constraints.maxWidth < 720;

        return RefreshIndicator(
          onRefresh: () => ref
              .read(auditProvider.notifier)
              .fetchPage(page: state.meta?.currentPage ?? 1),
          child: CustomScrollView(
            physics: const AlwaysScrollableScrollPhysics(),
            slivers: <Widget>[
              SliverToBoxAdapter(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    if (isCompact)
                      Row(
                        children: <Widget>[
                          Expanded(
                            child: Text(
                              'Trilha de Auditoria',
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: context.textTheme.headlineSmall?.copyWith(
                                fontWeight: FontWeight.w800,
                                color: scheme.onSurface,
                              ),
                            ),
                          ),
                          if (state.meta != null)
                            SemanticChip(
                              label: formatInteger(state.meta!.total),
                              tone: SemanticTone.secondary,
                              compact: true,
                            ),
                        ],
                      )
                    else
                      Wrap(
                        spacing: 12,
                        runSpacing: 10,
                        alignment: WrapAlignment.spaceBetween,
                        crossAxisAlignment: WrapCrossAlignment.center,
                        children: <Widget>[
                          Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: <Widget>[
                              Text(
                                'Trilha de Auditoria',
                                style: context.textTheme.headlineSmall
                                    ?.copyWith(
                                      fontWeight: FontWeight.w800,
                                      color: scheme.onSurface,
                                    ),
                              ),
                              const SizedBox(height: 2),
                              Text(
                                state.meta != null
                                    ? '${formatInteger(state.meta!.total)} registros · página ${state.meta!.currentPage} de ${state.meta!.lastPage}'
                                    : 'Registro completo de alterações, logins e execuções',
                                style: context.textTheme.bodyMedium?.copyWith(
                                  color: scheme.onSurfaceVariant,
                                ),
                              ),
                            ],
                          ),
                          if (activeCount > 0 || _searchCtrl.text.isNotEmpty)
                            TextButton.icon(
                              onPressed: _clearAllFilters,
                              icon: const Icon(
                                Icons.filter_alt_off_outlined,
                                size: 18,
                              ),
                              label: const Text('Limpar filtros'),
                            ),
                        ],
                      ),
                    const SizedBox(height: 10),
                    UnifiedFilterBar(
                      searchController: _searchCtrl,
                      searchHint: 'Buscar descrição, recurso ou e-mail...',
                      onSearchChanged: (v) => ref
                          .read(auditProvider.notifier)
                          .applyFilters(state.filters.copyWith(search: v)),
                      activeFilterCount: activeCount,
                      filterSheetTitle: 'Filtrar auditoria',
                      filterSheetSubtitle:
                          'Filtre registros por tipo de ação ou recurso afetado.',
                      filterSheetBuilder: (sheetCtx, setSheetState) {
                        final current = ref.read(auditProvider).filters;
                        return ResponsiveFieldRow(
                          breakpoint: 480,
                          children: <Widget>[
                            AppDropdownFormField<String?>(
                              initialValue:
                                  kAuditActionOptions.any(
                                    (a) => a.$1 == current.action,
                                  )
                                  ? current.action
                                  : null,
                              decoration: const InputDecoration(
                                labelText: 'Ação',
                              ),
                              items: <DropdownMenuItem<String?>>[
                                const DropdownMenuItem<String?>(
                                  value: null,
                                  child: Text(
                                    'Todas as ações',
                                    overflow: TextOverflow.ellipsis,
                                  ),
                                ),
                                for (final (v, label) in kAuditActionOptions)
                                  DropdownMenuItem<String?>(
                                    value: v,
                                    child: Text(
                                      label,
                                      overflow: TextOverflow.ellipsis,
                                    ),
                                  ),
                              ],
                              onChanged: (v) {
                                ref
                                    .read(auditProvider.notifier)
                                    .applyFilters(
                                      v == null
                                          ? current.copyWith(clearAction: true)
                                          : current.copyWith(action: v),
                                    );
                                setSheetState(() {});
                              },
                            ),
                            AppDropdownFormField<String?>(
                              initialValue:
                                  kAuditResourceOptions.any(
                                    (r) => r.$1 == current.resourceType,
                                  )
                                  ? current.resourceType
                                  : null,
                              decoration: const InputDecoration(
                                labelText: 'Recurso',
                              ),
                              items: <DropdownMenuItem<String?>>[
                                const DropdownMenuItem<String?>(
                                  value: null,
                                  child: Text(
                                    'Todos os recursos',
                                    overflow: TextOverflow.ellipsis,
                                  ),
                                ),
                                for (final (v, label) in kAuditResourceOptions)
                                  DropdownMenuItem<String?>(
                                    value: v,
                                    child: Text(
                                      label,
                                      overflow: TextOverflow.ellipsis,
                                    ),
                                  ),
                              ],
                              onChanged: (v) {
                                ref
                                    .read(auditProvider.notifier)
                                    .applyFilters(
                                      v == null
                                          ? current.copyWith(
                                              clearResource: true,
                                            )
                                          : current.copyWith(resourceType: v),
                                    );
                                setSheetState(() {});
                              },
                            ),
                          ],
                        );
                      },
                      onClearAll: _clearAllFilters,
                      activeChips: _buildActiveChips(state),
                    ),
                    const SizedBox(height: 12),
                  ],
                ),
              ),
              if (state.loading && state.entries.isEmpty)
                const SliverFillRemaining(
                  hasScrollBody: false,
                  child: LoadingStateView(
                    message: 'Carregando trilha de auditoria...',
                  ),
                )
              else if (state.error != null && state.entries.isEmpty)
                SliverFillRemaining(
                  hasScrollBody: false,
                  child: ErrorStateView(
                    message: state.error!,
                    onRetry: () =>
                        ref.read(auditProvider.notifier).fetchPage(page: 1),
                  ),
                )
              else if (state.entries.isEmpty)
                const SliverFillRemaining(
                  hasScrollBody: false,
                  child: EmptyStateView(
                    title: 'Nenhum registro encontrado',
                    description:
                        'Ajuste os filtros para consultar outros períodos ou recursos.',
                    icon: Icons.verified_user_outlined,
                  ),
                )
              else ...<Widget>[
                SliverList.separated(
                  itemCount: state.entries.length,
                  separatorBuilder: (_, _) => const SizedBox(height: 8),
                  itemBuilder: (context, index) {
                    final entry = state.entries[index];
                    final scheme = context.colorScheme;
                    return Card(
                      elevation: 1,
                      shape: RoundedRectangleBorder(
                        borderRadius: AppTheme.borderRadius,
                      ),
                      clipBehavior: Clip.antiAlias,
                      child: InkWell(
                        onTap: () => _openDetail(entry),
                        child: Padding(
                          padding: const EdgeInsets.symmetric(
                            horizontal: 14,
                            vertical: 12,
                          ),
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: <Widget>[
                              Row(
                                children: <Widget>[
                                  Expanded(
                                    child: Row(
                                      children: <Widget>[
                                        SemanticChip(
                                          label: auditActionLabel(entry.action),
                                          tone: _actionTone(entry.action),
                                          compact: true,
                                        ),
                                        const SizedBox(width: 6),
                                        Flexible(
                                          child: SemanticChip(
                                            label: auditResourceLabel(
                                              entry.resourceType,
                                            ),
                                            tone: SemanticTone.secondary,
                                            compact: true,
                                          ),
                                        ),
                                      ],
                                    ),
                                  ),
                                  const SizedBox(width: 8),
                                  Text(
                                    formatShortDateTime(entry.createdAt),
                                    style: context.textTheme.labelSmall
                                        ?.copyWith(
                                          fontFamily: 'monospace',
                                          fontWeight: FontWeight.w600,
                                          color: scheme.onSurfaceVariant,
                                        ),
                                  ),
                                ],
                              ),
                              if (entry.resourceLabel != null &&
                                  entry.resourceLabel!.isNotEmpty) ...<Widget>[
                                const SizedBox(height: 8),
                                Text(
                                  entry.resourceLabel!,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: context.textTheme.titleSmall?.copyWith(
                                    fontWeight: FontWeight.w700,
                                  ),
                                ),
                              ],
                              const SizedBox(height: 4),
                              Text(
                                '${entry.userEmail ?? 'Sistema'}'
                                '${entry.ipAddress != null ? ' · ${entry.ipAddress}' : ''}',
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: context.textTheme.bodySmall?.copyWith(
                                  color: scheme.onSurfaceVariant,
                                ),
                              ),
                              if (entry.description != null &&
                                  entry.description!.isNotEmpty) ...<Widget>[
                                const SizedBox(height: 4),
                                Text(
                                  entry.description!,
                                  maxLines: 2,
                                  overflow: TextOverflow.ellipsis,
                                  style: context.textTheme.bodySmall,
                                ),
                              ],
                            ],
                          ),
                        ),
                      ),
                    );
                  },
                ),
                if (state.meta != null && state.meta!.lastPage > 1)
                  SliverToBoxAdapter(
                    child: Padding(
                      padding: const EdgeInsets.symmetric(vertical: 12),
                      child: Wrap(
                        alignment: WrapAlignment.center,
                        crossAxisAlignment: WrapCrossAlignment.center,
                        spacing: 10,
                        runSpacing: 8,
                        children: <Widget>[
                          OutlinedButton.icon(
                            onPressed:
                                state.meta!.currentPage > 1 && !state.loading
                                ? () => ref
                                      .read(auditProvider.notifier)
                                      .fetchPage(
                                        page: state.meta!.currentPage - 1,
                                      )
                                : null,
                            icon: const Icon(Icons.chevron_left),
                            label: const Text('Anterior'),
                          ),
                          Text(
                            '${state.meta!.currentPage} / ${state.meta!.lastPage}',
                            style: const TextStyle(fontWeight: FontWeight.w700),
                          ),
                          OutlinedButton.icon(
                            onPressed: state.meta!.hasNext && !state.loading
                                ? () => ref
                                      .read(auditProvider.notifier)
                                      .fetchPage(
                                        page: state.meta!.currentPage + 1,
                                      )
                                : null,
                            icon: const Icon(Icons.chevron_right),
                            label: const Text('Próxima'),
                          ),
                        ],
                      ),
                    ),
                  ),
                const SliverToBoxAdapter(child: SizedBox(height: 24)),
              ],
            ],
          ),
        );
      },
    );
  }
}
