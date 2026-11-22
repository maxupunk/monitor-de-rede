import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';
import 'package:netmonitor_app/features/ai/providers/ai_providers.dart';
import 'package:netmonitor_app/features/ai/widgets/ai_model_catalog_sheet.dart';
import 'package:netmonitor_app/features/ai/widgets/ai_model_pull_panel.dart';
import 'package:netmonitor_app/features/ai/widgets/ai_proactive_settings_section.dart';
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
    final chosen = await showAiModelCatalogSheet(
      context: context,
      driver: _form.activeDriver,
      items: items,
    );
    if (chosen == null || chosen.isEmpty || !mounted) return;
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
                AppDropdownFormField<String>(
                  initialValue: _form.activeDriver,
                  decoration: const InputDecoration(
                    labelText: 'Provedor ativo',
                  ),
                  items: const <DropdownMenuItem<String>>[
                    DropdownMenuItem<String>(
                      value: 'ollama',
                      child: Text(
                        'Ollama (Local / Privado)',
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                    DropdownMenuItem<String>(
                      value: 'openrouter',
                      child: Text(
                        'OpenRouter (Nuvem)',
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                    DropdownMenuItem<String>(
                      value: 'opencode',
                      child: Text(
                        'OpenCode Zen (Nuvem)',
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                  ],
                  onChanged: (value) {
                    if (value == null) return;
                    setState(() => _form = _form.copyWith(activeDriver: value));
                    _loadModelsForDriver(value);
                  },
                ),
                const SizedBox(height: 12),

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
                  LayoutBuilder(
                    builder: (context, constraints) {
                      final field = TextField(
                        controller: _ollamaModelCtrl,
                        decoration: const InputDecoration(
                          labelText: 'Modelo Ollama',
                          hintText: 'netmonitor/chat:q8_0',
                        ),
                      );
                      final btn = OutlinedButton.icon(
                        onPressed: () => _openModelSearchDialog(
                          catalog?.models ?? const <AiProviderModelItem>[],
                        ),
                        icon: const Icon(Icons.search, size: 18),
                        label: const Text('Catálogo'),
                      );
                      if (constraints.maxWidth < 400) {
                        return Column(
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: <Widget>[
                            field,
                            const SizedBox(height: 8),
                            Align(alignment: Alignment.centerRight, child: btn),
                          ],
                        );
                      }
                      return Row(
                        children: <Widget>[
                          Expanded(child: field),
                          const SizedBox(width: 8),
                          btn,
                        ],
                      );
                    },
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
                  AppDropdownFormField<int>(
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
                            child: Text(
                              formatContextWindow(v, detailed: true),
                              overflow: TextOverflow.ellipsis,
                            ),
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
                  const SizedBox(height: 8),
                  AiModelPullPanel(
                    title: 'Baixar modelo no Ollama',
                    controller: _pullModelCtrl,
                    pulling: state.pulling,
                    progress: state.pullProgress,
                    onCancel: () =>
                        ref.read(aiSettingsProvider.notifier).cancelPull(),
                    onPull: () {
                      final m = _pullModelCtrl.text.trim();
                      if (m.isEmpty) return;
                      ref
                          .read(aiSettingsProvider.notifier)
                          .pullOllamaModel(m, baseUrl: _ollamaUrlCtrl.text);
                    },
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
                  showSelectedIcon: false,
                  style: SegmentedButton.styleFrom(
                    visualDensity: VisualDensity.compact,
                    padding: const EdgeInsets.symmetric(horizontal: 10),
                  ),
                  selected: <AiResponseStyle>{_form.responseStyle},
                  onSelectionChanged: (sel) => setState(
                    () => _form = _form.copyWith(responseStyle: sel.first),
                  ),
                  segments: const <ButtonSegment<AiResponseStyle>>[
                    ButtonSegment<AiResponseStyle>(
                      value: AiResponseStyle.concise,
                      label: Text('Direto', maxLines: 1, softWrap: false),
                    ),
                    ButtonSegment<AiResponseStyle>(
                      value: AiResponseStyle.normal,
                      label: Text('Normal', maxLines: 1, softWrap: false),
                    ),
                  ],
                ),
                const SizedBox(height: 12),
                ResponsiveFieldRow(
                  breakpoint: 640,
                  spacing: 12,
                  runSpacing: 12,
                  children: <Widget>[
                    AppDropdownFormField<AiAutomationMode>(
                      initialValue: _form.activeToolsMode,
                      decoration: const InputDecoration(
                        labelText: 'Testes ativos de rede',
                      ),
                      items: AiAutomationMode.values
                          .map(
                            (m) => DropdownMenuItem<AiAutomationMode>(
                              value: m,
                              child: Text(
                                m.label,
                                overflow: TextOverflow.ellipsis,
                              ),
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
                    AppDropdownFormField<AiAutomationMode>(
                      initialValue: _form.actionMode,
                      decoration: const InputDecoration(
                        labelText: 'Ações do sistema',
                      ),
                      items: AiAutomationMode.values
                          .map(
                            (m) => DropdownMenuItem<AiAutomationMode>(
                              value: m,
                              child: Text(
                                m.label,
                                overflow: TextOverflow.ellipsis,
                              ),
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
                    AppDropdownFormField<AiAutomationMode>(
                      initialValue: _form.containerActions,
                      decoration: const InputDecoration(
                        labelText: 'Ações em containers',
                      ),
                      items: AiAutomationMode.values
                          .map(
                            (m) => DropdownMenuItem<AiAutomationMode>(
                              value: m,
                              child: Text(
                                m.label,
                                overflow: TextOverflow.ellipsis,
                              ),
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
                AiProactiveSettingsSection(
                  proactive: _form.proactive,
                  onChanged: (next) =>
                      setState(() => _form = _form.copyWith(proactive: next)),
                ),

                const SizedBox(height: 12),
                TextField(
                  controller: _customPromptCtrl,
                  maxLines: 3,
                  decoration: const InputDecoration(
                    labelText: 'Instruções personalizadas (opcional)',
                    hintText:
                        'Ex.: Sempre responda com foco em links BGP e clientes corporativos.',
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
