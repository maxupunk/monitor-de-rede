import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/core/models/log_models.dart';
import 'package:netmonitor_app/features/core/providers/logs_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

Future<void> showLogDetailSheet(
  BuildContext context, {
  required WidgetRef ref,
  required LogEntry entry,
  LogTemplateInfo? template,
}) async {
  await showAdaptiveFormSheet<void>(
    context: context,
    title: entry.deviceName ?? entry.hostname ?? entry.sourceIp,
    subtitle: formatDateTime(entry.receivedAt),
    maxWidth: 640,
    builder: (sheetCtx) => Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Wrap(
          spacing: 6,
          runSpacing: 6,
          children: <Widget>[
            SemanticChip(
              label: (entry.severityLabel ?? 'SEV ${entry.severity ?? '-'}')
                  .toUpperCase(),
              tone: entry.severityTone,
              compact: true,
            ),
            if (entry.appName != null && entry.appName!.isNotEmpty)
              SemanticChip(
                label: entry.appName!,
                tone: SemanticTone.secondary,
                compact: true,
              ),
            if (template != null)
              SemanticChip(
                label: logCategoryLabel(template.category),
                tone: template.confirmed
                    ? SemanticTone.primary
                    : SemanticTone.info,
                compact: true,
                onTap: () {
                  Navigator.of(sheetCtx).pop();
                  showLogTemplateSheet(
                    context,
                    ref: ref,
                    entry: entry,
                    info: template,
                  );
                },
              ),
          ],
        ),
        const SizedBox(height: 12),
        KeyValueRow(
          label: 'IP de origem',
          value: entry.sourceIp,
          monospace: true,
          copyable: true,
        ),
        if (entry.hostname != null && entry.hostname!.isNotEmpty)
          KeyValueRow(
            label: 'Hostname',
            value: entry.hostname!,
            monospace: true,
          ),
        if (entry.facility != null)
          KeyValueRow(label: 'Facility', value: '${entry.facility}'),
        if (entry.topics.isNotEmpty)
          KeyValueRow(label: 'Tópicos', value: entry.topics.join(', ')),
        const SizedBox(height: 12),
        CodeViewer(title: 'Mensagem Syslog', code: entry.message),
        if (template != null) ...<Widget>[
          const SizedBox(height: 12),
          OutlinedButton.icon(
            onPressed: () {
              Navigator.of(sheetCtx).pop();
              showLogTemplateSheet(
                context,
                ref: ref,
                entry: entry,
                info: template,
              );
            },
            icon: const Icon(Icons.label_outline, size: 18),
            label: const Text('Classificar padrão do log'),
          ),
        ],
      ],
    ),
  );
}

Future<void> showLogTemplateSheet(
  BuildContext context, {
  required WidgetRef ref,
  required LogEntry entry,
  required LogTemplateInfo info,
}) async {
  await showAdaptiveFormSheet<void>(
    context: context,
    title: 'Classificação do padrão de log',
    subtitle: info.confirmed
        ? 'Confirmada pelo operador'
        : 'Sugestão automática do Decisor',
    builder: (_) =>
        _LogTemplateSheetBody(entry: entry, info: info, parentContext: context),
  );
}

class _LogTemplateSheetBody extends ConsumerStatefulWidget {
  const _LogTemplateSheetBody({
    required this.entry,
    required this.info,
    required this.parentContext,
  });

  final LogEntry entry;
  final LogTemplateInfo info;
  final BuildContext parentContext;

  @override
  ConsumerState<_LogTemplateSheetBody> createState() =>
      _LogTemplateSheetBodyState();
}

class _LogTemplateSheetBodyState extends ConsumerState<_LogTemplateSheetBody> {
  late String _selectedCategory;
  bool _saving = false;

  @override
  void initState() {
    super.initState();
    _selectedCategory = widget.info.category ?? 'system';
  }

  @override
  Widget build(BuildContext context) {
    final auth = ref.watch(authProvider);
    final info = widget.info;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        CodeViewer(title: 'Padrão', code: info.template),
        const SizedBox(height: 12),
        AppDropdownFormField<String>(
          initialValue: kLogCategories.any((c) => c.$1 == _selectedCategory)
              ? _selectedCategory
              : 'system',
          decoration: const InputDecoration(labelText: 'Categoria'),
          items: <DropdownMenuItem<String>>[
            for (final (k, label) in kLogCategories)
              DropdownMenuItem<String>(
                value: k,
                child: Text(label, overflow: TextOverflow.ellipsis),
              ),
          ],
          onChanged: auth.canWrite
              ? (v) {
                  if (v != null) {
                    setState(() => _selectedCategory = v);
                  }
                }
              : null,
        ),
        const SizedBox(height: 14),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: <Widget>[
            if (auth.canWrite)
              FilledButton.icon(
                onPressed: _saving
                    ? null
                    : () async {
                        setState(() => _saving = true);
                        final err = await ref
                            .read(logsProvider.notifier)
                            .confirmTemplate(
                              info.templateHash,
                              _selectedCategory,
                            );
                        if (!context.mounted) return;
                        Navigator.of(context).pop();
                        if (widget.parentContext.mounted) {
                          ScaffoldMessenger.of(
                            widget.parentContext,
                          ).showSnackBar(
                            SnackBar(
                              content: Text(err ?? 'Categoria confirmada.'),
                            ),
                          );
                        }
                      },
                icon: const Icon(Icons.check, size: 18),
                label: const Text('Confirmar categoria'),
              ),
            if (info.alertTemplate != null && info.alertTemplate!.isNotEmpty)
              OutlinedButton.icon(
                onPressed: () {
                  Navigator.of(context).pop();
                  if (widget.parentContext.mounted) {
                    widget.parentContext.go(
                      '/alerts?tab=rules&catalog=${info.alertTemplate}',
                    );
                  }
                },
                icon: const Icon(Icons.notifications_active_outlined, size: 18),
                label: const Text('Criar regra de alerta'),
              ),
          ],
        ),
      ],
    );
  }
}
