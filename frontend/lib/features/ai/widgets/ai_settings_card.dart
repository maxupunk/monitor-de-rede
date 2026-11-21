import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';
import 'package:netmonitor_app/features/ai/providers/ai_providers.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

const List<int> _ollamaCtxSteps = <int>[
  8192,
  16384,
  32768,
  65536,
  131072,
  262144,
];

/// Cartão de configurações gerais da IA na aba `ia` de `/settings`.
class AiSettingsCard extends ConsumerStatefulWidget {
  const AiSettingsCard({super.key});

  @override
  ConsumerState<AiSettingsCard> createState() => _AiSettingsCardState();
}

class _AiSettingsCardState extends ConsumerState<AiSettingsCard> {
  AiSettings _form = const AiSettings();
  bool _hydrated = false;
  bool _showApiKey = false;

  final _ollamaUrlCtrl = TextEditingController();
  final _ollamaModelCtrl = TextEditingController();
  final _pullModelCtrl = TextEditingController();
  final _openRouterKeyCtrl = TextEditingController();
  final _openRouterModelCtrl = TextEditingController();
  final _openCodeKeyCtrl = TextEditingController();
  final _openCodeModelCtrl = TextEditingController();
  final _customPromptCtrl = TextEditingController();

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      await ref.read(aiSettingsProvider.notifier).load();
      if (mounted) _hydrate();
    });
  }

  @override
  void dispose() {
    _ollamaUrlCtrl.dispose();
    _ollamaModelCtrl.dispose();
    _pullModelCtrl.dispose();
    _openRouterKeyCtrl.dispose();
    _openRouterModelCtrl.dispose();
    _openCodeKeyCtrl.dispose();
    _openCodeModelCtrl.dispose();
    _customPromptCtrl.dispose();
    super.dispose();
  }

  void _hydrate() {
    final loaded = ref.read(aiSettingsProvider).settings;
    if (loaded == null) return;
    _form = loaded;
    _ollamaUrlCtrl.text = loaded.ollamaBaseUrl ?? 'http://ollama:11434/v1';
    _ollamaModelCtrl.text = loaded.ollamaModel ?? '';
    _pullModelCtrl.text = loaded.ollamaModel ?? '';
    _openRouterKeyCtrl.text = loaded.openrouterApiKey ?? '';
    _openRouterModelCtrl.text = loaded.openrouterModel ?? 'openrouter/free';
    _openCodeKeyCtrl.text = loaded.opencodeApiKey ?? '';
    _openCodeModelCtrl.text =
        loaded.opencodeModel ?? 'muse-spark-1.3-contributor-free';
    _customPromptCtrl.text = loaded.customSystemPrompt ?? '';
    setState(() => _hydrated = true);
    _loadModelsForDriver(loaded.activeDriver);
  }

  void _loadModelsForDriver(String driver) {
    final notifier = ref.read(aiSettingsProvider.notifier);
    if (driver == 'ollama') {
      notifier.loadOllamaModels(baseUrl: _ollamaUrlCtrl.text);
    } else if (driver == 'openrouter') {
      notifier.loadOpenRouterModels(apiKey: _openRouterKeyCtrl.text);
    } else if (driver == 'opencode') {
      notifier.loadOpenCodeModels(
        apiKey: _openCodeKeyCtrl.text,
        baseUrl: _form.opencodeBaseUrl,
      );
    }
  }

  AiSettings _buildPayload() {
    return _form.copyWith(
      ollamaBaseUrl: _ollamaUrlCtrl.text.trim(),
      ollamaModel: _ollamaModelCtrl.text.trim(),
      openrouterApiKey: _openRouterKeyCtrl.text.trim(),
      openrouterModel: _openRouterModelCtrl.text.trim(),
      opencodeApiKey: _openCodeKeyCtrl.text.trim(),
      opencodeModel: _openCodeModelCtrl.text.trim(),
      customSystemPrompt: _customPromptCtrl.text.trim().isEmpty
          ? null
          : _customPromptCtrl.text.trim(),
    );
  }

  Future<void> _openModelSearchDialog(List<AiProviderModelItem> items) async {
    final searchCtrl = TextEditingController();
    var filter = 'all';

    final chosen = await showAdaptiveFormSheet<String>(
      context: context,
      title: 'Selecionar modelo (${_form.activeDriver})',
      maxWidth: 640,
      builder: (sheetCtx) {
        return StatefulBuilder(
          builder: (context, setModalState) {
            final q = searchCtrl.text.trim().toLowerCase();
            final filtered = items
                .where((m) {
                  if (filter == 'free' && !m.isFree) return false;
                  if (filter == 'tools' && !m.supportsTools) return false;
                  if (filter == 'installed' && !m.installed) return false;
                  if (filter == 'recommended' && !m.recommended) return false;
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
                  controller: searchCtrl,
                  onChanged: (_) => setModalState(() {}),
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
                        selected: filter == entry.$1,
                        onSelected: (_) =>
                            setModalState(() => filter = entry.$1),
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
                                Navigator.of(sheetCtx)
                                    .pop(searchCtrl.text.trim()),
                            child: Text('Usar "${searchCtrl.text.trim()}"'),
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
                          onTap: () => Navigator.of(sheetCtx).pop(m.id),
                        );
                      },
                    ),
                  ),
              ],
            );
          },
        );
      },
    );
    searchCtrl.dispose();

    if (chosen == null || chosen.isEmpty) return;
    setState(() {
      if (_form.activeDriver == 'ollama') {
        _ollamaModelCtrl.text = chosen;
        _pullModelCtrl.text = chosen;
        _form = _form.copyWith(clearOllamaNetmonitorTrained: true);
      } else if (_form.activeDriver == 'openrouter') {
        _openRouterModelCtrl.text = chosen;
      } else {
        _openCodeModelCtrl.text = chosen;
      }
    });
  }

  Future<void> _testConnection() async {
    final payload = _buildPayload();
    final driver = payload.activeDriver;
    final res = await ref
        .read(aiSettingsProvider.notifier)
        .testConnection(
          driver: driver,
          baseUrl: driver == 'ollama'
              ? payload.ollamaBaseUrl
              : (driver == 'opencode' ? payload.opencodeBaseUrl : null),
          apiKey: driver == 'openrouter'
              ? payload.openrouterApiKey
              : (driver == 'opencode' ? payload.opencodeApiKey : null),
          model: payload.currentModelLabel,
        );
    if (!mounted || res == null) return;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(
          res.success
              ? 'Conexão OK (${formatLatency(res.latencyMs)}): ${res.message}'
              : 'Falha: ${res.message}',
        ),
      ),
    );
  }

  Future<void> _save() async {
    final ok = await ref
        .read(aiSettingsProvider.notifier)
        .save(_buildPayload());
    if (!mounted) return;
    if (ok) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Configurações de IA salvas.')),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(aiSettingsProvider);
    if (!_hydrated && state.settings != null) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted && !_hydrated) _hydrate();
      });
    }

    final scheme = context.colorScheme;
    final catalog = switch (_form.activeDriver) {
      'openrouter' => state.openRouterCatalog,
      'opencode' => state.openCodeCatalog,
      _ => state.ollamaCatalog,
    };

    return SectionCard(
      title: 'Assistente IA',
      subtitle: 'Provedor principal, modelo, automação e resumos proativos',
      icon: Icons.smart_toy_outlined,
      iconTone: SemanticTone.ai,
      trailing: Switch(
        value: _form.enabled,
        onChanged: (v) => setState(() => _form = _form.copyWith(enabled: v)),
      ),
      child: state.loading && !_hydrated
          ? const LoadingStateView(message: 'Carregando configurações de IA...')
          : Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                if (state.error != null) ...<Widget>[
                  NestedPanel(
                    tone: SemanticTone.error,
                    child: Text(state.error!),
                  ),
                  const SizedBox(height: 12),
                ],
                DropdownButtonFormField<String>(
                  initialValue: _form.activeDriver,
                  decoration: const InputDecoration(
                    labelText: 'Provedor ativo',
                  ),
                  items: const <DropdownMenuItem<String>>[
                    DropdownMenuItem<String>(
                      value: 'ollama',
                      child: Text('Ollama (Local / Privado)'),
                    ),
                    DropdownMenuItem<String>(
                      value: 'openrouter',
                      child: Text('OpenRouter (Nuvem)'),
                    ),
                    DropdownMenuItem<String>(
                      value: 'opencode',
                      child: Text('OpenCode Zen (Nuvem)'),
                    ),
                  ],
                  onChanged: (value) {
                    if (value == null) return;
                    setState(() => _form = _form.copyWith(activeDriver: value));
                    _loadModelsForDriver(value);
                  },
                ),
                const SizedBox(height: 12),

                // Campos específicos por provedor
                if (_form.activeDriver == 'ollama') ...<Widget>[
                  TextField(
                    controller: _ollamaUrlCtrl,
                    decoration: InputDecoration(
                      labelText: 'URL do Ollama',
                      hintText: 'http://ollama:11434/v1',
                      suffixIcon: IconButton(
                        tooltip: 'Recarregar modelos',
                        icon: const Icon(Icons.refresh),
                        onPressed: () => _loadModelsForDriver('ollama'),
                      ),
                    ),
                  ),
                  const SizedBox(height: 12),
                  Row(
                    children: <Widget>[
                      Expanded(
                        child: TextField(
                          controller: _ollamaModelCtrl,
                          decoration: const InputDecoration(
                            labelText: 'Modelo Ollama',
                            hintText: 'netmonitor/chat:q8_0',
                          ),
                        ),
                      ),
                      const SizedBox(width: 8),
                      OutlinedButton.icon(
                        onPressed: () => _openModelSearchDialog(
                          catalog?.models ?? const <AiProviderModelItem>[],
                        ),
                        icon: const Icon(Icons.search, size: 18),
                        label: const Text('Catálogo'),
                      ),
                    ],
                  ),
                  const SizedBox(height: 8),
                  Wrap(
                    spacing: 8,
                    runSpacing: 6,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: <Widget>[
                      TextButton.icon(
                        onPressed: () => context.go('/ai-benchmark?tab=chat'),
                        icon: const Icon(Icons.speed_outlined, size: 17),
                        label: const Text('Comparar modelos no benchmark'),
                      ),
                      if (catalog != null && !catalog.isOnline)
                        const SemanticChip(
                          label: 'Ollama offline',
                          tone: SemanticTone.warning,
                          compact: true,
                        ),
                    ],
                  ),
                  const SizedBox(height: 8),
                  DropdownButtonFormField<int>(
                    initialValue: _ollamaCtxSteps.contains(_form.ollamaNumCtx)
                        ? _form.ollamaNumCtx
                        : 16384,
                    decoration: const InputDecoration(
                      labelText: 'Janela de contexto (num_ctx)',
                    ),
                    items: _ollamaCtxSteps
                        .map(
                          (v) => DropdownMenuItem<int>(
                            value: v,
                            child: Text(formatContextWindow(v, detailed: true)),
                          ),
                        )
                        .toList(growable: false),
                    onChanged: (v) {
                      if (v != null) {
                        setState(() => _form = _form.copyWith(ollamaNumCtx: v));
                      }
                    },
                  ),
                  const SizedBox(height: 8),
                  SwitchListTile(
                    contentPadding: EdgeInsets.zero,
                    title: const Text('Raciocínio estendido (reasoning)'),
                    subtitle: const Text(
                      'Mantido desligado por padrão para respostas mais rápidas no estilo Direto.',
                    ),
                    value: _form.ollamaReasoning,
                    onChanged: (v) => setState(
                      () => _form = _form.copyWith(ollamaReasoning: v),
                    ),
                  ),
                  SwitchListTile(
                    contentPadding: EdgeInsets.zero,
                    title: const Text('Modelo treinado para o NetMonitor'),
                    subtitle: Text(
                      _form.ollamaNetmonitorTrained == null
                          ? 'Automático (detectado pelo nome do modelo)'
                          : (_form.ollamaNetmonitorTrained!
                                ? 'Ativado manualmente'
                                : 'Desativado manualmente'),
                    ),
                    value: _form.ollamaNetmonitorTrained ?? false,
                    onChanged: (v) => setState(
                      () => _form = _form.copyWith(ollamaNetmonitorTrained: v),
                    ),
                  ),
                  // Instalador Ollama (stream SSE-over-POST)
                  const SizedBox(height: 8),
                  NestedPanel(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: <Widget>[
                        Text(
                          'Baixar modelo no Ollama',
                          style: context.textTheme.titleSmall?.copyWith(
                            fontWeight: FontWeight.w700,
                          ),
                        ),
                        const SizedBox(height: 8),
                        Row(
                          children: <Widget>[
                            Expanded(
                              child: TextField(
                                controller: _pullModelCtrl,
                                enabled: !state.pulling,
                                decoration: const InputDecoration(
                                  hintText: 'Ex.: qwen2.5:7b-instruct',
                                  isDense: true,
                                ),
                              ),
                            ),
                            const SizedBox(width: 8),
                            if (state.pulling)
                              OutlinedButton.icon(
                                onPressed: () => ref
                                    .read(aiSettingsProvider.notifier)
                                    .cancelPull(),
                                icon: const Icon(Icons.close, size: 18),
                                label: const Text('Cancelar'),
                              )
                            else
                              FilledButton.tonalIcon(
                                onPressed: () {
                                  final m = _pullModelCtrl.text.trim();
                                  if (m.isEmpty) return;
                                  ref
                                      .read(aiSettingsProvider.notifier)
                                      .pullOllamaModel(
                                        m,
                                        baseUrl: _ollamaUrlCtrl.text,
                                      );
                                },
                                icon: const Icon(Icons.download, size: 18),
                                label: const Text('Baixar'),
                              ),
                          ],
                        ),
                        if (state.pullProgress != null) ...<Widget>[
                          const SizedBox(height: 10),
                          LinearProgressIndicator(
                            value: state.pullProgress!.percentage != null
                                ? (state.pullProgress!.percentage! / 100).clamp(
                                    0.0,
                                    1.0,
                                  )
                                : null,
                          ),
                          const SizedBox(height: 4),
                          Text(
                            state.pullProgress!.error ??
                                '${state.pullProgress!.status}'
                                    '${state.pullProgress!.completed != null && state.pullProgress!.total != null ? ' · ${formatBytes(state.pullProgress!.completed)} / ${formatBytes(state.pullProgress!.total)}' : ''}',
                            style: context.textTheme.labelSmall?.copyWith(
                              color: state.pullProgress!.error != null
                                  ? scheme.error
                                  : scheme.onSurfaceVariant,
                            ),
                          ),
                        ],
                      ],
                    ),
                  ),
                ] else if (_form.activeDriver == 'openrouter') ...<Widget>[
                  TextField(
                    controller: _openRouterKeyCtrl,
                    obscureText: !_showApiKey,
                    decoration: InputDecoration(
                      labelText: 'Chave de API OpenRouter',
                      suffixIcon: IconButton(
                        icon: Icon(
                          _showApiKey ? Icons.visibility_off : Icons.visibility,
                        ),
                        onPressed: () =>
                            setState(() => _showApiKey = !_showApiKey),
                      ),
                    ),
                  ),
                  const SizedBox(height: 12),
                  Row(
                    children: <Widget>[
                      Expanded(
                        child: TextField(
                          controller: _openRouterModelCtrl,
                          decoration: const InputDecoration(
                            labelText: 'Modelo OpenRouter',
                            hintText: 'openrouter/free',
                          ),
                        ),
                      ),
                      const SizedBox(width: 8),
                      OutlinedButton.icon(
                        onPressed: () => _openModelSearchDialog(
                          catalog?.models ?? const <AiProviderModelItem>[],
                        ),
                        icon: const Icon(Icons.search, size: 18),
                        label: const Text('Buscar'),
                      ),
                    ],
                  ),
                ] else ...<Widget>[
                  KeyValueRow(
                    label: 'Endpoint OpenCode Zen',
                    value:
                        _form.opencodeBaseUrl ?? 'https://opencode.ai/zen/v1',
                    monospace: true,
                  ),
                  const SizedBox(height: 8),
                  TextField(
                    controller: _openCodeKeyCtrl,
                    obscureText: !_showApiKey,
                    decoration: InputDecoration(
                      labelText: 'Chave de API OpenCode',
                      suffixIcon: IconButton(
                        icon: Icon(
                          _showApiKey ? Icons.visibility_off : Icons.visibility,
                        ),
                        onPressed: () =>
                            setState(() => _showApiKey = !_showApiKey),
                      ),
                    ),
                  ),
                  const SizedBox(height: 12),
                  Row(
                    children: <Widget>[
                      Expanded(
                        child: TextField(
                          controller: _openCodeModelCtrl,
                          decoration: const InputDecoration(
                            labelText: 'Modelo OpenCode',
                          ),
                        ),
                      ),
                      const SizedBox(width: 8),
                      OutlinedButton.icon(
                        onPressed: () => _openModelSearchDialog(
                          catalog?.models ?? const <AiProviderModelItem>[],
                        ),
                        icon: const Icon(Icons.search, size: 18),
                        label: const Text('Buscar'),
                      ),
                    ],
                  ),
                ],

                const Divider(height: 28),
                Text(
                  'Estilo de resposta e automação',
                  style: context.textTheme.titleSmall?.copyWith(
                    fontWeight: FontWeight.w700,
                  ),
                ),
                const SizedBox(height: 10),
                SegmentedButton<AiResponseStyle>(
                  selected: <AiResponseStyle>{_form.responseStyle},
                  onSelectionChanged: (sel) => setState(
                    () => _form = _form.copyWith(responseStyle: sel.first),
                  ),
                  segments: const <ButtonSegment<AiResponseStyle>>[
                    ButtonSegment<AiResponseStyle>(
                      value: AiResponseStyle.concise,
                      label: Text('Direto'),
                    ),
                    ButtonSegment<AiResponseStyle>(
                      value: AiResponseStyle.normal,
                      label: Text('Normal'),
                    ),
                  ],
                ),
                const SizedBox(height: 12),
                ResponsiveGrid(
                  xs: 1,
                  sm: 3,
                  md: 3,
                  lg: 3,
                  xl: 3,
                  children: <Widget>[
                    DropdownButtonFormField<AiAutomationMode>(
                      initialValue: _form.activeToolsMode,
                      decoration: const InputDecoration(
                        labelText: 'Testes ativos de rede',
                      ),
                      items: AiAutomationMode.values
                          .map(
                            (m) => DropdownMenuItem<AiAutomationMode>(
                              value: m,
                              child: Text(m.label),
                            ),
                          )
                          .toList(growable: false),
                      onChanged: (mode) {
                        if (mode == null) return;
                        setState(() {
                          _form = _form.copyWith(
                            allowActiveTools: mode != AiAutomationMode.off,
                            requireToolConfirmation:
                                mode != AiAutomationMode.auto,
                          );
                        });
                      },
                    ),
                    DropdownButtonFormField<AiAutomationMode>(
                      initialValue: _form.actionMode,
                      decoration: const InputDecoration(
                        labelText: 'Ações do sistema',
                      ),
                      items: AiAutomationMode.values
                          .map(
                            (m) => DropdownMenuItem<AiAutomationMode>(
                              value: m,
                              child: Text(m.label),
                            ),
                          )
                          .toList(growable: false),
                      onChanged: (mode) {
                        if (mode != null) {
                          setState(
                            () => _form = _form.copyWith(actionMode: mode),
                          );
                        }
                      },
                    ),
                    DropdownButtonFormField<AiAutomationMode>(
                      initialValue: _form.containerActions,
                      decoration: const InputDecoration(
                        labelText: 'Ações em containers',
                      ),
                      items: AiAutomationMode.values
                          .map(
                            (m) => DropdownMenuItem<AiAutomationMode>(
                              value: m,
                              child: Text(m.label),
                            ),
                          )
                          .toList(growable: false),
                      onChanged: (mode) {
                        if (mode != null) {
                          setState(
                            () =>
                                _form = _form.copyWith(containerActions: mode),
                          );
                        }
                      },
                    ),
                  ],
                ),

                const Divider(height: 28),
                Row(
                  children: <Widget>[
                    Text(
                      'IA Proativa (resumos e digests)',
                      style: context.textTheme.titleSmall?.copyWith(
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                    const SizedBox(width: 8),
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
                  value: _form.proactive.incidentSummaries,
                  onChanged: (v) => setState(
                    () => _form = _form.copyWith(
                      proactive: _form.proactive.copyWith(incidentSummaries: v),
                    ),
                  ),
                ),
                if (_form.proactive.incidentSummaries) ...<Widget>[
                  ResponsiveGrid(
                    xs: 1,
                    sm: 2,
                    md: 2,
                    lg: 2,
                    xl: 2,
                    children: <Widget>[
                      DropdownButtonFormField<String>(
                        initialValue: _form.proactive.incidentMinSeverity,
                        decoration: const InputDecoration(
                          labelText: 'Severidade mínima',
                        ),
                        items: const <DropdownMenuItem<String>>[
                          DropdownMenuItem<String>(
                            value: 'critical',
                            child: Text('Crítico'),
                          ),
                          DropdownMenuItem<String>(
                            value: 'warning',
                            child: Text('Aviso ou maior'),
                          ),
                        ],
                        onChanged: (v) {
                          if (v != null) {
                            setState(
                              () => _form = _form.copyWith(
                                proactive: _form.proactive.copyWith(
                                  incidentMinSeverity: v,
                                ),
                              ),
                            );
                          }
                        },
                      ),
                      DropdownButtonFormField<int>(
                        initialValue: _form.proactive.maxSummariesPerHour.clamp(
                          1,
                          60,
                        ),
                        decoration: const InputDecoration(
                          labelText: 'Máx. resumos por hora',
                        ),
                        items: const <int>[2, 4, 6, 10, 20, 30, 60]
                            .map(
                              (n) => DropdownMenuItem<int>(
                                value: n,
                                child: Text('$n / hora'),
                              ),
                            )
                            .toList(growable: false),
                        onChanged: (v) {
                          if (v != null) {
                            setState(
                              () => _form = _form.copyWith(
                                proactive: _form.proactive.copyWith(
                                  maxSummariesPerHour: v,
                                ),
                              ),
                            );
                          }
                        },
                      ),
                    ],
                  ),
                  const SizedBox(height: 10),
                ],
                ResponsiveGrid(
                  xs: 1,
                  sm: 2,
                  md: 2,
                  lg: 2,
                  xl: 2,
                  children: <Widget>[
                    DropdownButtonFormField<String>(
                      initialValue: _form.proactive.digest,
                      decoration: const InputDecoration(
                        labelText: 'Resumo periódico (Digest)',
                      ),
                      items: const <DropdownMenuItem<String>>[
                        DropdownMenuItem<String>(
                          value: 'off',
                          child: Text('Desligado'),
                        ),
                        DropdownMenuItem<String>(
                          value: 'daily',
                          child: Text('Diário'),
                        ),
                        DropdownMenuItem<String>(
                          value: 'weekly',
                          child: Text('Semanal'),
                        ),
                      ],
                      onChanged: (v) {
                        if (v != null) {
                          setState(
                            () => _form = _form.copyWith(
                              proactive: _form.proactive.copyWith(digest: v),
                            ),
                          );
                        }
                      },
                    ),
                    if (_form.proactive.digest != 'off')
                      DropdownButtonFormField<int>(
                        initialValue: _form.proactive.digestHour.clamp(0, 23),
                        decoration: const InputDecoration(
                          labelText: 'Horário do servidor',
                        ),
                        items: List<DropdownMenuItem<int>>.generate(
                          24,
                          (h) => DropdownMenuItem<int>(
                            value: h,
                            child: Text(formatHourOfDay(h)),
                          ),
                        ),
                        onChanged: (v) {
                          if (v != null) {
                            setState(
                              () => _form = _form.copyWith(
                                proactive: _form.proactive.copyWith(
                                  digestHour: v,
                                ),
                              ),
                            );
                          }
                        },
                      ),
                  ],
                ),
                if (_form.proactive.digest != 'off')
                  SwitchListTile(
                    contentPadding: EdgeInsets.zero,
                    title: const Text('Pular digest quando tudo estiver calmo'),
                    value: _form.proactive.skipQuietDigest,
                    onChanged: (v) => setState(
                      () => _form = _form.copyWith(
                        proactive: _form.proactive.copyWith(skipQuietDigest: v),
                      ),
                    ),
                  ),

                const SizedBox(height: 12),
                TextField(
                  controller: _customPromptCtrl,
                  maxLines: 3,
                  decoration: const InputDecoration(
                    labelText: 'Instruções personalizadas (opcional)',
                    hintText: 'Ex.: Sempre responda com foco em links BGP e clientes corporativos.',
                  ),
                ),

                if (state.testResult != null) ...<Widget>[
                  const SizedBox(height: 12),
                  NestedPanel(
                    tone: state.testResult!.success
                        ? SemanticTone.success
                        : SemanticTone.error,
                    child: Text(
                      '${state.testResult!.message} (${formatLatency(state.testResult!.latencyMs)})',
                    ),
                  ),
                ],
                const SizedBox(height: 16),
                Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  alignment: WrapAlignment.end,
                  children: <Widget>[
                    OutlinedButton.icon(
                      onPressed: state.testing ? null : _testConnection,
                      icon: state.testing
                          ? const SizedBox(
                              width: 16,
                              height: 16,
                              child: CircularProgressIndicator(strokeWidth: 2),
                            )
                          : const Icon(Icons.cable_outlined, size: 18),
                      label: const Text('Testar conexão'),
                    ),
                    FilledButton.icon(
                      onPressed: state.saving ? null : _save,
                      icon: state.saving
                          ? const SizedBox(
                              width: 16,
                              height: 16,
                              child: CircularProgressIndicator(strokeWidth: 2),
                            )
                          : const Icon(Icons.save_outlined, size: 18),
                      label: const Text('Salvar'),
                    ),
                  ],
                ),
              ],
            ),
    );
  }
}
