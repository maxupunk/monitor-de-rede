import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_markdown_plus/flutter_markdown_plus.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/features/plugins/data/plugins_repository.dart';
import 'package:netmonitor_app/features/plugins/models/plugin_models.dart';
import 'package:netmonitor_app/features/plugins/providers/plugins_provider.dart';
import 'package:netmonitor_app/features/plugins/widgets/plugin_output_view.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Editor completo de plugins em 6 abas (`PluginEditorDialog.vue`):
/// Manifesto, Script Rhai, Uso (Markdown + prévia), Testes JSON, Prévia sandbox e Compatibilidade.
Future<PluginDetail?> showPluginEditorDialog({
  required BuildContext context,
  PluginDetail? existing,
  int? exclusiveDeviceId,
}) {
  return showAdaptiveFormSheet<PluginDetail>(
    context: context,
    title: existing == null
        ? 'Novo plugin'
        : '${existing.summary.source == 'builtin' ? 'Ver plugin' : 'Editar plugin'} · ${existing.summary.name}',
    subtitle: existing != null
        ? '${existing.summary.slug} · v${existing.summary.version}'
        : 'Defina o manifesto JSON, script Rhai, documentação de uso e testes.',
    maxWidth: 860,
    builder: (_) => _PluginEditorBody(
      existing: existing,
      exclusiveDeviceId: exclusiveDeviceId,
    ),
  );
}

class _PluginEditorBody extends ConsumerStatefulWidget {
  const _PluginEditorBody({this.existing, this.exclusiveDeviceId});

  final PluginDetail? existing;
  final int? exclusiveDeviceId;

  @override
  ConsumerState<_PluginEditorBody> createState() => _PluginEditorBodyState();
}

class _PluginEditorBodyState extends ConsumerState<_PluginEditorBody>
    with SingleTickerProviderStateMixin {
  late TabController _tabCtrl;
  late TextEditingController _manifestCtrl;
  late TextEditingController _scriptCtrl;
  late TextEditingController _usageCtrl;
  late TextEditingController _testsCtrl;

  bool _exclusive = false;
  bool _saving = false;
  bool _previewing = false;
  PluginPreviewResult? _preview;
  String? _error;

  bool get _isBuiltin => widget.existing?.summary.source == 'builtin';

  @override
  void initState() {
    super.initState();
    _tabCtrl = TabController(length: 6, vsync: this);
    _tabCtrl.addListener(_onTabChanged);

    final encoder = const JsonEncoder.withIndent('  ');
    final pkg = widget.existing?.package;
    _manifestCtrl = TextEditingController(
      text: encoder.convert(
        pkg?.manifest ??
            <String, dynamic>{
              'slug': 'meu-plugin',
              'name': 'Meu Plugin',
              'version': '1.0.0',
              'transports': <String>['ssh'],
              'actions': <Map<String, dynamic>>[
                <String, dynamic>{
                  'name': 'status',
                  'title': 'Consultar status',
                  'effect': 'read',
                },
              ],
            },
      ),
    );
    _scriptCtrl = TextEditingController(
      text:
          pkg?.script ??
          '// Script Rhai (use {{username}} e {{password}} no transporte)\n'
              'fn status(ctx) {\n  #{ ok: true }\n}\n',
    );
    _usageCtrl = TextEditingController(
      text:
          pkg?.usage ??
          '# Uso\n\nDescreva o que este plugin consulta ou altera.',
    );
    _testsCtrl = TextEditingController(
      text: encoder.convert(
        pkg?.tests ??
            <String, dynamic>{'unit': <dynamic>[], 'functional': <dynamic>[]},
      ),
    );
    _exclusive = widget.exclusiveDeviceId != null;
  }

  @override
  void dispose() {
    _tabCtrl.removeListener(_onTabChanged);
    _tabCtrl.dispose();
    _manifestCtrl.dispose();
    _scriptCtrl.dispose();
    _usageCtrl.dispose();
    _testsCtrl.dispose();
    super.dispose();
  }

  void _onTabChanged() {
    if (_tabCtrl.index == 4 && _preview == null && !_previewing) {
      _runPreview();
    }
  }

  PluginPackage? _buildPackage() {
    try {
      final manifest = jsonDecode(_manifestCtrl.text) as Map<String, dynamic>;
      final tests = jsonDecode(_testsCtrl.text) as Map<String, dynamic>;
      return PluginPackage(
        manifest: manifest,
        script: _scriptCtrl.text,
        usage: _usageCtrl.text,
        tests: tests,
        raw: widget.existing?.package.raw ?? const <String, dynamic>{},
      );
    } catch (err) {
      setState(() => _error = 'JSON inválido no manifesto ou testes: $err');
      return null;
    }
  }

  Future<void> _runPreview() async {
    final pkg = _buildPackage();
    if (pkg == null) return;
    setState(() {
      _previewing = true;
      _error = null;
    });
    try {
      final res = await ref.read(pluginsRepositoryProvider).previewPackage(pkg);
      if (mounted) {
        setState(() {
          _preview = res;
          _previewing = false;
        });
      }
    } catch (err) {
      if (mounted) {
        setState(() {
          _previewing = false;
          _error = err.toString();
        });
      }
    }
  }

  Future<void> _save({bool runTestsAfter = false}) async {
    final pkg = _buildPackage();
    if (pkg == null) return;
    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      PluginDetail detail;
      if (widget.existing == null) {
        detail = await ref
            .read(pluginsProvider.notifier)
            .createPlugin(
              pkg,
              deviceId: _exclusive ? widget.exclusiveDeviceId : null,
              refreshDeviceId: widget.exclusiveDeviceId,
            );
      } else {
        detail = await ref
            .read(pluginsProvider.notifier)
            .updatePlugin(
              widget.existing!.summary.id,
              pkg,
              refreshDeviceId: widget.exclusiveDeviceId,
            );
      }
      if (runTestsAfter) {
        await ref
            .read(pluginsProvider.notifier)
            .runTests(
              detail.summary.id,
              refreshDeviceId: widget.exclusiveDeviceId,
            );
        detail = await ref
            .read(pluginsProvider.notifier)
            .fetchDetail(detail.summary.id);
      }
      if (mounted) Navigator.of(context).pop(detail);
    } catch (err) {
      if (mounted) {
        setState(() {
          _saving = false;
          _error = err.toString();
        });
      }
    }
  }

  Future<void> _duplicate() async {
    if (widget.existing == null) return;
    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      final copy = await ref
          .read(pluginsProvider.notifier)
          .lifecycle(
            widget.existing!.summary.id,
            'duplicate',
            refreshDeviceId: widget.exclusiveDeviceId,
          );
      if (mounted) Navigator.of(context).pop(copy);
    } catch (err) {
      if (mounted) {
        setState(() {
          _saving = false;
          _error = err.toString();
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final pluginColor = context.semanticColors.plugin;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        TabBar(
          controller: _tabCtrl,
          isScrollable: true,
          indicatorColor: pluginColor,
          labelColor: pluginColor,
          tabs: const <Tab>[
            Tab(text: 'Manifesto'),
            Tab(text: 'Script'),
            Tab(text: 'Uso'),
            Tab(text: 'Testes'),
            Tab(text: 'Prévia'),
            Tab(text: 'Compatibilidade'),
          ],
        ),
        const SizedBox(height: 12),
        if (_error != null) ...<Widget>[
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
          const SizedBox(height: 10),
        ],
        SizedBox(
          height: 360,
          child: TabBarView(
            controller: _tabCtrl,
            children: <Widget>[
              TextField(
                controller: _manifestCtrl,
                readOnly: _isBuiltin,
                maxLines: null,
                expands: true,
                style: const TextStyle(fontFamily: 'monospace', fontSize: 12.5),
                decoration: const InputDecoration(
                  alignLabelWithHint: true,
                  labelText: 'Manifesto (JSON)',
                ),
              ),
              Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  const NestedPanel(
                    tone: SemanticTone.info,
                    padding: EdgeInsets.all(8),
                    child: Text(
                      'Segredos nunca entram no script: use {{username}} e {{password}}, substituídos no transporte seguro.',
                      style: TextStyle(fontSize: 12),
                    ),
                  ),
                  const SizedBox(height: 8),
                  Expanded(
                    child: TextField(
                      controller: _scriptCtrl,
                      readOnly: _isBuiltin,
                      maxLines: null,
                      expands: true,
                      style: const TextStyle(
                        fontFamily: 'monospace',
                        fontSize: 12.5,
                      ),
                      decoration: const InputDecoration(
                        alignLabelWithHint: true,
                        labelText: 'Script Rhai',
                      ),
                    ),
                  ),
                ],
              ),
              Row(
                children: <Widget>[
                  Expanded(
                    child: TextField(
                      controller: _usageCtrl,
                      readOnly: _isBuiltin,
                      maxLines: null,
                      expands: true,
                      onChanged: (_) => setState(() {}),
                      decoration: const InputDecoration(
                        alignLabelWithHint: true,
                        labelText: 'Uso (Markdown)',
                      ),
                    ),
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: NestedPanel(
                      child: SingleChildScrollView(
                        child: MarkdownBody(data: _usageCtrl.text),
                      ),
                    ),
                  ),
                ],
              ),
              TextField(
                controller: _testsCtrl,
                readOnly: _isBuiltin,
                maxLines: null,
                expands: true,
                style: const TextStyle(fontFamily: 'monospace', fontSize: 12.5),
                decoration: const InputDecoration(
                  alignLabelWithHint: true,
                  labelText: 'Testes (JSON unit + functional)',
                ),
              ),
              _buildPreviewTab(),
              _buildCompatibilityTab(),
            ],
          ),
        ),
        if (widget.existing == null && widget.exclusiveDeviceId != null)
          CheckboxListTile(
            contentPadding: EdgeInsets.zero,
            value: _exclusive,
            onChanged: (v) => setState(() => _exclusive = v == true),
            title: const Text('Exclusivo deste dispositivo'),
          ),
        const SizedBox(height: 12),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          alignment: WrapAlignment.end,
          children: <Widget>[
            TextButton(
              onPressed: _saving ? null : () => Navigator.of(context).pop(),
              child: const Text('Fechar'),
            ),
            if (_isBuiltin)
              FilledButton.icon(
                onPressed: _saving ? null : _duplicate,
                icon: const Icon(Icons.copy, size: 18),
                label: const Text('Duplicar para editar'),
              )
            else ...<Widget>[
              OutlinedButton.icon(
                onPressed: _saving ? null : () => _save(runTestsAfter: false),
                icon: const Icon(Icons.save_outlined, size: 18),
                label: const Text('Salvar'),
              ),
              FilledButton.icon(
                onPressed: _saving ? null : () => _save(runTestsAfter: true),
                icon: _saving
                    ? const SizedBox(
                        width: 16,
                        height: 16,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(Icons.science_outlined, size: 18),
                label: const Text('Salvar e testar'),
              ),
            ],
          ],
        ),
      ],
    );
  }

  Widget _buildPreviewTab() {
    if (_previewing) {
      return const LoadingStateView(message: 'Executando prévia no sandbox...');
    }
    return SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Align(
            alignment: Alignment.centerRight,
            child: FilledButton.tonalIcon(
              onPressed: _runPreview,
              icon: const Icon(Icons.play_arrow, size: 18),
              label: const Text('Atualizar prévia'),
            ),
          ),
          if (_preview != null) ...<Widget>[
            const SizedBox(height: 8),
            Wrap(
              spacing: 8,
              runSpacing: 6,
              children: <Widget>[
                SemanticChip(
                  label:
                      '${_preview!.report.passed}/${_preview!.report.total} testes OK',
                  tone: _preview!.report.failed == 0
                      ? SemanticTone.success
                      : SemanticTone.error,
                  compact: true,
                ),
              ],
            ),
            for (final p in _preview!.report.problems) ...<Widget>[
              const SizedBox(height: 6),
              NestedPanel(tone: SemanticTone.error, child: Text(p)),
            ],
            for (final hint in _preview!.report.hints) ...<Widget>[
              const SizedBox(height: 6),
              NestedPanel(tone: SemanticTone.info, child: Text(hint)),
            ],
            for (final testCase in _preview!.report.cases) ...<Widget>[
              const SizedBox(height: 8),
              NestedPanel(
                tone: testCase.passed
                    ? SemanticTone.success
                    : SemanticTone.error,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Text(
                      '${testCase.passed ? '✓' : '✗'} ${testCase.name}',
                      style: const TextStyle(fontWeight: FontWeight.w700),
                    ),
                    if (testCase.error != null) Text(testCase.error!),
                    if (testCase.output != null) ...<Widget>[
                      const SizedBox(height: 6),
                      PluginOutputView(output: testCase.output),
                    ],
                  ],
                ),
              ),
            ],
          ],
        ],
      ),
    );
  }

  Widget _buildCompatibilityTab() {
    final summary = widget.existing?.summary;
    if (summary == null) {
      return const EmptyStateView(
        title: 'Salve o plugin para acompanhar validações em equipamentos.',
        icon: Icons.verified_outlined,
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        KeyValueRow(
          label: 'Equipamentos validados',
          value: '${summary.validatedCount}',
        ),
        KeyValueRow(
          label: 'Transportes',
          value: summary.transports.join(', ').toUpperCase(),
        ),
        KeyValueRow(
          label: 'Escopo',
          value: summary.scope == 'device'
              ? 'Exclusivo (dispositivo #${summary.deviceId})'
              : 'Modelo compatível',
        ),
      ],
    );
  }
}
