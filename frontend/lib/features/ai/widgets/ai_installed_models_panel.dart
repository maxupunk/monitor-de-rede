import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';
import 'package:netmonitor_app/features/ai/providers/ai_providers.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Os modelos baixados no Ollama, com o espaço de cada um e a ação de
/// apagar os que não são mais usados. Fica logo abaixo de "Baixar modelo no
/// Ollama": quem baixa é quem limpa, no mesmo lugar.
///
/// O modelo do chat aparece como "Em uso" e não tem botão de apagar (o
/// servidor também recusa); só o administrador apaga.
class AiInstalledModelsPanel extends ConsumerWidget {
  const AiInstalledModelsPanel({super.key, required this.baseUrl});

  /// A URL do Ollama digitada na tela, lida na hora da ação.
  final ValueGetter<String> baseUrl;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(aiSettingsProvider);
    final catalog = state.ollamaCatalog;
    if (catalog == null || !catalog.isOnline) return const SizedBox.shrink();

    final isAdmin = ref.watch(authProvider).isAdmin;
    final models = _sorted(catalog.installedModels);
    final scheme = context.colorScheme;

    return NestedPanel(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Wrap(
            spacing: 8,
            runSpacing: 6,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: <Widget>[
              Text(
                'Modelos baixados no servidor',
                style: context.textTheme.titleSmall?.copyWith(
                  fontWeight: FontWeight.w700,
                ),
              ),
              if (models.isNotEmpty)
                SemanticChip(
                  label:
                      '${models.length} ${models.length == 1 ? 'modelo' : 'modelos'} · '
                      '${formatBytes(catalog.installedDiskBytes, fractionDigits: 1)} no disco',
                  tone: SemanticTone.info,
                  icon: Icons.storage_outlined,
                  compact: true,
                ),
            ],
          ),
          const SizedBox(height: 4),
          Text(
            isAdmin
                ? 'Apague os modelos que não usa mais para liberar espaço no '
                      'disco do servidor. O modelo em uso pelo Assistente fica '
                      'protegido: troque e salve outro modelo antes de apagá-lo.'
                : 'Apenas administradores podem apagar modelos.',
            style: context.textTheme.bodySmall?.copyWith(
              color: scheme.onSurfaceVariant,
            ),
          ),
          const SizedBox(height: 8),
          if (models.isEmpty)
            Text(
              'Nenhum modelo baixado neste Ollama.',
              style: context.textTheme.bodyMedium,
            )
          else
            for (final model in models)
              _InstalledModelTile(
                model: model,
                canDelete: isAdmin,
                deleting: state.deletingModel == model.id,
                busy: state.deletingModel != null,
                onDelete: () => _confirmAndDelete(context, ref, model),
              ),
        ],
      ),
    );
  }

  /// O modelo em uso primeiro; depois os mais antigos — os candidatos a sair.
  static List<AiProviderModelItem> _sorted(List<AiProviderModelItem> models) {
    final sorted = List<AiProviderModelItem>.of(models);
    sorted.sort((a, b) {
      if (a.inUse != b.inUse) return a.inUse ? -1 : 1;
      final aDate = a.modifiedAt;
      final bDate = b.modifiedAt;
      if (aDate == null || bDate == null) {
        return (aDate == null ? 1 : 0) - (bDate == null ? 1 : 0);
      }
      return aDate.compareTo(bDate);
    });
    return sorted;
  }

  Future<void> _confirmAndDelete(
    BuildContext context,
    WidgetRef ref,
    AiProviderModelItem model,
  ) async {
    final shared = model.sharedWith.isNotEmpty;
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: 'Apagar o modelo ${model.name}?',
      message: shared
          ? 'Este nome usa o mesmo arquivo de ${model.sharedWith.join(', ')}. '
                'Apagar remove só o nome e não libera espaço no disco.'
          : 'O modelo sai do Ollama e libera '
                '${formatBytes(model.sizeBytes, fractionDigits: 1, fallback: 'o espaço que ocupa')} '
                'no disco do servidor. Para usá-lo de novo, será preciso baixá-lo outra vez.',
      confirmLabel: 'Apagar modelo',
      icon: Icons.delete_outline,
    );
    if (!confirmed || !context.mounted) return;

    final result = await ref
        .read(aiSettingsProvider.notifier)
        .deleteOllamaModel(model.id, baseUrl: baseUrl());
    if (!context.mounted) return;
    final removal = result.removal;
    final message = removal == null
        ? result.error ?? 'Falha ao apagar o modelo.'
        : removal.freedBytes > 0
        ? 'Modelo ${removal.name} apagado: '
              '${formatBytes(removal.freedBytes, fractionDigits: 1)} liberados.'
        : 'Nome ${removal.name} removido; o arquivo continua em uso.';
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(SnackBar(content: Text(message)));
  }
}

class _InstalledModelTile extends StatelessWidget {
  const _InstalledModelTile({
    required this.model,
    required this.canDelete,
    required this.deleting,
    required this.busy,
    required this.onDelete,
  });

  final AiProviderModelItem model;
  final bool canDelete;
  final bool deleting;
  final bool busy;
  final VoidCallback onDelete;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final details = <String>[
      formatBytes(model.sizeBytes, fractionDigits: 1, fallback: 'tamanho ?'),
      if (model.modifiedAt != null)
        'baixado ${formatRelativeTime(model.modifiedAt)}',
      if (model.sharedWith.isNotEmpty)
        'mesmo arquivo de ${model.sharedWith.join(', ')}',
    ];

    final Widget trailing;
    if (model.inUse) {
      trailing = const SemanticChip(
        label: 'Em uso',
        tone: SemanticTone.success,
        icon: Icons.check_circle_outline,
        compact: true,
      );
    } else if (deleting) {
      trailing = const SizedBox(
        width: 40,
        height: 40,
        child: Padding(
          padding: EdgeInsets.all(10),
          child: CircularProgressIndicator(strokeWidth: 2),
        ),
      );
    } else if (canDelete) {
      trailing = IconButton(
        tooltip: 'Apagar modelo',
        color: scheme.error,
        icon: const Icon(Icons.delete_outline),
        onPressed: busy ? null : onDelete,
      );
    } else {
      trailing = const SizedBox.shrink();
    }

    return ListTile(
      contentPadding: EdgeInsets.zero,
      dense: true,
      leading: Icon(Icons.memory_outlined, color: scheme.primary),
      title: Text(
        model.name,
        overflow: TextOverflow.ellipsis,
        style: context.textTheme.bodyMedium?.copyWith(
          fontWeight: FontWeight.w600,
        ),
      ),
      subtitle: Text(details.join(' · ')),
      trailing: trailing,
    );
  }
}
