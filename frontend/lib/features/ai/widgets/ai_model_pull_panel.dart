import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Barra de progresso padronizada para streams SSE-over-POST de download de modelos LLM
/// (reutilizada entre Ollama em `AiSettingsCard` e Ollaya/Decider em `DeciderSettingsCard`).
class AiModelPullProgressBar extends StatelessWidget {
  const AiModelPullProgressBar({super.key, required this.progress});

  final ModelPullProgress progress;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final hasBytes = progress.completed != null && progress.total != null;
    final statusLine =
        progress.error ??
        '${progress.status}${hasBytes ? ' · ${formatBytes(progress.completed)} / ${formatBytes(progress.total)}' : ''}';

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        LinearProgressIndicator(
          value: progress.percentage != null
              ? (progress.percentage! / 100).clamp(0.0, 1.0)
              : null,
        ),
        const SizedBox(height: 4),
        Text(
          statusLine,
          style: context.textTheme.labelSmall?.copyWith(
            color: progress.error != null
                ? scheme.error
                : scheme.onSurfaceVariant,
          ),
        ),
      ],
    );
  }
}

/// Painel completo de download/pull de modelo LLM com entrada de nome e cancelamento.
class AiModelPullPanel extends StatelessWidget {
  const AiModelPullPanel({
    super.key,
    required this.title,
    required this.controller,
    required this.pulling,
    required this.onPull,
    required this.onCancel,
    this.progress,
    this.hintText = 'Ex.: qwen2.5:7b-instruct',
  });

  final String title;
  final TextEditingController controller;
  final bool pulling;
  final VoidCallback onPull;
  final VoidCallback onCancel;
  final ModelPullProgress? progress;
  final String hintText;

  @override
  Widget build(BuildContext context) {
    return NestedPanel(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Text(
            title,
            style: context.textTheme.titleSmall?.copyWith(
              fontWeight: FontWeight.w700,
            ),
          ),
          const SizedBox(height: 8),
          LayoutBuilder(
            builder: (context, constraints) {
              final field = TextField(
                controller: controller,
                enabled: !pulling,
                decoration: InputDecoration(hintText: hintText, isDense: true),
              );
              final actionBtn = pulling
                  ? OutlinedButton.icon(
                      onPressed: onCancel,
                      icon: const Icon(Icons.close, size: 18),
                      label: const Text('Cancelar'),
                    )
                  : FilledButton.tonalIcon(
                      onPressed: onPull,
                      icon: const Icon(Icons.download, size: 18),
                      label: const Text('Baixar'),
                    );
              if (constraints.maxWidth < 400) {
                return Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    field,
                    const SizedBox(height: 8),
                    Align(alignment: Alignment.centerRight, child: actionBtn),
                  ],
                );
              }
              return Row(
                children: <Widget>[
                  Expanded(child: field),
                  const SizedBox(width: 8),
                  actionBtn,
                ],
              );
            },
          ),
          if (progress != null) ...<Widget>[
            const SizedBox(height: 10),
            AiModelPullProgressBar(progress: progress!),
          ],
        ],
      ),
    );
  }
}
