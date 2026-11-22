import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/config/server_config.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/deploy_models.dart';
import 'package:netmonitor_app/features/servers/providers/deploys_provider.dart';
import 'package:netmonitor_app/features/servers/utils/server_utils.dart';
import 'package:netmonitor_app/features/servers/widgets/deploy_run_dialog.dart';
import 'package:netmonitor_app/features/servers/widgets/docker_history_widgets.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';
import 'package:url_launcher/url_launcher.dart';

/// Diálogo para iniciar publicação manual em um alvo (`DeployPublishDialog`).
Future<void> showDeployPublishDialog(
  BuildContext context, {
  required DeployTargetView target,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Publicar em ${target.name}',
    subtitle:
        'Projeto ${target.project} · branch ${refShortName(target.reference)}',
    builder: (_) => _DeployPublishBody(target: target),
  );
}

class _DeployPublishBody extends ConsumerStatefulWidget {
  const _DeployPublishBody({required this.target});
  final DeployTargetView target;

  @override
  ConsumerState<_DeployPublishBody> createState() => _DeployPublishBodyState();
}

class _DeployPublishBodyState extends ConsumerState<_DeployPublishBody> {
  final TextEditingController _shaCtrl = TextEditingController();
  String _when = 'now';
  bool _submitting = false;
  String? _error;

  @override
  void dispose() {
    _shaCtrl.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    setState(() {
      _submitting = true;
      _error = null;
    });
    try {
      final run = await ref
          .read(deploysProvider.notifier)
          .triggerDeploy(
            widget.target.id,
            commitSha: _shaCtrl.text.trim(),
            when: _when,
          );
      if (!mounted) return;
      Navigator.of(context).pop();
      await showDeployRunDialog(context, runId: run.id, canAdmin: true);
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = describeError(err, fallback: 'Erro ao disparar publicação.');
        _submitting = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final hasWindow =
        widget.target.windowStart != null &&
        widget.target.windowStart!.isNotEmpty;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        TextField(
          controller: _shaCtrl,
          decoration: InputDecoration(
            labelText:
                'Commit SHA (opcional — vazio usa o mais recente / reaplica .env)',
            hintText: widget.target.live?.shortSha ?? '40 caracteres hex',
          ),
        ),
        const SizedBox(height: 12),
        if (hasWindow)
          SegmentedButton<String>(
            showSelectedIcon: false,
            style: SegmentedButton.styleFrom(
              visualDensity: VisualDensity.compact,
              padding: const EdgeInsets.symmetric(horizontal: 8),
            ),
            segments: const <ButtonSegment<String>>[
              ButtonSegment<String>(
                value: 'now',
                label: Text('Publicar agora', maxLines: 1, softWrap: false),
              ),
              ButtonSegment<String>(
                value: 'window',
                label: Text('Na janela', maxLines: 1, softWrap: false),
              ),
            ],
            selected: <String>{_when},
            onSelectionChanged: (set) => setState(() => _when = set.first),
          ),
        if (_error != null) ...<Widget>[
          const SizedBox(height: 10),
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
        ],
        const SizedBox(height: 16),
        Align(
          alignment: Alignment.centerRight,
          child: FilledButton.icon(
            onPressed: _submitting ? null : _submit,
            icon: const Icon(Icons.rocket_launch),
            label: const Text('Publicar'),
          ),
        ),
      ],
    );
  }
}

/// Diálogo de workflow GitHub Actions + abertura de PR em branch dedicada.
Future<void> showDeployWorkflowDialog(
  BuildContext context, {
  required DeployApplicationView application,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Workflow GitHub Actions · ${application.name}',
    subtitle: 'Notificação assinada por OIDC (sem segredo compartilhado)',
    maxWidth: 780,
    builder: (_) => _DeployWorkflowBody(application: application),
  );
}

class _DeployWorkflowBody extends ConsumerStatefulWidget {
  const _DeployWorkflowBody({required this.application});
  final DeployApplicationView application;

  @override
  ConsumerState<_DeployWorkflowBody> createState() =>
      _DeployWorkflowBodyState();
}

class _DeployWorkflowBodyState extends ConsumerState<_DeployWorkflowBody> {
  late final TextEditingController _urlCtrl;
  DeployWorkflowView? _workflow;
  DeployGithubStatusView? _status;
  bool _loading = true;
  bool _openingPr = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    final defaultUrl = resolveCentralOrigin(
      ref.read(serverConfigProvider).baseUrl,
    );
    _urlCtrl = TextEditingController(text: defaultUrl);
    Future<void>.microtask(_load);
  }

  @override
  void dispose() {
    _urlCtrl.dispose();
    super.dispose();
  }

  Future<void> _load() async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final url = _urlCtrl.text.trim();
      final notifier = ref.read(deploysProvider.notifier);
      final wf = await notifier.workflow(widget.application.id, url);
      DeployGithubStatusView? ghStatus;
      try {
        ghStatus = await notifier.githubStatus(widget.application.id, url);
      } catch (_) {}
      if (!mounted) return;
      setState(() {
        _workflow = wf;
        _status = ghStatus;
        _loading = false;
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = describeError(err, fallback: 'Erro ao gerar workflow.');
        _loading = false;
      });
    }
  }

  Future<void> _openPr() async {
    setState(() {
      _openingPr = true;
      _error = null;
    });
    try {
      final res = await ref
          .read(deploysProvider.notifier)
          .openWorkflowPullRequest(widget.application.id, _urlCtrl.text.trim());
      if (!mounted) return;
      final url = res['url'] as String?;
      if (url != null) {
        await launchUrl(Uri.parse(url), mode: LaunchMode.externalApplication);
      }
      await _load();
    } catch (err) {
      if (!mounted) return;
      setState(
        () => _error = describeError(err, fallback: 'Erro ao abrir PR.'),
      );
    } finally {
      if (mounted) setState(() => _openingPr = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final isPrivate = looksPrivateUrl(_urlCtrl.text);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        InlineFieldActionRow(
          field: TextField(
            controller: _urlCtrl,
            decoration: const InputDecoration(
              labelText: 'Endereço público HTTPS da central',
            ),
            onChanged: (_) => setState(() {}),
          ),
          action: OutlinedButton(onPressed: _load, child: const Text('Gerar')),
        ),
        if (isPrivate) ...<Widget>[
          const SizedBox(height: 8),
          const NestedPanel(
            tone: SemanticTone.warning,
            child: Text(
              'Este endereço parece ser interno/privado — o GitHub Actions precisa de um endereço público HTTPS.',
            ),
          ),
        ],
        if (_status != null) ...<Widget>[
          const SizedBox(height: 10),
          NestedPanel(
            tone: _status!.workflow == 'current'
                ? SemanticTone.success
                : SemanticTone.info,
            child: Wrap(
              spacing: 8,
              runSpacing: 8,
              alignment: WrapAlignment.spaceBetween,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: <Widget>[
                Text('Status no repositório: ${_status!.workflow}'),
                if (_status!.canWrite && _status!.workflow != 'current')
                  FilledButton.tonalIcon(
                    onPressed: _openingPr ? null : _openPr,
                    icon: const Icon(Icons.merge_type, size: 16),
                    label: const Text('Abrir PR com workflow'),
                  ),
              ],
            ),
          ),
        ],
        if (_error != null) ...<Widget>[
          const SizedBox(height: 10),
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
        ],
        const SizedBox(height: 12),
        if (_loading)
          const LoadingStateView(message: 'Gerando YAML do workflow...')
        else if (_workflow != null)
          CodeViewer(
            title: _workflow!.path,
            code: _workflow!.yaml,
            maxHeight: 300,
          ),
      ],
    );
  }
}

/// Histórico de deploys de um alvo específico (`DeployHistoryDialog`).
Future<void> showDeployHistoryDialog(
  BuildContext context, {
  required DeployTargetView target,
  required bool canAdmin,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Histórico · ${target.name}',
    subtitle: 'Projeto ${target.project}',
    maxWidth: 700,
    builder: (_) => _DeployHistoryBody(targetId: target.id, canAdmin: canAdmin),
  );
}

class _DeployHistoryBody extends ConsumerStatefulWidget {
  const _DeployHistoryBody({required this.targetId, required this.canAdmin});
  final int targetId;
  final bool canAdmin;

  @override
  ConsumerState<_DeployHistoryBody> createState() => _DeployHistoryBodyState();
}

class _DeployHistoryBodyState extends ConsumerState<_DeployHistoryBody> {
  List<DeployRunView> _runs = const <DeployRunView>[];
  bool _loading = true;
  String? _error;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(_load);
  }

  Future<void> _load() async {
    try {
      final list = await ref
          .read(deploysProvider.notifier)
          .targetHistory(widget.targetId);
      if (!mounted) return;
      setState(() {
        _runs = list;
        _loading = false;
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = describeError(err, fallback: 'Erro ao carregar histórico.');
        _loading = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_loading) {
      return const LoadingStateView(message: 'Carregando histórico...');
    }
    if (_error != null) return ErrorStateView(message: _error!, onRetry: _load);
    if (_runs.isEmpty) {
      return const EmptyStateView(title: 'Nenhum deploy neste alvo');
    }

    return Column(
      children: _runs
          .map((run) {
            final visual = deployRunStatusMeta(run.status);
            return Padding(
              padding: const EdgeInsets.only(bottom: 8),
              child: ChoiceCard(
                title:
                    '#${run.id} · ${run.shortSha} ${commitFirstLine(run.commitMessage)}',
                subtitle:
                    '${formatDateTime(run.createdAt)}'
                    '${run.durationMs != null ? ' · ${formatElapsedMs(run.durationMs)}' : ''}',
                badge: visual.label,
                badgeTone: visual.tone,
                tone: visual.tone,
                icon: visual.icon,
                onTap: () => showDeployRunDialog(
                  context,
                  runId: run.id,
                  canAdmin: widget.canAdmin,
                ),
              ),
            );
          })
          .toList(growable: false),
    );
  }
}
