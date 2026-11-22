import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/features/network/models/link_test_models.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/features/network/providers/link_tests_provider.dart';
import 'package:netmonitor_app/features/network/providers/topology_provider.dart';
import 'package:netmonitor_app/features/network/widgets/device_access_host.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

class _FixStepOutcome {
  const _FixStepOutcome._({required this.isCancelled, this.summary = ''});

  factory _FixStepOutcome.applied(String summary) =>
      _FixStepOutcome._(isCancelled: false, summary: summary);

  static const _FixStepOutcome cancelled = _FixStepOutcome._(isCancelled: true);

  final bool isCancelled;
  final String summary;
}

class LinkTestWizardBody extends ConsumerStatefulWidget {
  const LinkTestWizardBody({
    super.key,
    this.initialDraft,
    required this.onStarted,
  });

  final LinkTestRequestDraft? initialDraft;
  final ValueChanged<int> onStarted;

  @override
  ConsumerState<LinkTestWizardBody> createState() => _LinkTestWizardBodyState();
}

class _LinkTestWizardBodyState extends ConsumerState<LinkTestWizardBody> {
  late String _kind;
  int? _linkId;
  int? _sourceDeviceId;
  int? _targetDeviceId;
  String _direction = 'both';
  int _durationS = 10;
  int _streams = 4;
  final TextEditingController _limitCtrl = TextEditingController();
  Timer? _limitDebounce;

  bool _planning = false;
  bool _starting = false;
  String? _resolvingFixKey;
  bool _resolvingBatch = false;
  bool _accessModalOpen = false;
  String? _resolvingBatchProgress;

  LinkTestPlanModel? _plan;
  String? _error;
  String? _feedbackMessage;
  SemanticTone _feedbackTone = SemanticTone.info;

  int _planEpoch = 0;
  String? _lastAutoPlannedSignature;

  bool get _isResolvingAny =>
      _resolvingFixKey != null || _resolvingBatch || _accessModalOpen;

  bool get _canPlan => switch (_kind) {
    'link' => _linkId != null,
    'server' => _targetDeviceId != null,
    _ =>
      _sourceDeviceId != null &&
          _targetDeviceId != null &&
          _sourceDeviceId != _targetDeviceId,
  };

  @override
  void initState() {
    super.initState();
    _kind = widget.initialDraft?.kind ?? 'server';
    _linkId = widget.initialDraft?.linkId;
    _sourceDeviceId = widget.initialDraft?.sourceDeviceId;
    _targetDeviceId = widget.initialDraft?.targetDeviceId;

    final devices = ref.read(devicesProvider).items;
    final edges = _nonVirtualEdges(ref.read(topologyProvider).edges);
    _syncEndpointsForCurrentMode(devices, edges);
    Future<void>.microtask(_triggerAutoPlanIfNeeded);
  }

  @override
  void dispose() {
    _limitDebounce?.cancel();
    _limitCtrl.dispose();
    super.dispose();
  }

  List<TopologyEdgeModel> _nonVirtualEdges(List<TopologyEdgeModel> raw) => raw
      .where((TopologyEdgeModel e) => !e.isVirtualHierarchy)
      .toList(growable: false);

  bool _syncEndpointsForCurrentMode(
    List<DeviceModel> devices,
    List<TopologyEdgeModel> edges,
  ) {
    var changed = false;
    if (_kind == 'link') {
      if (_linkId != null && !edges.any((e) => e.id == _linkId)) {
        _linkId = null;
        changed = true;
      }
      if (_linkId == null && edges.length == 1) {
        _linkId = edges.first.id;
        changed = true;
      }
      return changed;
    }

    if (_kind != 'server') {
      final sources = filterLinkTestSourceDevices(devices);
      if (_sourceDeviceId != null &&
          !sources.any((d) => d.id == _sourceDeviceId)) {
        _sourceDeviceId = null;
        changed = true;
      }
      if (_sourceDeviceId == null && sources.length == 1) {
        _sourceDeviceId = sources.first.id;
        changed = true;
      }
    }

    final targets = filterLinkTestTargetDevices(
      devices,
      kind: _kind,
      excludeSourceId: _kind != 'server' ? _sourceDeviceId : null,
    );
    if (_targetDeviceId != null &&
        !targets.any((d) => d.id == _targetDeviceId)) {
      _targetDeviceId = null;
      changed = true;
    }
    if (_targetDeviceId == null && targets.length == 1) {
      _targetDeviceId = targets.first.id;
      changed = true;
    }
    return changed;
  }

  String _requestSignature() =>
      '$_kind|${_linkId ?? ''}|${_sourceDeviceId ?? ''}|${_targetDeviceId ?? ''}|'
      '$_direction|$_durationS|$_streams|${_limitCtrl.text.trim()}';

  void _triggerAutoPlanIfNeeded() {
    if (!mounted || !_canPlan) return;
    final sig = _requestSignature();
    if (_lastAutoPlannedSignature == sig) return;
    _lastAutoPlannedSignature = sig;
    _runPlan(clearFeedback: false);
  }

  Map<String, dynamic> _buildRequest() => <String, dynamic>{
    'kind': _kind,
    if (_kind == 'link' && _linkId != null) 'linkId': _linkId,
    if (_kind != 'link' && _kind != 'server' && _sourceDeviceId != null)
      'sourceDeviceId': _sourceDeviceId,
    if (_kind != 'link' && _targetDeviceId != null)
      'targetDeviceId': _targetDeviceId,
    'spec': <String, dynamic>{
      'durationS': _durationS,
      'direction': _direction,
      'streams': _streams,
      if (double.tryParse(_limitCtrl.text.trim()) != null)
        'limitMbps': double.parse(_limitCtrl.text.trim()),
    },
  };

  String? _platformHintForDevice(int deviceId) {
    final device = ref
        .read(devicesProvider)
        .items
        .where((d) => d.id == deviceId)
        .firstOrNull;
    return device?.effectiveOperatingSystem ??
        device?.operatingSystem ??
        device?.vendor;
  }

  String _formatActionError(Object err, {int? deviceId}) {
    final platform = deviceId != null ? _platformHintForDevice(deviceId) : null;
    if (isDeviceAuthFailure(err)) {
      return formatDeviceAuthFailureMessage(err, platformOrOs: platform);
    }
    if (err is TimeoutException && (err.message?.trim().isNotEmpty ?? false)) {
      return err.message!.trim();
    }
    return formatDeviceAccessError(err, platformOrOs: platform);
  }

  Future<LinkTestPlanModel?> _runPlan({bool clearFeedback = false}) async {
    if (!_canPlan) return null;
    final epoch = ++_planEpoch;
    _lastAutoPlannedSignature = _requestSignature();
    setState(() {
      _planning = true;
      _error = null;
      if (clearFeedback) _feedbackMessage = null;
    });
    try {
      final plan = await ref
          .read(linkTestsProvider.notifier)
          .plan(_buildRequest());
      if (!mounted || epoch != _planEpoch) return null;
      setState(() {
        _plan = plan;
        _planning = false;
      });
      return plan;
    } catch (err) {
      if (!mounted || epoch != _planEpoch) return null;
      setState(() {
        _error = _formatActionError(err);
        _planning = false;
      });
      return null;
    }
  }

  Future<void> _startTest() async {
    setState(() {
      _starting = true;
      _error = null;
    });
    try {
      final id = await ref
          .read(linkTestsProvider.notifier)
          .start(_buildRequest());
      if (mounted) widget.onStarted(id);
    } catch (err) {
      if (mounted) setState(() => _error = _formatActionError(err));
    } finally {
      if (mounted) setState(() => _starting = false);
    }
  }

  Future<bool> _openDeviceAccessModal({
    required int deviceId,
    String? deviceName,
    String transport = 'ssh',
    String? message,
  }) async {
    if (!ref.read(authProvider).canManagePlugins) {
      throw StateError(
        'Somente administradores podem configurar credenciais de acesso no equipamento.',
      );
    }
    if (mounted) setState(() => _accessModalOpen = true);
    try {
      return await showDeviceAccessDialog(
        context: context,
        ref: ref,
        deviceId: deviceId,
        deviceName: deviceName,
        platformHint: _platformHintForDevice(deviceId),
        initialTransport: transport,
        bannerMessage: message,
      );
    } finally {
      if (mounted) setState(() => _accessModalOpen = false);
    }
  }

  Future<T?> _runWithModalDeviceAccess<T>({
    required int fallbackDeviceId,
    String? fallbackDeviceName,
    required Future<T> Function() action,
  }) async {
    try {
      return await action();
    } catch (err) {
      if (!isMissingDeviceCredentialError(err)) rethrow;
      if (!mounted) return null;
      final saved = await _openDeviceAccessModal(
        deviceId: fallbackDeviceId,
        deviceName: fallbackDeviceName,
        transport: 'ssh',
        message: _formatActionError(err, deviceId: fallbackDeviceId),
      );
      if (!saved || !mounted) return null;
      return action();
    }
  }

  Future<_FixStepOutcome> _applySingleFix(LinkTestFixModel fix) async {
    switch (fix.kind) {
      case 'install_plugin':
        if (fix.pluginId == null) {
          throw StateError(
            'Identificador do plugin ausente para ${fix.deviceName}.',
          );
        }
        final installed = await _runWithModalDeviceAccess<bool>(
          fallbackDeviceId: fix.deviceId,
          fallbackDeviceName: fix.deviceName,
          action: () async {
            await ref
                .read(linkTestsProvider.notifier)
                .installDevicePlugin(fix.deviceId, fix.pluginId!);
            return true;
          },
        );
        if (installed != true) return _FixStepOutcome.cancelled;
        return _FixStepOutcome.applied(
          'Plugin ativado com sucesso em ${fix.deviceName}!',
        );

      case 'add_credential':
        final saved = await _openDeviceAccessModal(
          deviceId: fix.deviceId,
          deviceName: fix.deviceName,
          transport: fix.transport ?? 'ssh',
          message: fix.message,
        );
        if (!saved) return _FixStepOutcome.cancelled;
        return _FixStepOutcome.applied(
          'Acesso configurado em ${fix.deviceName}!',
        );

      case 'install_package':
        if (fix.pluginId == null) {
          throw StateError(
            'Plugin de preparação não informado para ${fix.deviceName}.',
          );
        }
        final pkgLabel = fix.packageLabel;
        final runId = await _runWithModalDeviceAccess<int>(
          fallbackDeviceId: fix.deviceId,
          fallbackDeviceName: fix.deviceName,
          action: () => ref
              .read(linkTestsProvider.notifier)
              .runDevicePluginAction(
                deviceId: fix.deviceId,
                pluginId: fix.pluginId!,
                action: fix.action ?? 'prepare',
                params: <String, dynamic>{
                  if (fix.engine != null) 'engine': fix.engine,
                  if (fix.role != null) 'role': fix.role,
                },
                confirmWrite: true,
              ),
        );
        if (runId == null) return _FixStepOutcome.cancelled;
        if (runId <= 0) {
          throw StateError(
            'Não foi possível iniciar a instalação de $pkgLabel em ${fix.deviceName}.',
          );
        }
        final run = await ref
            .read(linkTestsProvider.notifier)
            .waitForPluginRun(runId)
            .timeout(
              const Duration(seconds: 90),
              onTimeout: () => throw TimeoutException(
                'Tempo esgotado aguardando a instalação de $pkgLabel em ${fix.deviceName}.',
              ),
            );
        if (run.status != 'succeeded') {
          final reason = (run.error != null && run.error!.trim().isNotEmpty)
              ? run.error!.trim()
              : 'Falha ao instalar pacote no equipamento.';
          throw StateError(reason);
        }
        return _FixStepOutcome.applied(
          'Pacote $pkgLabel instalado com sucesso em ${fix.deviceName}!',
        );

      default:
        throw StateError('Ação de preparação desconhecida (${fix.kind}).');
    }
  }

  void _applyPostFixFeedback({
    required String stepSummary,
    required LinkTestPlanModel? nextPlan,
    LinkTestFixModel? resolvedFix,
  }) {
    if (nextPlan == null) return;
    if (nextPlan.isReady && nextPlan.fixes.isEmpty) {
      _feedbackTone = SemanticTone.success;
      _feedbackMessage =
          '$stepSummary Pré-requisitos concluídos! O teste já pode ser iniciado.';
      return;
    }
    if (nextPlan.fixes.isNotEmpty) {
      final stillSame =
          resolvedFix != null &&
          nextPlan.fixes.any((f) => f.uniqueKey == resolvedFix.uniqueKey);
      if (stillSame) {
        _feedbackTone = SemanticTone.warning;
        _feedbackMessage =
            'A etapa ainda consta pendente: ${humanizeLinkTestText(nextPlan.fixes.first.message)}';
      } else {
        _feedbackTone = SemanticTone.info;
        _feedbackMessage =
            '$stepSummary Próximo passo: ${humanizeLinkTestText(nextPlan.fixes.first.message)}';
      }
      return;
    }
    if (nextPlan.blocking.isNotEmpty) {
      _feedbackTone = SemanticTone.error;
      _feedbackMessage =
          '$stepSummary Impedimento encontrado: ${humanizeLinkTestText(nextPlan.blocking.first)}';
    }
  }

  Future<void> _resolveFix(LinkTestFixModel fix) async {
    if (_isResolvingAny) return;
    setState(() {
      _resolvingFixKey = fix.uniqueKey;
      _error = null;
      _feedbackMessage = null;
    });
    try {
      final outcome = await _applySingleFix(fix);
      if (!mounted) return;
      if (outcome.isCancelled) {
        setState(() => _resolvingFixKey = null);
        return;
      }
      setState(() => _resolvingFixKey = null);
      final nextPlan = await _runPlan(clearFeedback: false);
      if (!mounted) return;
      setState(() {
        _applyPostFixFeedback(
          stepSummary: outcome.summary,
          nextPlan: nextPlan,
          resolvedFix: fix,
        );
      });
    } catch (err) {
      if (!mounted) return;
      final formatted = _formatActionError(err, deviceId: fix.deviceId);
      setState(() {
        _resolvingFixKey = null;
        _feedbackTone = SemanticTone.error;
        _feedbackMessage = formatted;
      });
      if (isDeviceAuthFailure(err)) {
        await showDeviceAuthFailureDialog(
          context,
          err,
          platformOrOs: _platformHintForDevice(fix.deviceId),
        );
      }
    }
  }

  Future<void> _resolveAllFixes() async {
    final fixes = List<LinkTestFixModel>.from(
      _plan?.fixes ?? const <LinkTestFixModel>[],
    );
    if (fixes.isEmpty || _isResolvingAny) return;

    setState(() {
      _resolvingBatch = true;
      _error = null;
      _feedbackMessage = null;
    });

    var appliedCount = 0;
    LinkTestFixModel? currentFix;

    try {
      for (var i = 0; i < fixes.length; i++) {
        currentFix = fixes[i];
        if (!mounted) return;
        setState(() {
          _resolvingFixKey = currentFix!.uniqueKey;
          _resolvingBatchProgress =
              'Preparando ${i + 1} de ${fixes.length}: ${currentFix.deviceName}...';
        });
        final outcome = await _applySingleFix(currentFix);
        if (!mounted) return;
        if (outcome.isCancelled) {
          setState(() {
            _resolvingBatch = false;
            _resolvingFixKey = null;
            _resolvingBatchProgress = null;
          });
          if (appliedCount > 0) {
            final nextPlan = await _runPlan(clearFeedback: false);
            if (mounted && nextPlan != null) {
              setState(() {
                _applyPostFixFeedback(
                  stepSummary:
                      '$appliedCount de ${fixes.length} etapa(s) concluída(s).',
                  nextPlan: nextPlan,
                );
              });
            }
          }
          return;
        }
        appliedCount++;
      }

      if (!mounted) return;
      setState(() {
        _resolvingBatch = false;
        _resolvingFixKey = null;
        _resolvingBatchProgress = null;
      });

      final nextPlan = await _runPlan(clearFeedback: false);
      if (!mounted) return;
      setState(() {
        _applyPostFixFeedback(
          stepSummary:
              'Preparação concluída ($appliedCount/${fixes.length} etapas)!',
          nextPlan: nextPlan,
        );
      });
    } catch (err) {
      if (!mounted) return;
      final failedDeviceId = currentFix?.deviceId;
      final failedName = currentFix?.deviceName;
      final formatted = _formatActionError(err, deviceId: failedDeviceId);
      setState(() {
        _resolvingBatch = false;
        _resolvingFixKey = null;
        _resolvingBatchProgress = null;
      });
      if (appliedCount > 0) {
        await _runPlan(clearFeedback: false);
      }
      if (!mounted) return;
      setState(() {
        _feedbackTone = SemanticTone.error;
        _feedbackMessage = failedName != null && failedName.isNotEmpty
            ? 'Falha ao preparar $failedName: $formatted'
            : formatted;
      });
      if (isDeviceAuthFailure(err)) {
        await showDeviceAuthFailureDialog(
          context,
          err,
          platformOrOs: failedDeviceId != null
              ? _platformHintForDevice(failedDeviceId)
              : null,
        );
      }
    }
  }

  IconData _fixIcon(String kind) => switch (kind) {
    'install_plugin' => Icons.extension_outlined,
    'add_credential' => Icons.key_outlined,
    'install_package' => Icons.download_outlined,
    _ => Icons.build_outlined,
  };

  IconData _feedbackIcon(SemanticTone tone) => switch (tone) {
    SemanticTone.success => Icons.check_circle_outline,
    SemanticTone.error => Icons.error_outline,
    SemanticTone.warning => Icons.warning_amber_rounded,
    _ => Icons.info_outline,
  };

  Widget _buildFixItem(BuildContext context, LinkTestFixModel fix) {
    final isThisResolving =
        _resolvingFixKey == fix.uniqueKey && !_accessModalOpen;
    final button = FilledButton.tonalIcon(
      key: ValueKey<String>('link-test-fix-btn-${fix.uniqueKey}'),
      onPressed: (_isResolvingAny || _planning || _starting)
          ? null
          : () => _resolveFix(fix),
      icon: isThisResolving
          ? const SizedBox(
              width: 15,
              height: 15,
              child: CircularProgressIndicator(strokeWidth: 2),
            )
          : Icon(_fixIcon(fix.kind), size: 17),
      label: Text(
        isThisResolving ? 'Resolvendo...' : fix.actionButtonLabel,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
      ),
    );

    return LayoutBuilder(
      builder: (context, constraints) {
        final messageWidget = Text(
          humanizeLinkTestText(fix.message),
          style: context.textTheme.bodyMedium?.copyWith(
            fontWeight: FontWeight.w600,
          ),
        );
        if (constraints.maxWidth < 460) {
          return NestedPanel(
            padding: const EdgeInsets.all(10),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                messageWidget,
                const SizedBox(height: 8),
                SymmetricalActionBar(children: <Widget>[button]),
              ],
            ),
          );
        }
        return NestedPanel(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
          child: Row(
            children: <Widget>[
              Expanded(child: messageWidget),
              const SizedBox(width: 10),
              button,
            ],
          ),
        );
      },
    );
  }

  Widget _buildPlanSection(BuildContext context, LinkTestPlanModel plan) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (plan.isReady) ...<Widget>[
          NestedPanel(
            key: const Key('link-test-ready-panel'),
            tone: SemanticTone.success,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Row(
                  children: <Widget>[
                    Icon(
                      Icons.check_circle_outline,
                      size: 20,
                      color: context.semanticColors.success,
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        'Pronto para iniciar medição',
                        style: context.textTheme.titleSmall?.copyWith(
                          fontWeight: FontWeight.w800,
                        ),
                      ),
                    ),
                    if (plan.estimatedSeconds > 0) ...<Widget>[
                      const SizedBox(width: 8),
                      SemanticChip(
                        label: '~${plan.estimatedSeconds}s',
                        icon: Icons.timer_outlined,
                        tone: SemanticTone.success,
                        compact: true,
                      ),
                    ],
                  ],
                ),
                if (plan.legs.isNotEmpty) ...<Widget>[
                  const SizedBox(height: 8),
                  for (final leg in plan.legs)
                    Padding(
                      padding: const EdgeInsets.only(top: 4),
                      child: Row(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          Icon(
                            Icons.route_outlined,
                            size: 16,
                            color: context.semanticColors.success,
                          ),
                          const SizedBox(width: 6),
                          Expanded(
                            child: Text(
                              leg.planSummary,
                              style: context.textTheme.bodySmall?.copyWith(
                                fontWeight: FontWeight.w600,
                              ),
                            ),
                          ),
                        ],
                      ),
                    ),
                ],
                for (final w in plan.warnings)
                  Padding(
                    padding: const EdgeInsets.only(top: 6),
                    child: Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Icon(
                          Icons.warning_amber_rounded,
                          size: 16,
                          color: context.semanticColors.warning,
                        ),
                        const SizedBox(width: 6),
                        Expanded(
                          child: Text(
                            humanizeLinkTestText(w),
                            style: context.textTheme.bodySmall,
                          ),
                        ),
                      ],
                    ),
                  ),
              ],
            ),
          ),
        ],
        if (plan.fixes.isNotEmpty) ...<Widget>[
          if (plan.isReady) const SizedBox(height: 10),
          NestedPanel(
            key: const Key('link-test-fixes-panel'),
            tone: SemanticTone.warning,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Row(
                  children: <Widget>[
                    Icon(
                      Icons.build_circle_outlined,
                      size: 20,
                      color: context.semanticColors.warning,
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        'Preparação necessária antes do teste',
                        style: context.textTheme.titleSmall?.copyWith(
                          fontWeight: FontWeight.w800,
                        ),
                      ),
                    ),
                    const SizedBox(width: 8),
                    SemanticChip(
                      label: '${plan.fixes.length} pendente(s)',
                      tone: SemanticTone.warning,
                      compact: true,
                    ),
                  ],
                ),
                const SizedBox(height: 6),
                Text(
                  _resolvingBatchProgress ??
                      'Conclua os pré-requisitos abaixo no equipamento antes de iniciar a medição de velocidade.',
                  style: context.textTheme.bodySmall,
                ),
                if (plan.fixes.length >= 2) ...<Widget>[
                  const SizedBox(height: 10),
                  SymmetricalActionBar(
                    children: <Widget>[
                      FilledButton.icon(
                        key: const Key('link-test-resolve-all-btn'),
                        onPressed: (_isResolvingAny || _planning || _starting)
                            ? null
                            : _resolveAllFixes,
                        icon: (_resolvingBatch && !_accessModalOpen)
                            ? const SizedBox(
                                width: 16,
                                height: 16,
                                child: CircularProgressIndicator(
                                  strokeWidth: 2,
                                ),
                              )
                            : const Icon(
                                Icons.auto_fix_high_outlined,
                                size: 18,
                              ),
                        label: Text(
                          _resolvingBatch
                              ? (_resolvingBatchProgress ??
                                    'Resolvendo pendências...')
                              : 'Resolver tudo (${plan.fixes.length})',
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                    ],
                  ),
                ],
                const SizedBox(height: 8),
                for (var i = 0; i < plan.fixes.length; i++) ...<Widget>[
                  if (i > 0) const SizedBox(height: 8),
                  _buildFixItem(context, plan.fixes[i]),
                ],
                if (!plan.isReady && plan.warnings.isNotEmpty) ...<Widget>[
                  const SizedBox(height: 6),
                  for (final w in plan.warnings)
                    Padding(
                      padding: const EdgeInsets.only(top: 4),
                      child: Row(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          Icon(
                            Icons.warning_amber_rounded,
                            size: 16,
                            color: context.semanticColors.warning,
                          ),
                          const SizedBox(width: 6),
                          Expanded(
                            child: Text(
                              humanizeLinkTestText(w),
                              style: context.textTheme.bodySmall,
                            ),
                          ),
                        ],
                      ),
                    ),
                ],
              ],
            ),
          ),
        ],
        if (!plan.isReady && plan.blocking.isNotEmpty) ...<Widget>[
          if (plan.fixes.isNotEmpty) const SizedBox(height: 10),
          NestedPanel(
            key: const Key('link-test-blocking-panel'),
            tone: SemanticTone.error,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Row(
                  children: <Widget>[
                    Icon(
                      Icons.block_outlined,
                      size: 20,
                      color: context.colorScheme.error,
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        'Não é possível medir este caminho',
                        style: context.textTheme.titleSmall?.copyWith(
                          fontWeight: FontWeight.w800,
                        ),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 6),
                for (final b in plan.blocking)
                  Padding(
                    padding: const EdgeInsets.only(top: 4),
                    child: Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Icon(
                          Icons.error_outline,
                          size: 16,
                          color: context.colorScheme.error,
                        ),
                        const SizedBox(width: 6),
                        Expanded(
                          child: Text(
                            humanizeLinkTestText(b),
                            style: context.textTheme.bodySmall?.copyWith(
                              color: context.colorScheme.error,
                              fontWeight: FontWeight.w600,
                            ),
                          ),
                        ),
                      ],
                    ),
                  ),
              ],
            ),
          ),
        ],
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    final devices = ref.watch(devicesProvider).items;
    final edges = _nonVirtualEdges(ref.watch(topologyProvider).edges);

    final sources = filterLinkTestSourceDevices(devices);
    final targets = filterLinkTestTargetDevices(
      devices,
      kind: _kind,
      excludeSourceId: _kind != 'server' ? _sourceDeviceId : null,
    );

    final canAutoFillDeferred =
        (_kind == 'link' && _linkId == null && edges.length == 1) ||
        (_kind != 'link' &&
            _kind != 'server' &&
            _sourceDeviceId == null &&
            sources.length == 1) ||
        (_kind != 'link' && _targetDeviceId == null && targets.length == 1);
    if (canAutoFillDeferred) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted) return;
        if (_syncEndpointsForCurrentMode(devices, edges)) {
          setState(() {});
          _triggerAutoPlanIfNeeded();
        }
      });
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        LayoutBuilder(
          builder: (context, constraints) {
            final isNarrow = constraints.maxWidth < 460;
            return SegmentedButton<String>(
              showSelectedIcon: false,
              style: SegmentedButton.styleFrom(
                visualDensity: isNarrow
                    ? VisualDensity.compact
                    : VisualDensity.standard,
                padding: EdgeInsets.symmetric(
                  horizontal: isNarrow ? 6 : 12,
                  vertical: 8,
                ),
              ),
              segments: const <ButtonSegment<String>>[
                ButtonSegment(
                  value: 'server',
                  label: Text(
                    'Servidor',
                    maxLines: 1,
                    softWrap: false,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                ButtonSegment(
                  value: 'link',
                  label: Text(
                    'Enlace',
                    maxLines: 1,
                    softWrap: false,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                ButtonSegment(
                  value: 'end_to_end',
                  label: Text(
                    'A -> B',
                    maxLines: 1,
                    softWrap: false,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                ButtonSegment(
                  value: 'path',
                  label: Text(
                    'Caminho',
                    maxLines: 1,
                    softWrap: false,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              ],
              selected: <String>{_kind},
              onSelectionChanged: (s) {
                setState(() {
                  _kind = s.first;
                  _plan = null;
                  _error = null;
                  _feedbackMessage = null;
                  _syncEndpointsForCurrentMode(devices, edges);
                });
                _triggerAutoPlanIfNeeded();
              },
            );
          },
        ),
        const SizedBox(height: 12),
        if (_kind == 'link')
          AppDropdownFormField<int?>(
            initialValue: edges.any((e) => e.id == _linkId) ? _linkId : null,
            decoration: const InputDecoration(labelText: 'Enlace da topologia'),
            items: edges
                .map(
                  (e) => DropdownMenuItem<int?>(
                    value: e.id,
                    child: Text(
                      '${e.sourceDeviceName ?? '#${e.sourceDeviceId}'} <-> ${e.targetDeviceName ?? '#${e.targetDeviceId}'}',
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                )
                .toList(growable: false),
            onChanged: (v) {
              setState(() {
                _linkId = v;
                _feedbackMessage = null;
              });
              _triggerAutoPlanIfNeeded();
            },
          )
        else ...<Widget>[
          if (_kind != 'server') ...<Widget>[
            AppDropdownFormField<int?>(
              initialValue: sources.any((d) => d.id == _sourceDeviceId)
                  ? _sourceDeviceId
                  : null,
              decoration: const InputDecoration(
                labelText: 'Equipamento origem',
              ),
              items: sources
                  .map(
                    (DeviceModel d) => DropdownMenuItem<int?>(
                      value: d.id,
                      child: Text(d.name, overflow: TextOverflow.ellipsis),
                    ),
                  )
                  .toList(growable: false),
              onChanged: (v) {
                setState(() {
                  _sourceDeviceId = v;
                  if (_targetDeviceId == v) _targetDeviceId = null;
                  final nextTargets = filterLinkTestTargetDevices(
                    devices,
                    kind: _kind,
                    excludeSourceId: v,
                  );
                  if (_targetDeviceId == null && nextTargets.length == 1) {
                    _targetDeviceId = nextTargets.first.id;
                  }
                  _feedbackMessage = null;
                });
                _triggerAutoPlanIfNeeded();
              },
            ),
            const SizedBox(height: 10),
          ],
          AppDropdownFormField<int?>(
            initialValue: targets.any((d) => d.id == _targetDeviceId)
                ? _targetDeviceId
                : null,
            decoration: const InputDecoration(labelText: 'Equipamento destino'),
            items: targets
                .map(
                  (DeviceModel d) => DropdownMenuItem<int?>(
                    value: d.id,
                    child: Text(d.name, overflow: TextOverflow.ellipsis),
                  ),
                )
                .toList(growable: false),
            onChanged: (v) {
              setState(() {
                _targetDeviceId = v;
                _feedbackMessage = null;
              });
              _triggerAutoPlanIfNeeded();
            },
          ),
        ],
        const SizedBox(height: 12),
        LayoutBuilder(
          builder: (context, constraints) {
            final directionField = AppDropdownFormField<String>(
              initialValue: _direction,
              decoration: const InputDecoration(labelText: 'Sentido'),
              items: const <DropdownMenuItem<String>>[
                DropdownMenuItem(
                  value: 'both',
                  child: Text('Ida e Volta', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem(
                  value: 'forward',
                  child: Text('Somente Ida', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem(
                  value: 'reverse',
                  child: Text('Somente Volta', overflow: TextOverflow.ellipsis),
                ),
              ],
              onChanged: (v) {
                setState(() => _direction = v ?? 'both');
                _triggerAutoPlanIfNeeded();
              },
            );
            final durationField = AppDropdownFormField<int>(
              initialValue: _durationS,
              decoration: const InputDecoration(labelText: 'Duração'),
              items: const <DropdownMenuItem<int>>[
                DropdownMenuItem(
                  value: 5,
                  child: Text('5 s', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem(
                  value: 10,
                  child: Text('10 s', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem(
                  value: 20,
                  child: Text('20 s', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem(
                  value: 30,
                  child: Text('30 s', overflow: TextOverflow.ellipsis),
                ),
              ],
              onChanged: (v) {
                setState(() => _durationS = v ?? 10);
                _triggerAutoPlanIfNeeded();
              },
            );
            final streamsField = AppDropdownFormField<int>(
              initialValue: _streams,
              decoration: const InputDecoration(labelText: 'Streams'),
              items: const <DropdownMenuItem<int>>[
                DropdownMenuItem(
                  value: 1,
                  child: Text('1', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem(
                  value: 4,
                  child: Text('4', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem(
                  value: 8,
                  child: Text('8', overflow: TextOverflow.ellipsis),
                ),
              ],
              onChanged: (v) {
                setState(() => _streams = v ?? 4);
                _triggerAutoPlanIfNeeded();
              },
            );

            if (constraints.maxWidth < 620) {
              return Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  directionField,
                  const SizedBox(height: 12),
                  Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      Expanded(child: durationField),
                      const SizedBox(width: 10),
                      Expanded(child: streamsField),
                    ],
                  ),
                ],
              );
            }

            return ResponsiveFieldRow(
              breakpoint: 620,
              children: <Widget>[directionField, durationField, streamsField],
            );
          },
        ),
        const SizedBox(height: 10),
        TextField(
          controller: _limitCtrl,
          keyboardType: TextInputType.number,
          onChanged: (_) {
            _limitDebounce?.cancel();
            _limitDebounce = Timer(const Duration(milliseconds: 450), () {
              if (mounted) _triggerAutoPlanIfNeeded();
            });
          },
          decoration: const InputDecoration(
            labelText: 'Limite de banda opcional (Mbps)',
            hintText: 'Vazio = sem limite',
          ),
        ),
        if (_feedbackMessage != null) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(
            key: const Key('link-test-fix-feedback'),
            tone: _feedbackTone,
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Icon(_feedbackIcon(_feedbackTone), size: 19),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    _feedbackMessage!,
                    style: context.textTheme.bodySmall?.copyWith(
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                ),
                IconButton(
                  visualDensity: VisualDensity.compact,
                  tooltip: 'Fechar aviso',
                  padding: EdgeInsets.zero,
                  constraints: const BoxConstraints(
                    minWidth: 28,
                    minHeight: 28,
                  ),
                  icon: const Icon(Icons.close, size: 16),
                  onPressed: () => setState(() => _feedbackMessage = null),
                ),
              ],
            ),
          ),
        ],
        if (_error != null) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
        ],
        if (_planning && _plan == null) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(
            tone: SemanticTone.info,
            child: Row(
              children: <Widget>[
                const SizedBox(
                  width: 16,
                  height: 16,
                  child: CircularProgressIndicator(strokeWidth: 2),
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: Text(
                    'Verificando prontidão e rota do teste...',
                    style: context.textTheme.bodySmall,
                  ),
                ),
              ],
            ),
          ),
        ] else if (_plan != null) ...<Widget>[
          const SizedBox(height: 12),
          _buildPlanSection(context, _plan!),
        ],
        const SizedBox(height: 14),
        SymmetricalActionBar(
          children: <Widget>[
            OutlinedButton.icon(
              key: const Key('link-test-revalidate-btn'),
              onPressed:
                  (_planning || _starting || _isResolvingAny || !_canPlan)
                  ? null
                  : () => _runPlan(clearFeedback: true),
              icon: _planning
                  ? const SizedBox(
                      width: 15,
                      height: 15,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(Icons.refresh_rounded, size: 18),
              label: Text(
                _planning ? 'Verificando...' : 'Verificar novamente',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
            ),
            if (_plan?.isReady == true)
              FilledButton.icon(
                key: const Key('link-test-start-btn'),
                onPressed: (_starting || _planning || _isResolvingAny)
                    ? null
                    : _startTest,
                icon: _starting
                    ? const SizedBox(
                        width: 15,
                        height: 15,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(Icons.play_arrow_rounded, size: 20),
                label: Text(
                  _starting ? 'Iniciando...' : 'Iniciar Teste',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
          ],
        ),
      ],
    );
  }
}
