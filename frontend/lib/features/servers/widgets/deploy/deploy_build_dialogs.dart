import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/deploy_models.dart';
import 'package:netmonitor_app/features/servers/providers/deploys_provider.dart';
import 'package:netmonitor_app/features/servers/utils/server_utils.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Diálogo para disparar compilação no servidor (`DeployBuildStartDialog`).
Future<void> showDeployBuildStartDialog(
  BuildContext context, {
  required DeployApplicationView application,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Compilar ${application.name}',
    subtitle: 'Compila no builder dedicado e envia para o Registry interno',
    builder: (_) => _DeployBuildStartBody(application: application),
  );
}

class _DeployBuildStartBody extends ConsumerStatefulWidget {
  const _DeployBuildStartBody({required this.application});
  final DeployApplicationView application;

  @override
  ConsumerState<_DeployBuildStartBody> createState() =>
      _DeployBuildStartBodyState();
}

class _DeployBuildStartBodyState extends ConsumerState<_DeployBuildStartBody> {
  late final TextEditingController _refCtrl;
  final TextEditingController _commitCtrl = TextEditingController();
  bool _noCache = false;
  BuilderReadiness? _builderStatus;
  bool _submitting = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    final firstRef = widget.application.targets.isNotEmpty
        ? refShortName(widget.application.targets.first.reference)
        : 'main';
    _refCtrl = TextEditingController(
      text: firstRef.contains('*') ? 'main' : firstRef,
    );
    final probeId =
        widget.application.buildProbeId ??
        widget.application.targets.firstOrNull?.agent?.id;
    if (probeId != null) {
      Future<void>.microtask(() async {
        try {
          final status = await ref
              .read(deploysProvider.notifier)
              .builderReadiness(probeId);
          if (mounted) setState(() => _builderStatus = status);
        } catch (_) {}
      });
    }
  }

  @override
  void dispose() {
    _refCtrl.dispose();
    _commitCtrl.dispose();
    super.dispose();
  }

  Future<void> _start() async {
    setState(() {
      _submitting = true;
      _error = null;
    });
    try {
      final build = await ref
          .read(deploysProvider.notifier)
          .startBuild(
            widget.application.id,
            reference: _refCtrl.text.trim(),
            commit: _commitCtrl.text.trim(),
            noCache: _noCache,
          );
      if (!mounted) return;
      Navigator.of(context).pop();
      await showDeployBuildDialog(context, buildId: build.id, canAdmin: true);
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = describeError(err, fallback: 'Erro ao iniciar compilação.');
        _submitting = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (_builderStatus != null) ...<Widget>[
          NestedPanel(
            tone: _builderStatus!.builderReady
                ? SemanticTone.success
                : SemanticTone.warning,
            child: Text(
              _builderStatus!.builderReady
                  ? 'Builder pronto · Buildx ${_builderStatus!.buildxVersion ?? ''} · '
                        '${formatBytes(_builderStatus!.freeBytes)} livres · ${_builderStatus!.cpus} vCPUs'
                  : 'Builder ainda não está pronto neste agente.',
              style: context.textTheme.bodySmall,
            ),
          ),
          const SizedBox(height: 12),
        ],
        TextField(
          controller: _refCtrl,
          decoration: const InputDecoration(labelText: 'Branch ou tag'),
        ),
        const SizedBox(height: 10),
        TextField(
          controller: _commitCtrl,
          decoration: const InputDecoration(
            labelText: 'Commit SHA exato (opcional)',
          ),
        ),
        const SizedBox(height: 8),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          value: _noCache,
          title: const Text('Compilar sem cache (--no-cache)'),
          onChanged: (v) => setState(() => _noCache = v),
        ),
        if (_error != null) ...<Widget>[
          const SizedBox(height: 10),
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
        ],
        const SizedBox(height: 14),
        Align(
          alignment: Alignment.centerRight,
          child: FilledButton.icon(
            onPressed: _submitting ? null : _start,
            icon: const Icon(Icons.build_outlined),
            label: const Text('Iniciar compilação'),
          ),
        ),
      ],
    );
  }
}

/// Diálogo de compilação no servidor (`DeployBuildDialog` — ADR 023).
Future<void> showDeployBuildDialog(
  BuildContext context, {
  required int buildId,
  required bool canAdmin,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Compilação #$buildId',
    subtitle: 'Builder BuildKit isolado · progresso ao vivo via SSE',
    maxWidth: 860,
    builder: (_) =>
        _DeployBuildBody(initialBuildId: buildId, canAdmin: canAdmin),
  );
}

class _DeployBuildBody extends ConsumerStatefulWidget {
  const _DeployBuildBody({
    required this.initialBuildId,
    required this.canAdmin,
  });
  final int initialBuildId;
  final bool canAdmin;

  @override
  ConsumerState<_DeployBuildBody> createState() => _DeployBuildBodyState();
}

class _DeployBuildBodyState extends ConsumerState<_DeployBuildBody> {
  late int _buildId;
  bool _acting = false;
  bool _logLoaded = false;
  String? _loadError;
  DeploysNotifier? _deploysNotifier;

  @override
  void initState() {
    super.initState();
    _buildId = widget.initialBuildId;
    _deploysNotifier = ref.read(deploysProvider.notifier);
    Future<void>.microtask(_loadConsole);
  }

  Future<void> _loadConsole() async {
    if (!mounted) return;
    try {
      await ref.read(deploysProvider.notifier).openBuildConsole(_buildId);
      if (mounted) setState(() => _logLoaded = true);
    } catch (err) {
      if (mounted) {
        setState(() {
          _logLoaded = true;
          _loadError = describeError(
            err,
            fallback: 'Não foi possível carregar o log da compilação.',
          );
        });
      }
    }
  }

  @override
  void dispose() {
    final buildId = _buildId;
    final notifier = _deploysNotifier;
    if (notifier != null) {
      Future<void>.microtask(() => notifier.closeBuildConsole(buildId));
    }
    super.dispose();
  }

  Future<void> _cancel() async {
    if (_acting) return;
    final ok = await showConfirmActionDialog(
      context: context,
      title: 'Cancelar compilação #$_buildId?',
      message: 'A compilação em andamento no builder será interrompida.',
      confirmLabel: 'Cancelar compilação',
      tone: SemanticTone.warning,
    );
    if (!ok || !mounted) return;
    setState(() => _acting = true);
    try {
      await ref.read(deploysProvider.notifier).cancelBuild(_buildId);
    } catch (err) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Erro ao cancelar compilação: ${describeError(err)}'),
          ),
        );
      }
    } finally {
      if (mounted) setState(() => _acting = false);
    }
  }

  Future<void> _rebuild() async {
    setState(() => _acting = true);
    try {
      final next = await ref.read(deploysProvider.notifier).rebuild(_buildId);
      if (!mounted) return;
      ref.read(deploysProvider.notifier).closeBuildConsole(_buildId);
      setState(() {
        _buildId = next.id;
        _acting = false;
      });
      await ref.read(deploysProvider.notifier).openBuildConsole(_buildId);
    } catch (err) {
      if (!mounted) return;
      setState(() => _acting = false);
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Erro ao recompilar: ${describeError(err)}')),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final deploys = ref.watch(deploysProvider);
    final build = deploys.builds[_buildId];
    final lines = deploys.buildConsoles[_buildId] ?? const <String>[];

    if (build == null) {
      if (_loadError != null) {
        return ErrorStateView(message: _loadError!, onRetry: _loadConsole);
      }
      if (!_logLoaded) {
        return const LoadingStateView(message: 'Carregando compilação...');
      }
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          SemanticChip(
            label: 'Compilação #$_buildId (histórico)',
            tone: SemanticTone.secondary,
          ),
          const SizedBox(height: 12),
          CodeViewer(
            title: 'Console da compilação #$_buildId',
            code: lines.isEmpty ? '(sem saída registrada)' : lines.join('\n'),
          ),
        ],
      );
    }

    final statusVisual = deployBuildStatusMeta(build.status);
    final stages = computeBuildStageStates(build.stage, build.status);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: <Widget>[
            SemanticChip(
              label: statusVisual.label,
              tone: statusVisual.tone,
              icon: statusVisual.icon,
            ),
            SemanticChip(label: build.application, tone: SemanticTone.primary),
            SemanticChip(label: build.shortSha, tone: SemanticTone.secondary),
            if (build.currentImage != null && statusVisual.live)
              SemanticChip(label: build.currentImage!, tone: SemanticTone.info),
            if (build.durationMs != null)
              SemanticChip(
                label: formatElapsedMs(build.durationMs),
                tone: SemanticTone.info,
              ),
          ],
        ),
        const SizedBox(height: 12),
        Wrap(
          spacing: 6,
          runSpacing: 6,
          children: buildStagesOrder
              .map((s) {
                final st = stages[s.key] ?? 'todo';
                final tone = switch (st) {
                  'done' => SemanticTone.success,
                  'current' => SemanticTone.primary,
                  'failed' => SemanticTone.error,
                  _ => SemanticTone.secondary,
                };
                return SemanticChip(
                  label: s.label,
                  tone: tone,
                  icon: s.icon,
                  compact: true,
                );
              })
              .toList(growable: false),
        ),
        if (build.error != null && build.error!.isNotEmpty) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(tone: SemanticTone.error, child: Text(build.error!)),
        ],
        if (build.images.isNotEmpty) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(
            tone: SemanticTone.success,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text(
                  'Imagens armazenadas no Registry',
                  style: context.textTheme.titleSmall?.copyWith(
                    fontWeight: FontWeight.w700,
                  ),
                ),
                const SizedBox(height: 6),
                ...build.images.map(
                  (img) => Text(
                    '${img.repository} · ${img.shortDigest} · ${formatDecimalBytes(img.bytes)}',
                    style: context.textTheme.bodySmall?.copyWith(
                      fontFamily: 'monospace',
                    ),
                  ),
                ),
              ],
            ),
          ),
        ],
        if (widget.canAdmin) ...<Widget>[
          const SizedBox(height: 12),
          Wrap(
            spacing: 8,
            children: <Widget>[
              if (statusVisual.live)
                OutlinedButton.icon(
                  onPressed: _acting ? null : _cancel,
                  icon: const Icon(Icons.cancel_outlined),
                  label: const Text('Cancelar compilação'),
                )
              else
                FilledButton.tonalIcon(
                  onPressed: _acting ? null : _rebuild,
                  icon: const Icon(Icons.refresh),
                  label: const Text('Recompilar sem cache'),
                ),
            ],
          ),
        ],
        const SizedBox(height: 14),
        CodeViewer(
          title: 'Console da compilação (${lines.length} linhas)',
          code: lines.isEmpty
              ? '(Aguardando logs do builder...)'
              : lines.join('\n'),
          maxHeight: 280,
        ),
      ],
    );
  }
}

/// Diálogo de segredos de build (BuildKit `--secret`, nunca `--build-arg` — ADR 023).
Future<void> showDeployBuildSecretsDialog(
  BuildContext context, {
  required DeployApplicationView application,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Segredos de build · ${application.name}',
    subtitle:
        'Injetados somente via BuildKit --secret (nunca expostos na imagem ou nos logs)',
    maxWidth: 640,
    builder: (_) => _BuildSecretsBody(applicationId: application.id),
  );
}

class _BuildSecretsBody extends ConsumerStatefulWidget {
  const _BuildSecretsBody({required this.applicationId});
  final int applicationId;

  @override
  ConsumerState<_BuildSecretsBody> createState() => _BuildSecretsBodyState();
}

class _BuildSecretsBodyState extends ConsumerState<_BuildSecretsBody> {
  final TextEditingController _nameCtrl = TextEditingController();
  final TextEditingController _valueCtrl = TextEditingController();
  List<DeployBuildSecretView> _secrets = const <DeployBuildSecretView>[];
  bool _loading = true;
  bool _saving = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(_load);
  }

  @override
  void dispose() {
    _nameCtrl.dispose();
    _valueCtrl.dispose();
    super.dispose();
  }

  Future<void> _load() async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final list = await ref
          .read(deploysProvider.notifier)
          .buildSecrets(widget.applicationId);
      if (!mounted) return;
      setState(() {
        _secrets = list;
        _loading = false;
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = describeError(err, fallback: 'Erro ao carregar segredos.');
        _loading = false;
      });
    }
  }

  Future<void> _save() async {
    final name = _nameCtrl.text.trim();
    final value = _valueCtrl.text;
    if (!isValidEnvKey(name) || value.isEmpty) {
      setState(
        () => _error = 'Informe um nome válido (A-Z, 0-9, _) e um valor.',
      );
      return;
    }
    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      final list = await ref
          .read(deploysProvider.notifier)
          .saveBuildSecret(widget.applicationId, name, value);
      if (!mounted) return;
      _nameCtrl.clear();
      _valueCtrl.clear();
      setState(() {
        _secrets = list;
        _saving = false;
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = describeError(err, fallback: 'Erro ao salvar segredo.');
        _saving = false;
      });
    }
  }

  Future<void> _remove(String name) async {
    final ok = await showConfirmActionDialog(
      context: context,
      title: 'Remover segredo $name?',
      message: 'As próximas compilações não receberão mais este segredo.',
      confirmLabel: 'Remover',
      tone: SemanticTone.error,
    );
    if (!ok || !mounted) return;
    await ref
        .read(deploysProvider.notifier)
        .removeBuildSecret(widget.applicationId, name);
    if (mounted) await _load();
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        ResponsiveFieldRow(
          breakpoint: 480,
          spacing: 8,
          runSpacing: 10,
          children: <Widget>[
            TextField(
              controller: _nameCtrl,
              decoration: const InputDecoration(
                labelText: 'Nome do segredo',
                hintText: 'NPM_TOKEN',
              ),
            ),
            TextField(
              controller: _valueCtrl,
              obscureText: true,
              decoration: const InputDecoration(labelText: 'Valor'),
            ),
          ],
        ),
        const SizedBox(height: 10),
        Align(
          alignment: Alignment.centerRight,
          child: FilledButton(
            onPressed: _saving ? null : _save,
            child: const Text('Salvar'),
          ),
        ),
        if (_error != null) ...<Widget>[
          const SizedBox(height: 10),
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
        ],
        const SizedBox(height: 14),
        if (_loading)
          const LoadingStateView(message: 'Carregando segredos...')
        else if (_secrets.isEmpty)
          const EmptyStateView(title: 'Nenhum segredo de build cadastrado')
        else
          ..._secrets.map(
            (s) => ListTile(
              contentPadding: EdgeInsets.zero,
              leading: const Icon(Icons.vpn_key_outlined),
              title: Text(
                s.name,
                style: const TextStyle(
                  fontFamily: 'monospace',
                  fontWeight: FontWeight.w700,
                ),
              ),
              subtitle: Text('Atualizado em ${formatDateTime(s.updatedAt)}'),
              trailing: IconButton(
                icon: const Icon(Icons.delete_outline),
                onPressed: () => _remove(s.name),
              ),
            ),
          ),
      ],
    );
  }
}
