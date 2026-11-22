import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Abre o catálogo filtrável de modelos LLM de um provedor com descarte seguro do controlador.
Future<String?> showAiModelCatalogSheet({
  required BuildContext context,
  required String driver,
  required List<AiProviderModelItem> items,
}) {
  return showAdaptiveFormSheet<String>(
    context: context,
    title: 'Selecionar modelo ($driver)',
    maxWidth: 640,
    builder: (_) => _AiModelCatalogSheet(items: items),
  );
}

class _AiModelCatalogSheet extends StatefulWidget {
  const _AiModelCatalogSheet({required this.items});

  final List<AiProviderModelItem> items;

  @override
  State<_AiModelCatalogSheet> createState() => _AiModelCatalogSheetState();
}

class _AiModelCatalogSheetState extends State<_AiModelCatalogSheet> {
  final TextEditingController _searchCtrl = TextEditingController();
  String _filter = 'all';

  @override
  void dispose() {
    _searchCtrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final q = _searchCtrl.text.trim().toLowerCase();
    final filtered = widget.items
        .where((m) {
          if (_filter == 'free' && !m.isFree) return false;
          if (_filter == 'tools' && !m.supportsTools) return false;
          if (_filter == 'installed' && !m.installed) return false;
          if (_filter == 'recommended' && !m.recommended) return false;
          if (q.isNotEmpty) {
            return m.id.toLowerCase().contains(q) ||
                m.name.toLowerCase().contains(q) ||
                (m.description?.toLowerCase().contains(q) ?? false);
          }
          return true;
        })
        .take(40)
        .toList(growable: false);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        TextField(
          controller: _searchCtrl,
          onChanged: (_) => setState(() {}),
          decoration: const InputDecoration(
            hintText: 'Buscar modelo por nome ou slug...',
            prefixIcon: Icon(Icons.search, size: 18),
            isDense: true,
          ),
        ),
        const SizedBox(height: 8),
        Wrap(
          spacing: 6,
          runSpacing: 6,
          children: <Widget>[
            for (final entry in const <(String, String)>[
              ('all', 'Todos'),
              ('free', 'Gratuitos'),
              ('tools', 'Com ferramentas'),
              ('installed', 'Instalados'),
              ('recommended', 'Recomendados'),
            ])
              ChoiceChip(
                label: Text(entry.$2),
                selected: _filter == entry.$1,
                onSelected: (_) => setState(() => _filter = entry.$1),
              ),
          ],
        ),
        const SizedBox(height: 10),
        if (filtered.isEmpty)
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 20),
            child: Column(
              children: <Widget>[
                const Text('Nenhum modelo encontrado na lista.'),
                if (q.isNotEmpty) ...<Widget>[
                  const SizedBox(height: 8),
                  FilledButton.tonal(
                    onPressed: () =>
                        Navigator.of(context).pop(_searchCtrl.text.trim()),
                    child: Text('Usar "${_searchCtrl.text.trim()}"'),
                  ),
                ],
              ],
            ),
          )
        else
          ConstrainedBox(
            constraints: const BoxConstraints(maxHeight: 340),
            child: ListView.separated(
              shrinkWrap: true,
              itemCount: filtered.length,
              separatorBuilder: (_, _) => const Divider(height: 1),
              itemBuilder: (context, index) {
                final m = filtered[index];
                return ListTile(
                  dense: true,
                  title: Text(
                    m.name,
                    style: const TextStyle(fontWeight: FontWeight.w600),
                  ),
                  subtitle: Text(
                    <String>[
                      m.id,
                      if (m.contextLength != null)
                        'ctx ${formatContextWindow(m.contextLength)}',
                      if (m.sizeBytes != null) formatBytes(m.sizeBytes),
                    ].join(' · '),
                  ),
                  trailing: Wrap(
                    spacing: 4,
                    children: <Widget>[
                      if (m.installed)
                        const SemanticChip(
                          label: 'Instalado',
                          tone: SemanticTone.success,
                          compact: true,
                        ),
                      if (m.recommended)
                        const SemanticChip(
                          label: 'Recomendado',
                          tone: SemanticTone.ai,
                          compact: true,
                        ),
                    ],
                  ),
                  onTap: () => Navigator.of(context).pop(m.id),
                );
              },
            ),
          ),
      ],
    );
  }
}
