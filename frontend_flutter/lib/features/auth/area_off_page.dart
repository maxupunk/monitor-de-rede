import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/areas/area_registry.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/areas/areas_provider.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/shared/widgets/nested_panel.dart';
import 'package:netmonitor_app/shared/widgets/section_card.dart';
import 'package:netmonitor_app/shared/widgets/semantic_chip.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Página exibida quando o usuário navega para uma rota de área desligada ou sem acesso.
/// Paridade com `AreaOffPage.vue`: volta automaticamente ao endereço `from` assim que
/// o SSE restaurar visibilidade/permissão.
class AreaOffPage extends ConsumerStatefulWidget {
  const AreaOffPage({
    super.key,
    required this.areaId,
    this.fromPath,
    this.reason,
  });

  final String areaId;
  final String? fromPath;
  final String? reason;

  @override
  ConsumerState<AreaOffPage> createState() => _AreaOffPageState();
}

class _AreaOffPageState extends ConsumerState<AreaOffPage> {
  String? _error;

  ProductArea get _area =>
      ProductArea.tryParse(widget.areaId) ?? ProductArea.core;
  bool get _noAccess => widget.reason == 'acesso';

  void _goBack() {
    final target = widget.fromPath;
    if (target != null &&
        target.startsWith('/') &&
        !target.startsWith('//') &&
        !target.startsWith('/area-desligada')) {
      context.go(target);
    } else {
      context.go('/');
    }
  }

  Future<void> _turnOn() async {
    setState(() => _error = null);
    try {
      await ref.read(areasProvider.notifier).setEnabled(_area, true);
    } catch (err) {
      if (!mounted) return;
      setState(() => _error = err.toString());
    }
  }

  @override
  Widget build(BuildContext context) {
    final area = _area;
    final visibleAreas = ref.watch(visibleAreasProvider);
    final areasState = ref.watch(areasProvider);
    final auth = ref.watch(authProvider);

    if (visibleAreas.contains(area)) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) _goBack();
      });
    }

    final state = areasState.stateOf(area);
    final title = state?.title ?? area.defaultTitle;
    final module = areaContributionById(area);
    final contents = (module?.nav ?? const [])
        .map((entry) => entry.item?.title)
        .whereType<String>()
        .toList(growable: false);

    return Center(
      child: SingleChildScrollView(
        padding: const EdgeInsets.symmetric(vertical: 24),
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 640),
          child: SectionCard(
            title: _noAccess
                ? 'Você não tem acesso a esta área'
                : 'Esta área está desligada',
            subtitle: _noAccess ? 'Sem acesso' : 'Área desligada',
            icon: _noAccess ? Icons.lock_outline : area.defaultIcon,
            iconTone: _noAccess ? SemanticTone.error : SemanticTone.warning,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Text.rich(
                  TextSpan(
                    children: <InlineSpan>[
                      TextSpan(
                        text: title,
                        style: const TextStyle(fontWeight: FontWeight.w700),
                      ),
                      if (state != null && state.summary.isNotEmpty)
                        TextSpan(text: ': ${state.summary}'),
                    ],
                  ),
                  style: context.textTheme.bodyLarge,
                ),
                if (state?.evidence?.present == true &&
                    state?.evidence?.reason != null &&
                    state!.evidence!.reason!.isNotEmpty &&
                    !_noAccess) ...<Widget>[
                  const SizedBox(height: 10),
                  Align(
                    alignment: Alignment.centerLeft,
                    child: SemanticChip(
                      label: state.evidence!.reason!,
                      tone: SemanticTone.info,
                      icon: Icons.info_outline,
                    ),
                  ),
                ],
                if (contents.isNotEmpty && !_noAccess) ...<Widget>[
                  const SizedBox(height: 10),
                  Text(
                    'Ela reúne ${contents.join(', ')}. Enquanto estiver desligada, '
                    'essas telas saem do menu e os cards dela não aparecem no Dashboard — '
                    'nada do que foi cadastrado é apagado.',
                    style: context.textTheme.bodyMedium?.copyWith(
                      color: context.colorScheme.onSurfaceVariant,
                    ),
                  ),
                ],
                if (_error != null) ...<Widget>[
                  const SizedBox(height: 12),
                  NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
                ],
                const SizedBox(height: 16),
                if (_noAccess) ...<Widget>[
                  const NestedPanel(
                    tone: SemanticTone.info,
                    child: Text(
                      'Seu usuário não tem papel nesta área. Peça a um administrador para '
                      'liberar o acesso em Administração › Usuários — assim que ele salvar, '
                      'esta página abre sozinha.',
                    ),
                  ),
                  const SizedBox(height: 14),
                  Align(
                    alignment: Alignment.centerLeft,
                    child: FilledButton.tonal(
                      onPressed: () => context.go('/'),
                      child: const Text('Voltar ao início'),
                    ),
                  ),
                ] else if (auth.isAdmin) ...<Widget>[
                  Wrap(
                    spacing: 10,
                    runSpacing: 10,
                    children: <Widget>[
                      FilledButton.icon(
                        onPressed: areasState.saving == area ? null : _turnOn,
                        icon: areasState.saving == area
                            ? const SizedBox(
                                width: 16,
                                height: 16,
                                child: CircularProgressIndicator(
                                  strokeWidth: 2,
                                ),
                              )
                            : const Icon(Icons.power_settings_new),
                        label: const Text('Ligar área'),
                      ),
                      OutlinedButton.icon(
                        onPressed: () => context.go('/settings?tab=geral'),
                        icon: const Icon(Icons.settings_outlined, size: 18),
                        label: const Text('Ir para Configurações'),
                      ),
                      FilledButton.tonal(
                        onPressed: () => context.go('/'),
                        child: const Text('Voltar ao início'),
                      ),
                    ],
                  ),
                ] else ...<Widget>[
                  const NestedPanel(
                    tone: SemanticTone.info,
                    child: Text(
                      'Só um administrador pode ligar a área. Peça a ele em '
                      'Configurações › Geral › Áreas do sistema.',
                    ),
                  ),
                  const SizedBox(height: 14),
                  Align(
                    alignment: Alignment.centerLeft,
                    child: FilledButton.tonal(
                      onPressed: () => context.go('/'),
                      child: const Text('Voltar ao início'),
                    ),
                  ),
                ],
              ],
            ),
          ),
        ),
      ),
    );
  }
}
