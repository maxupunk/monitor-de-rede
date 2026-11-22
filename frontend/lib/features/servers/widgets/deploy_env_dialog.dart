import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/deploy_models.dart';
import 'package:netmonitor_app/features/servers/providers/deploys_provider.dart';
import 'package:netmonitor_app/features/servers/utils/server_utils.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Editor criptografado de `.env` por alvo de deploy (ADR 021).
/// Suporta modo Lista (mascaramento automático de segredos) e modo Texto bruto,
/// revisão de diff apenas por nomes de chave e histórico versionado.
Future<void> showDeployEnvDialog(
  BuildContext context, {
  required DeployTargetView target,
  required bool canAdmin,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Variáveis (.env) · ${target.name}',
    subtitle: 'Cifradas na central e gravadas em modo 0600 no servidor',
    maxWidth: 820,
    builder: (_) => _DeployEnvBody(target: target, canAdmin: canAdmin),
  );
}

class _DeployEnvBody extends ConsumerStatefulWidget {
  const _DeployEnvBody({required this.target, required this.canAdmin});
  final DeployTargetView target;
  final bool canAdmin;

  @override
  ConsumerState<_DeployEnvBody> createState() => _DeployEnvBodyState();
}

class _DeployEnvBodyState extends ConsumerState<_DeployEnvBody>
    with SingleTickerProviderStateMixin {
  late final TabController _tabController;
  final TextEditingController _rawController = TextEditingController();
  final TextEditingController _noteController = TextEditingController();
  final TextEditingController _searchController = TextEditingController();

  bool _rawMode = false;
  bool _loading = true;
  bool _saving = false;
  String? _error;
  List<EnvLine> _lines = const <EnvLine>[];
  List<DeployEnvVersionView> _versions = const <DeployEnvVersionView>[];
  final Set<int> _revealedUids = <int>{};
  DeployEnvDiff? _previewDiff;

  @override
  void initState() {
    super.initState();
    _tabController = TabController(length: 2, vsync: this);
    Future<void>.microtask(_load);
  }

  @override
  void dispose() {
    _tabController.dispose();
    _rawController.dispose();
    _noteController.dispose();
    _searchController.dispose();
    super.dispose();
  }

  Future<void> _load() async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final notifier = ref.read(deploysProvider.notifier);
      final results = await Future.wait<dynamic>(<Future<dynamic>>[
        notifier.envCurrent(widget.target.id),
        notifier.envVersions(widget.target.id),
      ]);
      if (!mounted) return;
      final current = results[0] as DeployEnvContentView;
      _setContent(current.content);
      setState(() {
        _versions = results[1] as List<DeployEnvVersionView>;
        _loading = false;
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = describeError(
          err,
          fallback: 'Erro ao carregar variáveis .env.',
        );
        _loading = false;
      });
    }
  }

  void _setContent(String content) {
    _rawController.text = content;
    _lines = parseEnvFile(content);
    _revealedUids.clear();
    _previewDiff = null;
  }

  String _currentContent() =>
      _rawMode ? _rawController.text : serializeEnvFile(_lines);

  void _switchMode(bool toRaw) {
    if (toRaw == _rawMode) return;
    if (toRaw) {
      _rawController.text = serializeEnvFile(_lines);
    } else {
      _lines = parseEnvFile(_rawController.text);
      _revealedUids.clear();
    }
    setState(() {
      _rawMode = toRaw;
      _previewDiff = null;
    });
  }

  void _addVariable() {
    final next = List<EnvLine>.from(_lines)
      ..add(EnvLine.variable(key: 'NOVA_VARIAVEL', value: ''));
    setState(() {
      _lines = next;
      _previewDiff = null;
    });
  }

  void _updateVariable(int index, {String? key, String? value}) {
    final next = List<EnvLine>.from(_lines);
    final old = next[index];
    next[index] = old.copyWithVariable(key: key, value: value);
    setState(() {
      _lines = next;
      _previewDiff = null;
    });
  }

  void _removeLine(int index) {
    final removedUid = _lines[index].uid;
    final next = List<EnvLine>.from(_lines)..removeAt(index);
    setState(() {
      _revealedUids.remove(removedUid);
      _lines = next;
      _previewDiff = null;
    });
  }

  Future<void> _preview() async {
    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      final diff = await ref
          .read(deploysProvider.notifier)
          .envPreview(widget.target.id, _currentContent());
      if (!mounted) return;
      setState(() {
        _previewDiff = diff;
        _saving = false;
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = describeError(err, fallback: 'Erro ao revisar mudanças.');
        _saving = false;
      });
    }
  }

  Future<void> _save({required bool redeploy}) async {
    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      await ref
          .read(deploysProvider.notifier)
          .envSave(
            widget.target.id,
            content: _currentContent(),
            redeploy: redeploy,
            note: _noteController.text.trim(),
          );
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            redeploy
                ? 'Variáveis salvas e redeploy disparado.'
                : 'Variáveis salvas para o próximo deploy.',
          ),
        ),
      );
      Navigator.of(context).pop();
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = describeError(err, fallback: 'Erro ao salvar variáveis .env.');
        _saving = false;
      });
    }
  }

  Future<void> _loadVersionIntoEditor(DeployEnvVersionView version) async {
    try {
      final content = await ref
          .read(deploysProvider.notifier)
          .envVersion(version.id);
      if (!mounted) return;
      setState(() {
        _setContent(content.content);
        _tabController.animateTo(0);
      });
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Versão #${version.id} carregada no editor.')),
      );
    } catch (err) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Erro ao abrir versão: ${describeError(err)}')),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_loading) {
      return const LoadingStateView(message: 'Carregando variáveis .env...');
    }

    final term = _searchController.text.trim().toLowerCase();
    final viewportSize = MediaQuery.sizeOf(context);
    final isCompact = viewportSize.width < 480;
    final tabViewHeight = math.min(440.0, viewportSize.height * 0.56);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        TabBar(
          controller: _tabController,
          isScrollable: isCompact,
          tabAlignment: isCompact ? TabAlignment.start : TabAlignment.fill,
          tabs: <Tab>[
            const Tab(text: 'Editar'),
            Tab(text: 'Histórico (${_versions.length})'),
          ],
        ),
        const SizedBox(height: 12),
        SizedBox(
          height: tabViewHeight,
          child: TabBarView(
            controller: _tabController,
            children: <Widget>[
              ListView(
                children: <Widget>[
                  Wrap(
                    spacing: 8,
                    runSpacing: 8,
                    alignment: WrapAlignment.spaceBetween,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: <Widget>[
                      SegmentedButton<bool>(
                        showSelectedIcon: false,
                        style: SegmentedButton.styleFrom(
                          visualDensity: VisualDensity.compact,
                          padding: const EdgeInsets.symmetric(horizontal: 8),
                        ),
                        segments: const <ButtonSegment<bool>>[
                          ButtonSegment<bool>(
                            value: false,
                            label: Text(
                              'Modo lista',
                              maxLines: 1,
                              softWrap: false,
                            ),
                          ),
                          ButtonSegment<bool>(
                            value: true,
                            label: Text(
                              'Texto .env',
                              maxLines: 1,
                              softWrap: false,
                            ),
                          ),
                        ],
                        selected: <bool>{_rawMode},
                        onSelectionChanged: (set) => _switchMode(set.first),
                      ),
                      if (!_rawMode && widget.canAdmin)
                        OutlinedButton.icon(
                          onPressed: _addVariable,
                          icon: const Icon(Icons.add, size: 16),
                          label: const Text('Nova variável'),
                        ),
                    ],
                  ),
                  const SizedBox(height: 10),
                  if (_rawMode)
                    TextField(
                      controller: _rawController,
                      readOnly: !widget.canAdmin,
                      maxLines: 12,
                      style: const TextStyle(
                        fontFamily: 'monospace',
                        fontSize: 13,
                      ),
                      decoration: const InputDecoration(
                        labelText: 'Conteúdo do arquivo .env',
                        alignLabelWithHint: true,
                      ),
                      onChanged: (_) {
                        if (_previewDiff != null) {
                          setState(() => _previewDiff = null);
                        }
                      },
                    )
                  else ...<Widget>[
                    TextField(
                      controller: _searchController,
                      decoration: const InputDecoration(
                        hintText: 'Filtrar chave...',
                        prefixIcon: Icon(Icons.search, size: 18),
                        isDense: true,
                      ),
                      onChanged: (_) => setState(() {}),
                    ),
                    const SizedBox(height: 10),
                    for (var i = 0; i < _lines.length; i++)
                      if (_lines[i].isVariable &&
                          (term.isEmpty ||
                              _lines[i].key.toLowerCase().contains(term)))
                        Padding(
                          key: ValueKey<int>(_lines[i].uid),
                          padding: const EdgeInsets.only(bottom: 8),
                          child: _buildVarRow(i, _lines[i]),
                        ),
                    if (_lines.where((l) => l.isVariable).isEmpty)
                      const EmptyStateView(
                        title: 'Nenhuma variável definida ainda',
                      ),
                  ],
                  const SizedBox(height: 12),
                  if (widget.canAdmin) ...<Widget>[
                    TextField(
                      controller: _noteController,
                      decoration: const InputDecoration(
                        labelText: 'Nota da revisão (opcional)',
                        hintText: 'ex.: Atualiza URL do Redis',
                      ),
                    ),
                    const SizedBox(height: 12),
                    if (_previewDiff != null) ...<Widget>[
                      NestedPanel(
                        tone: SemanticTone.info,
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: <Widget>[
                            Text(
                              'Revisão das alterações (somente nomes das chaves):',
                              style: context.textTheme.titleSmall?.copyWith(
                                fontWeight: FontWeight.w700,
                              ),
                            ),
                            const SizedBox(height: 8),
                            if (_previewDiff!.isEmpty)
                              const Text('Nenhuma chave alterada.')
                            else
                              Wrap(
                                spacing: 6,
                                runSpacing: 6,
                                children: <Widget>[
                                  ..._previewDiff!.added.map(
                                    (k) => SemanticChip(
                                      label: '+ $k',
                                      tone: SemanticTone.success,
                                      compact: true,
                                    ),
                                  ),
                                  ..._previewDiff!.changed.map(
                                    (k) => SemanticChip(
                                      label: '~ $k',
                                      tone: SemanticTone.warning,
                                      compact: true,
                                    ),
                                  ),
                                  ..._previewDiff!.removed.map(
                                    (k) => SemanticChip(
                                      label: '- $k',
                                      tone: SemanticTone.error,
                                      compact: true,
                                    ),
                                  ),
                                ],
                              ),
                          ],
                        ),
                      ),
                      const SizedBox(height: 12),
                    ],
                    if (_error != null) ...<Widget>[
                      NestedPanel(
                        tone: SemanticTone.error,
                        child: Text(_error!),
                      ),
                      const SizedBox(height: 12),
                    ],
                    Wrap(
                      spacing: 8,
                      runSpacing: 8,
                      alignment: WrapAlignment.end,
                      children: <Widget>[
                        if (_previewDiff == null)
                          FilledButton.icon(
                            onPressed: _saving ? null : _preview,
                            icon: const Icon(Icons.difference_outlined),
                            label: const Text('Revisar mudanças'),
                          )
                        else ...<Widget>[
                          OutlinedButton.icon(
                            onPressed: _saving
                                ? null
                                : () => _save(redeploy: false),
                            icon: const Icon(Icons.save_outlined),
                            label: const Text('Salvar para o próximo deploy'),
                          ),
                          FilledButton.icon(
                            onPressed: (_saving || widget.target.live == null)
                                ? null
                                : () => _save(redeploy: true),
                            icon: const Icon(Icons.rocket_launch),
                            label: const Text('Salvar e aplicar agora'),
                          ),
                        ],
                      ],
                    ),
                  ],
                ],
              ),
              _versions.isEmpty
                  ? const EmptyStateView(title: 'Nenhuma versão salva ainda')
                  : ListView.separated(
                      itemCount: _versions.length,
                      separatorBuilder: (_, _) => const SizedBox(height: 8),
                      itemBuilder: (_, idx) {
                        final v = _versions[idx];
                        return NestedPanel(
                          tone: v.current ? SemanticTone.success : null,
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: <Widget>[
                              Row(
                                children: <Widget>[
                                  Expanded(
                                    child: Text(
                                      'Versão #${v.id} · ${formatDateTime(v.createdAt)}',
                                      style: const TextStyle(
                                        fontWeight: FontWeight.w700,
                                      ),
                                    ),
                                  ),
                                  if (v.current)
                                    const SemanticChip(
                                      label: 'Atual',
                                      tone: SemanticTone.success,
                                      compact: true,
                                    ),
                                ],
                              ),
                              if (v.note != null &&
                                  v.note!.isNotEmpty) ...<Widget>[
                                const SizedBox(height: 4),
                                Text(v.note!),
                              ],
                              const SizedBox(height: 4),
                              Text(
                                '${countNoun(v.keys.length, 'chave')}: ${v.keys.take(8).join(', ')}'
                                '${v.keys.length > 8 ? '…' : ''}',
                                style: context.textTheme.bodySmall,
                              ),
                              if (widget.canAdmin) ...<Widget>[
                                const SizedBox(height: 8),
                                OutlinedButton.icon(
                                  onPressed: () => _loadVersionIntoEditor(v),
                                  icon: const Icon(Icons.restore, size: 16),
                                  label: const Text('Abrir no editor'),
                                ),
                              ],
                            ],
                          ),
                        );
                      },
                    ),
            ],
          ),
        ),
      ],
    );
  }

  Widget _buildVarRow(int index, EnvLine line) {
    final secret = looksLikeSecretKey(line.key);
    final revealed = _revealedUids.contains(line.uid);
    final validKey = isValidEnvKey(line.key);

    final keyField = TextFormField(
      key: ValueKey<String>('env-k-${line.uid}'),
      initialValue: line.key,
      readOnly: !widget.canAdmin,
      style: const TextStyle(fontFamily: 'monospace', fontSize: 13),
      decoration: InputDecoration(
        labelText: 'Chave',
        isDense: true,
        errorText: validKey ? null : 'Chave inválida',
      ),
      onChanged: (v) => _updateVariable(index, key: v),
    );

    final valueField = TextFormField(
      key: ValueKey<String>('env-v-${line.uid}'),
      initialValue: line.value,
      readOnly: !widget.canAdmin,
      obscureText: secret && !revealed,
      style: const TextStyle(fontFamily: 'monospace', fontSize: 13),
      decoration: InputDecoration(
        labelText: 'Valor',
        isDense: true,
        suffixIcon: secret
            ? IconButton(
                tooltip: revealed ? 'Ocultar' : 'Mostrar',
                icon: Icon(
                  revealed ? Icons.visibility_off : Icons.visibility,
                  size: 18,
                ),
                onPressed: () => setState(() {
                  if (revealed) {
                    _revealedUids.remove(line.uid);
                  } else {
                    _revealedUids.add(line.uid);
                  }
                }),
              )
            : null,
      ),
      onChanged: (v) => _updateVariable(index, value: v),
    );

    final deleteButton = widget.canAdmin
        ? IconButton(
            tooltip: 'Remover variável',
            icon: const Icon(Icons.delete_outline, size: 20),
            onPressed: () => _removeLine(index),
          )
        : null;

    return NestedPanel(
      padding: const EdgeInsets.all(10),
      child: LayoutBuilder(
        builder: (context, constraints) {
          if (constraints.maxWidth < 480) {
            return Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Expanded(child: keyField),
                    ?deleteButton,
                  ],
                ),
                const SizedBox(height: 8),
                valueField,
              ],
            );
          }

          return Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Expanded(flex: 2, child: keyField),
              const SizedBox(width: 8),
              Expanded(flex: 3, child: valueField),
              ?deleteButton,
            ],
          );
        },
      ),
    );
  }
}
