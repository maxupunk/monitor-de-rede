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

  @override
  Widget build(BuildContext context) {
    final auth = ref.watch(authProvider);
    final state = ref.watch(auditProvider);

    if (!auth.isAdmin) {
      return const EmptyStateView(
        title: 'Acesso restrito a administradores',
        icon: Icons.lock_outline,
        tone: SemanticTone.error,
      );
    }

    return RefreshIndicator(
      onRefresh: () => ref
          .read(auditProvider.notifier)
          .fetchPage(page: state.meta?.currentPage ?? 1),
      child: ListView(
        children: <Widget>[
          SearchFilterHeader(
            title: 'Trilha de Auditoria',
            subtitle: state.meta != null
                ? '${formatInteger(state.meta!.total)} registros · página ${state.meta!.currentPage} de ${state.meta!.lastPage}'
                : 'Registro completo de alterações, logins e execuções',
            searchController: _searchCtrl,
            searchHint: 'Buscar descrição, recurso ou e-mail...',
            onSearchChanged: (v) => ref
                .read(auditProvider.notifier)
                .applyFilters(state.filters.copyWith(search: v)),
            actions: <Widget>[
              TextButton.icon(
                onPressed: () {
                  _searchCtrl.clear();
                  ref.read(auditProvider.notifier).clearFilters();
                },
                icon: const Icon(Icons.filter_alt_off_outlined, size: 18),
                label: const Text('Limpar filtros'),
              ),
            ],
            filters: <Widget>[
              DropdownButton<String?>(
                value: state.filters.action,
                hint: const Text('Ação'),
                underline: const SizedBox.shrink(),
                items: <DropdownMenuItem<String?>>[
                  const DropdownMenuItem<String?>(
                    value: null,
                    child: Text('Todas as ações'),
                  ),
                  for (final (v, label) in kAuditActionOptions)
                    DropdownMenuItem<String?>(value: v, child: Text(label)),
                ],
                onChanged: (v) => ref
                    .read(auditProvider.notifier)
                    .applyFilters(
                      v == null
                          ? state.filters.copyWith(clearAction: true)
                          : state.filters.copyWith(action: v),
                    ),
              ),
              DropdownButton<String?>(
                value: state.filters.resourceType,
                hint: const Text('Recurso'),
                underline: const SizedBox.shrink(),
                items: <DropdownMenuItem<String?>>[
                  const DropdownMenuItem<String?>(
                    value: null,
                    child: Text('Todos os recursos'),
                  ),
                  for (final (v, label) in kAuditResourceOptions)
                    DropdownMenuItem<String?>(value: v, child: Text(label)),
                ],
                onChanged: (v) => ref
                    .read(auditProvider.notifier)
                    .applyFilters(
                      v == null
                          ? state.filters.copyWith(clearResource: true)
                          : state.filters.copyWith(resourceType: v),
                    ),
              ),
            ],
          ),
          const SizedBox(height: 14),
          if (state.loading && state.entries.isEmpty)
            const LoadingStateView(message: 'Carregando trilha de auditoria...')
          else if (state.error != null && state.entries.isEmpty)
            ErrorStateView(
              message: state.error!,
              onRetry: () =>
                  ref.read(auditProvider.notifier).fetchPage(page: 1),
            )
          else if (state.entries.isEmpty)
            const EmptyStateView(
              title: 'Nenhum registro encontrado',
              description: 'Ajuste os filtros para consultar outros períodos ou recursos.',
              icon: Icons.verified_user_outlined,
            )
          else ...<Widget>[
            for (final entry in state.entries) ...<Widget>[
              Card(
                elevation: 1,
                shape: RoundedRectangleBorder(
                  borderRadius: AppTheme.borderRadius,
                ),
                child: ListTile(
                  onTap: () => _openDetail(entry),
                  title: Wrap(
                    spacing: 6,
                    runSpacing: 4,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: <Widget>[
                      SemanticChip(
                        label: auditActionLabel(entry.action),
                        tone: _actionTone(entry.action),
                        compact: true,
                      ),
                      SemanticChip(
                        label: auditResourceLabel(entry.resourceType),
                        tone: SemanticTone.secondary,
                        compact: true,
                      ),
                      if (entry.resourceLabel != null)
                        Text(
                          entry.resourceLabel!,
                          style: const TextStyle(fontWeight: FontWeight.w700),
                        ),
                    ],
                  ),
                  subtitle: Padding(
                    padding: const EdgeInsets.only(top: 4),
                    child: Text(
                      '${entry.userEmail ?? 'Sistema'}'
                      '${entry.ipAddress != null ? ' · ${entry.ipAddress}' : ''}'
                      '${entry.description != null ? '\n${entry.description}' : ''}',
                      maxLines: 3,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  trailing: Text(
                    formatShortDateTime(entry.createdAt),
                    style: context.textTheme.labelSmall,
                  ),
                ),
              ),
              const SizedBox(height: 8),
            ],
            if (state.meta != null && state.meta!.lastPage > 1)
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 12),
                child: Row(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: <Widget>[
                    OutlinedButton.icon(
                      onPressed: state.meta!.currentPage > 1 && !state.loading
                          ? () => ref
                                .read(auditProvider.notifier)
                                .fetchPage(page: state.meta!.currentPage - 1)
                          : null,
                      icon: const Icon(Icons.chevron_left),
                      label: const Text('Anterior'),
                    ),
                    Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 14),
                      child: Text(
                        '${state.meta!.currentPage} / ${state.meta!.lastPage}',
                        style: const TextStyle(fontWeight: FontWeight.w700),
                      ),
                    ),
                    OutlinedButton.icon(
                      onPressed: state.meta!.hasNext && !state.loading
                          ? () => ref
                                .read(auditProvider.notifier)
                                .fetchPage(page: state.meta!.currentPage + 1)
                          : null,
                      icon: const Icon(Icons.chevron_right),
                      label: const Text('Próxima'),
                    ),
                  ],
                ),
              ),
          ],
        ],
      ),
    );
  }
}
