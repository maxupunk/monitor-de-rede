import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Seção de configurações proativas da IA (resumos automáticos de incidentes e digests).
class AiProactiveSettingsSection extends StatelessWidget {
  const AiProactiveSettingsSection({
    super.key,
    required this.proactive,
    required this.onChanged,
  });

  final AiProactiveSettings proactive;
  final ValueChanged<AiProactiveSettings> onChanged;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Wrap(
          spacing: 8,
          runSpacing: 6,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: <Widget>[
            Text(
              'IA Proativa (resumos e digests)',
              style: context.textTheme.titleSmall?.copyWith(
                fontWeight: FontWeight.w700,
              ),
            ),
            const SemanticChip(
              label: 'consome tokens',
              tone: SemanticTone.warning,
              compact: true,
            ),
          ],
        ),
        const SizedBox(height: 8),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          title: const Text('Resumo automático de incidentes'),
          subtitle: const Text(
            'Explica causa provável e próximos passos quando alertas disparam.',
          ),
          value: proactive.incidentSummaries,
          onChanged: (v) => onChanged(proactive.copyWith(incidentSummaries: v)),
        ),
        if (proactive.incidentSummaries) ...<Widget>[
          ResponsiveFieldRow(
            breakpoint: 480,
            spacing: 12,
            runSpacing: 12,
            children: <Widget>[
              AppDropdownFormField<String>(
                initialValue: proactive.incidentMinSeverity,
                decoration: const InputDecoration(
                  labelText: 'Severidade mínima',
                ),
                items: const <DropdownMenuItem<String>>[
                  DropdownMenuItem<String>(
                    value: 'critical',
                    child: Text('Crítico', overflow: TextOverflow.ellipsis),
                  ),
                  DropdownMenuItem<String>(
                    value: 'warning',
                    child: Text(
                      'Aviso ou maior',
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                ],
                onChanged: (v) {
                  if (v != null) {
                    onChanged(proactive.copyWith(incidentMinSeverity: v));
                  }
                },
              ),
              Builder(
                builder: (_) {
                  final currentMax = proactive.maxSummariesPerHour.clamp(1, 60);
                  final options = <int>{
                    2,
                    4,
                    6,
                    10,
                    20,
                    30,
                    60,
                    currentMax,
                  }.toList()..sort();
                  return AppDropdownFormField<int>(
                    initialValue: currentMax,
                    decoration: const InputDecoration(
                      labelText: 'Máx. resumos por hora',
                    ),
                    items: options
                        .map(
                          (n) => DropdownMenuItem<int>(
                            value: n,
                            child: Text(
                              '$n / hora',
                              overflow: TextOverflow.ellipsis,
                            ),
                          ),
                        )
                        .toList(growable: false),
                    onChanged: (v) {
                      if (v != null) {
                        onChanged(proactive.copyWith(maxSummariesPerHour: v));
                      }
                    },
                  );
                },
              ),
            ],
          ),
          const SizedBox(height: 10),
        ],
        ResponsiveFieldRow(
          breakpoint: 480,
          spacing: 12,
          runSpacing: 12,
          children: <Widget>[
            AppDropdownFormField<String>(
              initialValue: proactive.digest,
              decoration: const InputDecoration(
                labelText: 'Resumo periódico (Digest)',
              ),
              items: const <DropdownMenuItem<String>>[
                DropdownMenuItem<String>(
                  value: 'off',
                  child: Text('Desligado', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem<String>(
                  value: 'daily',
                  child: Text('Diário', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem<String>(
                  value: 'weekly',
                  child: Text('Semanal', overflow: TextOverflow.ellipsis),
                ),
              ],
              onChanged: (v) {
                if (v != null) onChanged(proactive.copyWith(digest: v));
              },
            ),
            if (proactive.digest != 'off')
              AppDropdownFormField<int>(
                initialValue: proactive.digestHour.clamp(0, 23),
                decoration: const InputDecoration(
                  labelText: 'Horário do servidor',
                ),
                items: List<DropdownMenuItem<int>>.generate(
                  24,
                  (h) => DropdownMenuItem<int>(
                    value: h,
                    child: Text(
                      formatHourOfDay(h),
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                ),
                onChanged: (v) {
                  if (v != null) onChanged(proactive.copyWith(digestHour: v));
                },
              ),
          ],
        ),
        if (proactive.digest != 'off')
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            title: const Text('Pular digest quando tudo estiver calmo'),
            value: proactive.skipQuietDigest,
            onChanged: (v) => onChanged(proactive.copyWith(skipQuietDigest: v)),
          ),
      ],
    );
  }
}
