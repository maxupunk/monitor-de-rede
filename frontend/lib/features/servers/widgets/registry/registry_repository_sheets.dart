import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/registry_models.dart';
import 'package:netmonitor_app/features/servers/providers/registry_provider.dart';
import 'package:netmonitor_app/features/servers/utils/server_utils.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';
import 'package:url_launcher/url_launcher.dart';

/// Abre o detalhe de versões e tags imutáveis de um repositório do Registry.
Future<void> showRegistryRepositoryDetailSheet(
  BuildContext context, {
  required RegistryRepositoryView repository,
  required bool canAdmin,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: repository.name,
    subtitle: 'Versões, tags imutáveis e varredura SBOM/OSV',
    maxWidth: 820,
    builder: (_) => RegistryRepositoryDetailBody(
      repositoryId: repository.id,
      canAdmin: canAdmin,
    ),
  );
}

class RegistryRepositoryDetailBody extends ConsumerStatefulWidget {
  const RegistryRepositoryDetailBody({
    super.key,
    required this.repositoryId,
    required this.canAdmin,
  });

  final int repositoryId;
  final bool canAdmin;

  @override
  ConsumerState<RegistryRepositoryDetailBody> createState() =>
      _RegistryRepositoryDetailBodyState();
}

class _RegistryRepositoryDetailBodyState
    extends ConsumerState<RegistryRepositoryDetailBody> {
  RegistryRepositoryDetail? _detail;
  bool _loading = true;
  String? _error;
  int _lastRevision = -1;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(_load);
  }

  Future<void> _load() async {
    try {
      final d = await ref
          .read(registryProvider.notifier)
          .repositoryDetail(widget.repositoryId);
      if (!mounted) return;
      setState(() {
        _detail = d;
        _loading = false;
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = describeError(
          err,
          fallback: 'Erro ao carregar versões do repositório.',
        );
        _loading = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final revision = ref.watch(registryProvider).revision;
    if (_lastRevision != -1 && _lastRevision != revision && !_loading) {
      _lastRevision = revision;
      WidgetsBinding.instance.addPostFrameCallback((_) => _load());
    } else if (_lastRevision == -1) {
      _lastRevision = revision;
    }

    if (_loading && _detail == null) {
      return const LoadingStateView(message: 'Carregando versões...');
    }
    if (_error != null) return ErrorStateView(message: _error!, onRetry: _load);
    final versions = _detail?.versions ?? const <RegistryVersionView>[];
    if (versions.isEmpty) {
      return const EmptyStateView(title: 'Nenhuma versão neste repositório');
    }

    return Column(
      children: versions
          .map((v) {
            final scanTone = registryScanTone(v.scan);
            return Padding(
              padding: const EdgeInsets.only(bottom: 10),
              child: NestedPanel(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Wrap(
                      spacing: 6,
                      runSpacing: 6,
                      children: <Widget>[
                        ...v.tags.map(
                          (tag) => SemanticChip(
                            label: shortRegistryTag(tag),
                            tone: isCommitTag(tag)
                                ? SemanticTone.info
                                : SemanticTone.primary,
                            icon: isCommitTag(tag)
                                ? Icons.lock_outline
                                : Icons.local_offer_outlined,
                            compact: true,
                          ),
                        ),
                        if (v.pinned)
                          const SemanticChip(
                            label: 'Fixada',
                            tone: SemanticTone.success,
                            icon: Icons.push_pin_outlined,
                            compact: true,
                          ),
                        ...v.usedBy.map(
                          (target) => SemanticChip(
                            label: 'No ar em $target',
                            tone: SemanticTone.success,
                            compact: true,
                          ),
                        ),
                        SemanticChip(
                          label: registryScanSentence(v.scan),
                          tone: scanTone,
                          compact: true,
                        ),
                      ],
                    ),
                    const SizedBox(height: 6),
                    Text(
                      '${shortRegistryDigest(v.digest)} · ${formatDecimalBytes(v.bytes)} · '
                      '${registryPlatformsLabel(v.platforms)} · ${formatDateTime(v.createdAt)}',
                      style: context.textTheme.bodySmall?.copyWith(
                        fontFamily: 'monospace',
                      ),
                    ),
                    const SizedBox(height: 8),
                    Wrap(
                      spacing: 8,
                      runSpacing: 8,
                      children: <Widget>[
                        OutlinedButton.icon(
                          onPressed: () => showAdaptiveFormSheet<void>(
                            context: context,
                            title:
                                'Vulnerabilidades · ${shortRegistryDigest(v.digest)}',
                            maxWidth: 780,
                            builder: (_) => RegistryFindingsDialogBody(
                              version: v,
                              applicationId: _detail?.repository.applicationId,
                              canAdmin: widget.canAdmin,
                            ),
                          ),
                          icon: const Icon(Icons.shield_outlined, size: 16),
                          label: const Text('Vulnerabilidades'),
                        ),
                        OutlinedButton.icon(
                          onPressed: () async {
                            await ref
                                .read(registryProvider.notifier)
                                .pinVersion(v.id, pinned: !v.pinned);
                            await _load();
                          },
                          icon: Icon(
                            v.pinned ? Icons.push_pin : Icons.push_pin_outlined,
                            size: 16,
                          ),
                          label: Text(v.pinned ? 'Soltar' : 'Fixar'),
                        ),
                        if (widget.canAdmin)
                          Tooltip(
                            message: v.canDelete
                                ? 'Apagar versão'
                                : 'Versões em uso, protegidas ou fixadas não podem ser apagadas.',
                            child: TextButton.icon(
                              onPressed: v.canDelete
                                  ? () async {
                                      final ok = await showConfirmActionDialog(
                                        context: context,
                                        title:
                                            'Apagar versão ${shortRegistryDigest(v.digest)}?',
                                        message:
                                            'Esta versão será removida do catálogo.',
                                        confirmLabel: 'Apagar',
                                        tone: SemanticTone.error,
                                      );
                                      if (!ok) return;
                                      await ref
                                          .read(registryProvider.notifier)
                                          .removeVersion(v.id);
                                      await _load();
                                    }
                                  : null,
                              icon: const Icon(Icons.delete_outline, size: 16),
                              label: const Text('Apagar'),
                            ),
                          ),
                      ],
                    ),
                  ],
                ),
              ),
            );
          })
          .toList(growable: false),
    );
  }
}

class RegistryFindingsDialogBody extends ConsumerStatefulWidget {
  const RegistryFindingsDialogBody({
    super.key,
    required this.version,
    required this.applicationId,
    required this.canAdmin,
  });

  final RegistryVersionView version;
  final int? applicationId;
  final bool canAdmin;

  @override
  ConsumerState<RegistryFindingsDialogBody> createState() =>
      _RegistryFindingsDialogBodyState();
}

class _RegistryFindingsDialogBodyState
    extends ConsumerState<RegistryFindingsDialogBody> {
  List<RegistryFindingView> _findings = const <RegistryFindingView>[];
  bool _loading = true;
  bool _scanning = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(_load);
  }

  Future<void> _load() async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final list = await ref
          .read(registryProvider.notifier)
          .versionFindings(widget.version.id);
      if (!mounted) return;
      setState(() {
        _findings = list;
        _loading = false;
      });
    } catch (err) {
      if (mounted) {
        setState(() {
          _loading = false;
          _error = describeError(
            err,
            fallback: 'Erro ao carregar vulnerabilidades.',
          );
        });
      }
    }
  }

  Future<void> _rescan() async {
    setState(() {
      _scanning = true;
      _error = null;
    });
    try {
      final list = await ref
          .read(registryProvider.notifier)
          .rescanVersion(widget.version.id);
      if (!mounted) return;
      setState(() {
        _findings = list;
        _scanning = false;
      });
    } catch (err) {
      if (mounted) {
        setState(() {
          _scanning = false;
          _error = describeError(err, fallback: 'Erro ao reanalisar imagem.');
        });
      }
    }
  }

  Future<void> _acceptRisk(RegistryFindingView f) async {
    final reason = await showDialog<String>(
      context: context,
      builder: (_) => _AcceptRiskDialog(advisory: f.advisory),
    );
    if (reason == null || !mounted) return;
    try {
      await ref
          .read(registryProvider.notifier)
          .addException(
            applicationId: widget.applicationId,
            advisory: f.advisory,
            reason: reason,
            days: 30,
          );
      await _load();
    } catch (err) {
      if (mounted) {
        setState(
          () => _error = describeError(
            err,
            fallback: 'Erro ao registrar exceção.',
          ),
        );
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Wrap(
          spacing: 8,
          runSpacing: 8,
          alignment: WrapAlignment.spaceBetween,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: <Widget>[
            SemanticChip(
              label: registryScanSentence(widget.version.scan),
              tone: registryScanTone(widget.version.scan),
            ),
            OutlinedButton.icon(
              onPressed: _scanning ? null : _rescan,
              icon: const Icon(Icons.refresh, size: 16),
              label: const Text('Analisar de novo'),
            ),
          ],
        ),
        const SizedBox(height: 12),
        if (_error != null) ...<Widget>[
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
          const SizedBox(height: 10),
        ],
        if (_loading)
          const LoadingStateView(message: 'Lendo relatório SBOM/OSV...')
        else if (_findings.isEmpty)
          EmptyStateView(
            title: registryScanSentence(widget.version.scan),
            icon: Icons.verified_user_outlined,
            tone: registryScanTone(widget.version.scan),
          )
        else
          ..._findings.map(
            (f) => Padding(
              padding: const EdgeInsets.only(bottom: 8),
              child: NestedPanel(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Wrap(
                      spacing: 6,
                      runSpacing: 4,
                      crossAxisAlignment: WrapCrossAlignment.center,
                      children: <Widget>[
                        SemanticChip(
                          label: registrySeverityLabel(f.severity),
                          tone: registrySeverityTone(f.severity),
                          compact: true,
                        ),
                        if (f.excepted)
                          const SemanticChip(
                            label: 'Exceção',
                            tone: SemanticTone.info,
                            compact: true,
                          ),
                        TextButton(
                          onPressed: () => launchUrl(
                            Uri.parse(
                              'https://osv.dev/vulnerability/${Uri.encodeComponent(f.advisory)}',
                            ),
                            mode: LaunchMode.externalApplication,
                          ),
                          child: Text(
                            f.advisory,
                            style: const TextStyle(fontFamily: 'monospace'),
                          ),
                        ),
                      ],
                    ),
                    if (f.summary != null && f.summary!.isNotEmpty)
                      Text(f.summary!, style: context.textTheme.bodySmall),
                    const SizedBox(height: 4),
                    Text(
                      'Pacote: ${f.package} (${f.ecosystem}) · Instalado: ${f.installed} · '
                      'Correção: ${f.fixed ?? 'sem correção ainda'}',
                      style: context.textTheme.bodySmall?.copyWith(
                        color: context.colorScheme.onSurfaceVariant,
                      ),
                    ),
                    if (widget.canAdmin && !f.excepted) ...<Widget>[
                      const SizedBox(height: 6),
                      TextButton.icon(
                        onPressed: () => _acceptRisk(f),
                        icon: const Icon(Icons.rule, size: 16),
                        label: const Text('Aceitar risco'),
                      ),
                    ],
                  ],
                ),
              ),
            ),
          ),
      ],
    );
  }
}

class _AcceptRiskDialog extends StatefulWidget {
  const _AcceptRiskDialog({required this.advisory});
  final String advisory;

  @override
  State<_AcceptRiskDialog> createState() => _AcceptRiskDialogState();
}

class _AcceptRiskDialogState extends State<_AcceptRiskDialog> {
  final TextEditingController _reasonCtrl = TextEditingController();
  String? _dialogError;

  @override
  void dispose() {
    _reasonCtrl.dispose();
    super.dispose();
  }

  void _confirm() {
    final trimmed = _reasonCtrl.text.trim();
    if (trimmed.length < 5) {
      setState(
        () => _dialogError = 'Informe ao menos 5 caracteres de justificativa.',
      );
      return;
    }
    Navigator.of(context).pop(trimmed);
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: Text('Aceitar risco · ${widget.advisory}'),
      content: TextField(
        controller: _reasonCtrl,
        autofocus: true,
        maxLines: 3,
        onChanged: (_) {
          if (_dialogError != null) {
            setState(() => _dialogError = null);
          }
        },
        decoration: InputDecoration(
          labelText: 'Justificativa (mín. 5 caracteres)',
          errorText: _dialogError,
        ),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Cancelar'),
        ),
        FilledButton(
          onPressed: _confirm,
          child: const Text('Aceitar por 30 dias'),
        ),
      ],
    );
  }
}
