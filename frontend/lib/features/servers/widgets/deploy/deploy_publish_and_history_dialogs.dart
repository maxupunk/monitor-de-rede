import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/network_address_utils.dart';
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
  DeployBranchCommitsView? _commits;
  bool _loadingCommits = true;

  @override
  void initState() {
    super.initState();
    _shaCtrl.addListener(_onShaChanged);
    Future<void>.microtask(_loadCommits);
  }

  @override
  void dispose() {
    _shaCtrl
      ..removeListener(_onShaChanged)
      ..dispose();
    super.dispose();
  }

  void _onShaChanged() {
    if (mounted) setState(() {});
  }

  /// Uma consulta ao abrir: os commits recentes da branch do alvo.
  Future<void> _loadCommits() async {
    DeployBranchCommitsView commits;
    try {
      commits = await ref
          .read(deploysProvider.notifier)
          .targetCommits(widget.target.id);
    } catch (err) {
      commits = DeployBranchCommitsView(
        branch: refShortName(widget.target.reference),
        commits: const <DeployCommitView>[],
        unavailableReason: describeError(
          err,
          fallback: 'Não foi possível listar os commits da branch.',
        ),
      );
    }
    if (!mounted) return;
    setState(() {
      _commits = commits;
      _loadingCommits = false;
    });
  }

  /// Um toque escolhe o commit (preenche o SHA); outro no mesmo desfaz.
  void _toggleCommit(DeployCommitView commit) {
    final selected = _shaCtrl.text.trim().toLowerCase() == commit.sha;
    _shaCtrl.text = selected ? '' : commit.sha;
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
    final target = widget.target;
    final hasWindow =
        target.windowStart != null && target.windowStart!.isNotEmpty;
    final commits = _commits;
    final live = target.live;
    final plan = publishEmptyCommitPlan(
      reference: target.reference,
      liveShortSha: live?.shortSha,
      latestShortSha: commits?.latest?.shortSha,
      latestUnavailable: commits?.unavailableReason != null,
    );
    final typed = _shaCtrl.text.trim();
    final canSubmit = !_submitting && (typed.isNotEmpty || plan.allowed);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        TextField(
          controller: _shaCtrl,
          decoration: InputDecoration(
            labelText: plan.allowed ? 'Commit SHA (opcional)' : 'Commit SHA',
            hintText: '40 caracteres hex',
            helperText: typed.isEmpty ? plan.hint : null,
            helperMaxLines: 3,
            suffixIcon: typed.isEmpty
                ? null
                : IconButton(
                    tooltip: 'Limpar',
                    icon: const Icon(Icons.clear),
                    onPressed: _shaCtrl.clear,
                  ),
          ),
        ),
        const SizedBox(height: 12),
        _PublishCommitList(
          loading: _loadingCommits,
          commits: commits,
          liveSha: live?.commitSha,
          selectedSha: typed.toLowerCase(),
          onSelect: _toggleCommit,
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
            onPressed: canSubmit ? _submit : null,
            icon: const Icon(Icons.rocket_launch),
            label: const Text('Publicar'),
          ),
        ),
      ],
    );
  }
}

/// Os commits recentes da branch do alvo; sem a lista, o motivo — com o que
/// fazer.
class _PublishCommitList extends StatelessWidget {
  const _PublishCommitList({
    required this.loading,
    required this.commits,
    required this.liveSha,
    required this.selectedSha,
    required this.onSelect,
  });

  final bool loading;
  final DeployBranchCommitsView? commits;
  final String? liveSha;
  final String selectedSha;
  final ValueChanged<DeployCommitView> onSelect;

  @override
  Widget build(BuildContext context) {
    if (loading) {
      return const Row(
        children: <Widget>[
          SizedBox.square(
            dimension: 16,
            child: CircularProgressIndicator(strokeWidth: 2),
          ),
          SizedBox(width: 10),
          Expanded(child: Text('Buscando os commits recentes no GitHub…')),
        ],
      );
    }
    final view = commits;
    if (view == null) return const SizedBox.shrink();
    final reason = view.unavailableReason;
    if (reason != null) {
      return NestedPanel(tone: SemanticTone.warning, child: Text(reason));
    }
    if (view.commits.isEmpty) return const SizedBox.shrink();
    return NestedPanel(
      padding: const EdgeInsets.symmetric(vertical: 6),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Padding(
            padding: const EdgeInsets.fromLTRB(14, 6, 14, 4),
            child: Text(
              'Commits recentes de ${view.branch ?? ''} — toque para escolher',
              style: context.textTheme.titleSmall?.copyWith(
                fontWeight: FontWeight.w700,
              ),
            ),
          ),
          for (final (index, commit) in view.commits.indexed)
            _PublishCommitTile(
              commit: commit,
              latest: index == 0,
              live: commit.sha == liveSha,
              selected: commit.sha == selectedSha,
              onTap: () => onSelect(commit),
            ),
        ],
      ),
    );
  }
}

class _PublishCommitTile extends StatelessWidget {
  const _PublishCommitTile({
    required this.commit,
    required this.latest,
    required this.live,
    required this.selected,
    required this.onTap,
  });

  final DeployCommitView commit;
  final bool latest;
  final bool live;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final details = <String>[
      if (commit.author != null) commit.author!,
      if (commit.date != null) formatRelativeTime(commit.date),
    ].join(' · ');
    // Material próprio: o fundo do NestedPanel esconderia a seleção e o toque.
    return Material(
      type: MaterialType.transparency,
      child: ListTile(
        dense: true,
        selected: selected,
        onTap: onTap,
        leading: Icon(
          selected ? Icons.check_circle : Icons.radio_button_unchecked,
          color: selected ? scheme.primary : scheme.onSurface,
        ),
        title: Text(
          '${commit.shortSha} · ${commitFirstLine(commit.message)}',
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: const TextStyle(fontFamily: 'monospace'),
        ),
        subtitle: details.isEmpty ? null : Text(details),
        trailing: Wrap(
          spacing: 4,
          children: <Widget>[
            if (live)
              const SemanticChip(
                label: 'No ar',
                tone: SemanticTone.success,
                compact: true,
              ),
            if (latest)
              const SemanticChip(
                label: 'Mais recente',
                tone: SemanticTone.info,
                compact: true,
              ),
          ],
        ),
      ),
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
