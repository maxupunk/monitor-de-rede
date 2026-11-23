import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';
import 'package:netmonitor_app/features/ai/providers/ai_providers.dart';
import 'package:netmonitor_app/features/ai/utils/ai_chat_logic.dart';
import 'package:netmonitor_app/features/ai/widgets/ai_model_pull_panel.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Cartão de configurações do Decisor rápido (`DeciderSettingsCard.vue`) na aba `ia`.
class DeciderSettingsCard extends ConsumerStatefulWidget {
  const DeciderSettingsCard({super.key});

  @override
  ConsumerState<DeciderSettingsCard> createState() =>
      _DeciderSettingsCardState();
}

class _DeciderSettingsCardState extends ConsumerState<DeciderSettingsCard> {
  AiDeciderSettings _form = const AiDeciderSettings();
  bool _hydrated = false;
  bool _showKey = false;

  final _baseUrlCtrl = TextEditingController();
  final _apiKeyCtrl = TextEditingController();
  final _accountIdCtrl = TextEditingController();
  final _modelCtrl = TextEditingController();
  final _timeoutCtrl = TextEditingController(text: '4000');
  final _testQuestionCtrl = TextEditingController(
    text: 'Quais dispositivos estão fora do ar agora?',
  );

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      await ref.read(aiDeciderProvider.notifier).loadAll();
      if (mounted) _hydrate();
    });
  }

  @override
  void dispose() {
    _baseUrlCtrl.dispose();
    _apiKeyCtrl.dispose();
    _accountIdCtrl.dispose();
    _modelCtrl.dispose();
    _timeoutCtrl.dispose();
    _testQuestionCtrl.dispose();
    super.dispose();
  }

  void _hydrate() {
    final saved = ref.read(aiDeciderProvider).saved;
    if (saved == null) return;
    _form = saved;
    _baseUrlCtrl.text = saved.baseUrl;
    _apiKeyCtrl.text = saved.apiKey ?? '';
    _accountIdCtrl.text = saved.accountId ?? '';
    _modelCtrl.text = saved.model;
    _timeoutCtrl.text = saved.timeoutMs.toString();
    setState(() => _hydrated = true);
  }

  AiDeciderSettings _currentForm() {
    return _form.copyWith(
      baseUrl: _baseUrlCtrl.text.trim(),
      apiKey: _apiKeyCtrl.text.trim().isEmpty ? null : _apiKeyCtrl.text.trim(),
      clearApiKey: _apiKeyCtrl.text.trim().isEmpty,
      accountId: _accountIdCtrl.text.trim().isEmpty
          ? null
          : _accountIdCtrl.text.trim(),
      clearAccountId: _accountIdCtrl.text.trim().isEmpty,
      model: _modelCtrl.text.trim(),
      timeoutMs: int.tryParse(_timeoutCtrl.text.trim()) ?? _form.timeoutMs,
    );
  }

  Future<void> _toggleEnabled(bool next) async {
    if (!next) {
      setState(() => _form = _form.copyWith(enabled: false));
      return;
    }
    final current = _currentForm();
    final provider = ref.read(aiDeciderProvider).providerById(current.provider);
    final isCloud = provider?.cloud == true;
    final isLocal =
        current.provider == 'ollaya' && isLocalOllaya(current.baseUrl);

    if (isCloud || isLocal) {
      final confirmed = await showConfirmActionDialog(
        context: context,
        title: isCloud
            ? 'Ativar Decisor na nuvem?'
            : 'Ativar Decisor no Ollaya local?',
        message: isCloud
            ? 'Perguntas e resumos curtos serão enviados ao provedor ${provider?.label ?? current.provider} para classificação e podem gerar custos na sua conta.'
            : 'Carregar o modelo "${current.model}" no Ollaya local consome memória RAM neste servidor.',
        confirmLabel: 'Ativar Decisor',
        tone: SemanticTone.warning,
        icon: isCloud ? Icons.cloud_outlined : Icons.memory_outlined,
      );
      if (!confirmed || !mounted) return;
    }
    setState(() => _form = _form.copyWith(enabled: true));
  }

  void _selectProvider(DeciderProviderView provider) {
    final prevProvider = ref
        .read(aiDeciderProvider)
        .providerById(_form.provider);
    final nextTimeout = timeoutForModel(
      current: int.tryParse(_timeoutCtrl.text.trim()) ?? 4000,
      previousSuggested: prevProvider?.suggestedTimeoutMs,
      nextSuggested: provider.suggestedTimeoutMs,
    );
    setState(() {
      _form = _form.copyWith(
        provider: provider.id,
        baseUrl: provider.defaultBaseUrl,
        model: provider.defaultModel,
        clearApiKey: true,
        clearAccountId: true,
        timeoutMs: nextTimeout,
      );
      _baseUrlCtrl.text = provider.defaultBaseUrl;
      _modelCtrl.text = provider.defaultModel;
      _apiKeyCtrl.clear();
      _accountIdCtrl.clear();
      _timeoutCtrl.text = nextTimeout.toString();
    });
    ref.read(aiDeciderProvider.notifier).loadModels(_currentForm());
  }

  Future<void> _save() async {
    final err = await ref.read(aiDeciderProvider.notifier).save(_currentForm());
    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text(err ?? 'Configurações do Decisor salvas.')),
    );
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(aiDeciderProvider);
    if (!_hydrated && state.saved != null) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted && !_hydrated) _hydrate();
      });
    }

    final current = _currentForm();
    final provider = state.providerById(current.provider);
    final gap = providerMissing(current, provider);
    final isOllaya = current.provider == 'ollaya';
    final installed = !isOllaya || state.isInstalled(current.model);
    final inMemory = isOllaya && state.isInMemory(current.model);
    final online = state.models?.online ?? false;

    final statusLabel = state.loadingModels
        ? 'Verificando...'
        : gap ??
              (isOllaya && !online
                  ? 'Ollaya fora do ar'
                  : (isOllaya && !installed
                        ? 'Modelo não baixado'
                        : (_form.enabled ? 'Ativo' : 'Pronto, desligado')));
    final statusTone = state.loadingModels
        ? SemanticTone.info
        : (gap != null || (isOllaya && (!online || !installed))
              ? SemanticTone.warning
              : (_form.enabled
                    ? SemanticTone.success
                    : SemanticTone.secondary));

    final canEnable = gap == null && (!isOllaya || (online && installed));
    final statusChip = SemanticChip(
      label: statusLabel,
      tone: statusTone,
      compact: true,
    );

    return SectionCard(
      title: 'Decisor Rápido (Ollaya / Roteador)',
      subtitle:
          'Seleciona ferramentas, classifica logs e faz triagem de alertas com baixa latência',
      icon: Icons.bolt_outlined,
      iconTone: SemanticTone.ai,
      trailing: Wrap(
        spacing: 10,
        runSpacing: 6,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: <Widget>[
          statusChip,
          Switch(
            value: _form.enabled,
            onChanged: (_form.enabled || canEnable) ? _toggleEnabled : null,
          ),
        ],
      ),
      child: state.loading && !_hydrated
          ? const LoadingStateView(message: 'Carregando Decisor...')
          : Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                // Passo 1: Provedor
                if (state.providers.isNotEmpty) ...<Widget>[
                  AppDropdownFormField<String>(
                    initialValue:
                        state.providers.any((p) => p.id == _form.provider)
                        ? _form.provider
                        : state.providers.first.id,
                    decoration: const InputDecoration(labelText: '1. Provedor'),
                    items: state.providers
                        .map(
                          (p) => DropdownMenuItem<String>(
                            value: p.id,
                            child: Text(
                              p.label,
                              overflow: TextOverflow.ellipsis,
                            ),
                          ),
                        )
                        .toList(growable: false),
                    onChanged: (id) {
                      if (id == null) return;
                      final target = state.providerById(id);
                      if (target != null) _selectProvider(target);
                    },
                  ),
                  if (provider != null &&
                      provider.description.isNotEmpty) ...<Widget>[
                    const SizedBox(height: 6),
                    Text(
                      provider.description,
                      style: context.textTheme.bodySmall?.copyWith(
                        color: context.colorScheme.onSurfaceVariant,
                      ),
                    ),
                  ],
                  const SizedBox(height: 12),
                ],
                if (current.provider == 'custom') ...<Widget>[
                  SegmentedButton<String>(
                    showSelectedIcon: false,
                    style: SegmentedButton.styleFrom(
                      visualDensity: VisualDensity.compact,
                      padding: const EdgeInsets.symmetric(horizontal: 8),
                    ),
                    selected: <String>{_form.customProtocol},
                    onSelectionChanged: (sel) => setState(
                      () => _form = _form.copyWith(customProtocol: sel.first),
                    ),
                    segments: const <ButtonSegment<String>>[
                      ButtonSegment<String>(
                        value: 'systemone',
                        label: Text(
                          'SystemOne',
                          maxLines: 1,
                          softWrap: false,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                      ButtonSegment<String>(
                        value: 'openai',
                        label: Text(
                          'OpenAI compatível',
                          maxLines: 1,
                          softWrap: false,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                    ],
                  ),
                  const SizedBox(height: 12),
                ],
                if (isOllaya ||
                    current.provider == 'custom' ||
                    provider?.defaultBaseUrl.isEmpty == true) ...<Widget>[
                  TextField(
                    controller: _baseUrlCtrl,
                    onChanged: (_) => setState(() {}),
                    decoration: InputDecoration(
                      labelText: 'URL do servidor Decisor',
                      suffixIcon: IconButton(
                        tooltip: 'Consultar modelos',
                        icon: const Icon(Icons.refresh),
                        onPressed: () => ref
                            .read(aiDeciderProvider.notifier)
                            .loadModels(_currentForm()),
                      ),
                    ),
                  ),
                  const SizedBox(height: 12),
                ],
                if (provider?.requiresAccount == true) ...<Widget>[
                  TextField(
                    controller: _accountIdCtrl,
                    onChanged: (_) => setState(() {}),
                    decoration: const InputDecoration(
                      labelText: 'ID da conta (Cloudflare)',
                    ),
                  ),
                  const SizedBox(height: 12),
                ],
                if (provider?.requiresKey == true) ...<Widget>[
                  TextField(
                    controller: _apiKeyCtrl,
                    obscureText: !_showKey,
                    onChanged: (_) => setState(() {}),
                    decoration: InputDecoration(
                      labelText: 'Chave de API',
                      suffixIcon: IconButton(
                        icon: Icon(
                          _showKey ? Icons.visibility_off : Icons.visibility,
                        ),
                        onPressed: () => setState(() => _showKey = !_showKey),
                      ),
                    ),
                  ),
                  const SizedBox(height: 12),
                ],

                // Passo 2: Modelo + Pull / WarmUp
                TextField(
                  controller: _modelCtrl,
                  decoration: const InputDecoration(
                    labelText: '2. Modelo do Decisor',
                    hintText: 'netmonitor/decima-network:v15',
                  ),
                  onChanged: (_) => setState(() {}),
                ),
                const SizedBox(height: 8),
                if ((state.models?.options ?? const <DeciderModelOption>[])
                    .isNotEmpty) ...<Widget>[
                  Wrap(
                    spacing: 6,
                    runSpacing: 6,
                    children: state.models!.options
                        .map((opt) {
                          return ChoiceChip(
                            label: Text(
                              '${opt.name}${opt.recommended ? ' ★' : ''}${opt.installed ? ' (baixado)' : ''}',
                            ),
                            selected:
                                opt.name.toLowerCase() ==
                                current.model.toLowerCase(),
                            onSelected: (_) {
                              setState(() {
                                _modelCtrl.text = opt.name;
                                if (opt.spec != null) {
                                  _timeoutCtrl.text = opt
                                      .spec!
                                      .suggestedTimeoutMs
                                      .toString();
                                }
                              });
                            },
                          );
                        })
                        .toList(growable: false),
                  ),
                  const SizedBox(height: 8),
                ],
                Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: <Widget>[
                    TextButton.icon(
                      onPressed: () => context.go('/ai-benchmark?tab=decider'),
                      icon: const Icon(Icons.speed_outlined, size: 17),
                      label: const Text('Ver benchmark do Decisor'),
                    ),
                    if (isOllaya && !installed && !state.pulling)
                      FilledButton.tonalIcon(
                        onPressed: () => ref
                            .read(aiDeciderProvider.notifier)
                            .pull(_currentForm()),
                        icon: const Icon(Icons.download, size: 18),
                        label: Text('Baixar ${current.model}'),
                      ),
                    if (isOllaya && state.pulling)
                      OutlinedButton.icon(
                        onPressed: () =>
                            ref.read(aiDeciderProvider.notifier).cancelPull(),
                        icon: const Icon(Icons.close, size: 18),
                        label: const Text('Cancelar download'),
                      ),
                    if (isOllaya && installed)
                      FilledButton.tonalIcon(
                        onPressed: state.modelState?.state == 'loading'
                            ? null
                            : () => ref
                                  .read(aiDeciderProvider.notifier)
                                  .warmUp(_currentForm()),
                        icon: Icon(
                          inMemory
                              ? Icons.check_circle_outline
                              : Icons.memory_outlined,
                          size: 18,
                        ),
                        label: Text(
                          state.modelState?.state == 'loading'
                              ? 'Carregando na memória...'
                              : (inMemory ? 'Na memória' : 'Carregar agora'),
                        ),
                      ),
                  ],
                ),
                if (state.pullProgress != null) ...<Widget>[
                  const SizedBox(height: 8),
                  AiModelPullProgressBar(progress: state.pullProgress!),
                ],

                // Onde usar (6 switches)
                const Divider(height: 28),
                Text(
                  'Onde usar o Decisor',
                  style: context.textTheme.titleSmall?.copyWith(
                    fontWeight: FontWeight.w700,
                  ),
                ),
                SwitchListTile(
                  contentPadding: EdgeInsets.zero,
                  title: const Text('Ferramentas do chat'),
                  subtitle: const Text(
                    'Escolhe apenas os grupos necessários por pergunta.',
                  ),
                  value: _form.features.chatTools,
                  onChanged: (v) => setState(
                    () => _form = _form.copyWith(
                      features: _form.features.copyWith(chatTools: v),
                    ),
                  ),
                ),
                SwitchListTile(
                  contentPadding: EdgeInsets.zero,
                  title: const Text('Revisão de ações'),
                  subtitle: const Text(
                    'Avalia risco de ações propostas pelo assistente.',
                  ),
                  value: _form.features.actionReview,
                  onChanged: (v) => setState(
                    () => _form = _form.copyWith(
                      features: _form.features.copyWith(actionReview: v),
                    ),
                  ),
                ),
                SwitchListTile(
                  contentPadding: EdgeInsets.zero,
                  title: const Text('Identificação de equipamentos'),
                  value: _form.features.deviceIdentity,
                  onChanged: (v) => setState(
                    () => _form = _form.copyWith(
                      features: _form.features.copyWith(deviceIdentity: v),
                    ),
                  ),
                ),
                SwitchListTile(
                  contentPadding: EdgeInsets.zero,
                  title: const Text('Classificação de interfaces'),
                  value: _form.features.interfaces,
                  onChanged: (v) => setState(
                    () => _form = _form.copyWith(
                      features: _form.features.copyWith(interfaces: v),
                    ),
                  ),
                ),
                SwitchListTile(
                  contentPadding: EdgeInsets.zero,
                  title: const Text('Classificação de templates de logs'),
                  value: _form.features.logEvents,
                  onChanged: (v) => setState(
                    () => _form = _form.copyWith(
                      features: _form.features.copyWith(logEvents: v),
                    ),
                  ),
                ),
                SwitchListTile(
                  contentPadding: EdgeInsets.zero,
                  title: const Text('Triagem de incidentes (alertas)'),
                  value: _form.features.incidentTriage != 'off',
                  onChanged: (v) => setState(
                    () => _form = _form.copyWith(
                      features: _form.features.copyWith(
                        incidentTriage: v ? 'enforce' : 'off',
                      ),
                    ),
                  ),
                ),

                // Avançado
                const SizedBox(height: 8),
                NestedPanel(
                  padding: EdgeInsets.zero,
                  child: Material(
                    type: MaterialType.transparency,
                    child: ExpansionTile(
                      title: const Text(
                        'Avançado e teste ao vivo',
                        style: TextStyle(fontWeight: FontWeight.w700),
                      ),
                      childrenPadding: const EdgeInsets.fromLTRB(14, 0, 14, 14),
                      children: <Widget>[
                        LayoutBuilder(
                          builder: (context, constraints) {
                            final questionField = TextField(
                              controller: _testQuestionCtrl,
                              decoration: const InputDecoration(
                                labelText: 'Experimentar pergunta',
                                isDense: true,
                              ),
                            );
                            final testBtn = FilledButton.tonal(
                              onPressed: state.testing
                                  ? null
                                  : () => ref
                                        .read(aiDeciderProvider.notifier)
                                        .test(
                                          _currentForm(),
                                          question: _testQuestionCtrl.text,
                                        ),
                              child: state.testing
                                  ? const SizedBox(
                                      width: 16,
                                      height: 16,
                                      child: CircularProgressIndicator(
                                        strokeWidth: 2,
                                      ),
                                    )
                                  : const Text('Testar'),
                            );
                            if (constraints.maxWidth < 400) {
                              return Column(
                                crossAxisAlignment: CrossAxisAlignment.stretch,
                                children: <Widget>[
                                  questionField,
                                  const SizedBox(height: 8),
                                  Align(
                                    alignment: Alignment.centerRight,
                                    child: testBtn,
                                  ),
                                ],
                              );
                            }
                            return Row(
                              children: <Widget>[
                                Expanded(child: questionField),
                                const SizedBox(width: 8),
                                testBtn,
                              ],
                            );
                          },
                        ),
                        if (state.testResult != null) ...<Widget>[
                          const SizedBox(height: 10),
                          NestedPanel(
                            tone: state.testResult!.success
                                ? SemanticTone.success
                                : SemanticTone.error,
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.stretch,
                              children: <Widget>[
                                Text(
                                  '${state.testResult!.message} (${formatLatency(state.testResult!.latencyMs)})',
                                  style: const TextStyle(
                                    fontWeight: FontWeight.w600,
                                  ),
                                ),
                                for (final group in state.testResult!.groups)
                                  Padding(
                                    padding: const EdgeInsets.only(top: 6),
                                    child: Row(
                                      children: <Widget>[
                                        SizedBox(
                                          width: 100,
                                          child: Text(
                                            group.group,
                                            maxLines: 1,
                                            overflow: TextOverflow.ellipsis,
                                            style: context.textTheme.labelSmall,
                                          ),
                                        ),
                                        Expanded(
                                          child: LinearProgressIndicator(
                                            value: (group.confidence / 100)
                                                .clamp(0.0, 1.0),
                                            color:
                                                group.confidence >=
                                                    _form.minConfidence
                                                ? context.semanticColors.success
                                                : context.colorScheme.outline,
                                          ),
                                        ),
                                        const SizedBox(width: 8),
                                        Text('${group.confidence}%'),
                                      ],
                                    ),
                                  ),
                              ],
                            ),
                          ),
                        ],
                        const SizedBox(height: 12),
                        Text(
                          'Confiança mínima: ${_form.minConfidence}%',
                          style: context.textTheme.labelMedium,
                        ),
                        Slider(
                          min: 30,
                          max: 95,
                          divisions: 13,
                          value: _form.minConfidence.toDouble().clamp(30, 95),
                          label: '${_form.minConfidence}%',
                          onChanged: (v) => setState(
                            () => _form = _form.copyWith(
                              minConfidence: v.round(),
                            ),
                          ),
                        ),
                        if (_form.features.incidentTriage != 'off') ...<Widget>[
                          AppDropdownFormField<String>(
                            initialValue:
                                _form.features.incidentTriage == 'shadow'
                                ? 'shadow'
                                : 'enforce',
                            decoration: const InputDecoration(
                              labelText: 'Modo da triagem de incidentes',
                            ),
                            items: const <DropdownMenuItem<String>>[
                              DropdownMenuItem<String>(
                                value: 'enforce',
                                child: Text(
                                  'Ativo (enforce)',
                                  overflow: TextOverflow.ellipsis,
                                ),
                              ),
                              DropdownMenuItem<String>(
                                value: 'shadow',
                                child: Text(
                                  'Observação (shadow)',
                                  overflow: TextOverflow.ellipsis,
                                ),
                              ),
                            ],
                            onChanged: (v) {
                              if (v != null) {
                                setState(
                                  () => _form = _form.copyWith(
                                    features: _form.features.copyWith(
                                      incidentTriage: v,
                                    ),
                                  ),
                                );
                              }
                            },
                          ),
                          const SizedBox(height: 10),
                        ],
                        TextField(
                          controller: _timeoutCtrl,
                          keyboardType: TextInputType.number,
                          decoration: const InputDecoration(
                            labelText: 'Tempo máximo por decisão (ms)',
                            hintText: '200 a 10000',
                          ),
                        ),
                      ],
                    ),
                  ),
                ),

                const SizedBox(height: 14),
                Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  alignment: WrapAlignment.end,
                  children: <Widget>[
                    TextButton(
                      onPressed: _hydrate,
                      child: const Text('Descartar'),
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
                      label: const Text('Salvar Decisor'),
                    ),
                  ],
                ),
              ],
            ),
    );
  }
}
