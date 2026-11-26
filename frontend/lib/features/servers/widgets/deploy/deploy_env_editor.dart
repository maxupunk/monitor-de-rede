import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/deploy_models.dart';
import 'package:netmonitor_app/features/servers/utils/env_file_parser.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Editor do passo 3 (`3. .env`) do assistente de deploy contínuo.
///
/// Suporta carregamento automático e troca de modelos `.env.example` / `.env.*`
/// encontrados no repositório, edição estruturada em **Modo lista** (com
/// contagem de campos em branco e destaque de chaves sensíveis) e **Texto .env**
/// bruto sincronizado de forma lossless via `env_file_parser.dart`.
class DeployEnvStepEditor extends StatefulWidget {
  const DeployEnvStepEditor({
    super.key,
    required this.content,
    required this.templates,
    required this.loadingTemplates,
    required this.hasRepository,
    required this.onContentChanged,
    required this.onSelectTemplate,
    required this.onClear,
    this.activeTemplateName,
    this.onReloadTemplates,
  });

  final String content;
  final List<DeployEnvTemplateView> templates;
  final String? activeTemplateName;
  final bool loadingTemplates;
  final bool hasRepository;
  final ValueChanged<String> onContentChanged;
  final ValueChanged<DeployEnvTemplateView> onSelectTemplate;
  final VoidCallback onClear;
  final VoidCallback? onReloadTemplates;

  @override
  State<DeployEnvStepEditor> createState() => _DeployEnvStepEditorState();
}

class _DeployEnvStepEditorState extends State<DeployEnvStepEditor> {
  late final TextEditingController _rawController;
  final TextEditingController _searchController = TextEditingController();
  late List<EnvLine> _lines;
  final Set<int> _maskedUids = <int>{};
  bool _rawMode = false;
  String _lastEmitted = '';

  @override
  void initState() {
    super.initState();
    _lastEmitted = widget.content;
    _rawController = TextEditingController(text: widget.content);
    _lines = parseEnvFile(widget.content);
  }

  @override
  void didUpdateWidget(covariant DeployEnvStepEditor oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.content != _lastEmitted && widget.content != oldWidget.content) {
      _lastEmitted = widget.content;
      _rawController.text = widget.content;
      _lines = parseEnvFile(widget.content);
      _maskedUids.clear();
    }
  }

  @override
  void dispose() {
    _rawController.dispose();
    _searchController.dispose();
    super.dispose();
  }

  void _emit(String next) {
    _lastEmitted = next;
    widget.onContentChanged(next);
  }

  void _switchMode(bool toRaw) {
    if (toRaw == _rawMode) return;
    if (toRaw) {
      final serialized = serializeEnvFile(_lines);
      _rawController.text = serialized;
      _emit(serialized);
    } else {
      _lines = parseEnvFile(_rawController.text);
      _maskedUids.clear();
    }
    setState(() => _rawMode = toRaw);
  }

  void _addVariable() {
    final next = List<EnvLine>.from(_lines)
      ..add(EnvLine.variable(key: 'NOVA_VARIAVEL', value: ''));
    setState(() => _lines = next);
    _emit(serializeEnvFile(next));
  }

  void _updateVariable(int index, {String? key, String? value}) {
    final next = List<EnvLine>.from(_lines);
    next[index] = next[index].copyWithVariable(key: key, value: value);
    setState(() => _lines = next);
    _emit(serializeEnvFile(next));
  }

  void _removeLine(int index) {
    final removedUid = _lines[index].uid;
    final next = List<EnvLine>.from(_lines)..removeAt(index);
    setState(() {
      _maskedUids.remove(removedUid);
      _lines = next;
    });
    _emit(serializeEnvFile(next));
  }

  @override
  Widget build(BuildContext context) {
    final variables = _lines.where((l) => l.isVariable).toList(growable: false);
    final emptyCount = variables.where((l) => l.value.trim().isEmpty).length;
    final term = _searchController.text.trim().toLowerCase();

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (widget.loadingTemplates) ...<Widget>[
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
                    'Procurando arquivos .env.example / .env no repositório...',
                    style: context.textTheme.bodySmall,
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 10),
        ] else if (widget.templates.isNotEmpty) ...<Widget>[
          NestedPanel(
            tone: SemanticTone.info,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Wrap(
                  spacing: 8,
                  runSpacing: 6,
                  alignment: WrapAlignment.spaceBetween,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: <Widget>[
                    Row(
                      mainAxisSize: MainAxisSize.min,
                      children: <Widget>[
                        const Icon(Icons.description_outlined, size: 17),
                        const SizedBox(width: 6),
                        Text(
                          widget.templates.length == 1
                              ? 'Modelo encontrado no repositório'
                              : 'Modelos encontrados no repositório (${widget.templates.length})',
                          style: context.textTheme.labelLarge?.copyWith(
                            fontWeight: FontWeight.w700,
                          ),
                        ),
                      ],
                    ),
                    Wrap(
                      spacing: 6,
                      runSpacing: 4,
                      crossAxisAlignment: WrapCrossAlignment.center,
                      children: <Widget>[
                        if (variables.isNotEmpty)
                          SemanticChip(
                            label: countNoun(variables.length, 'variável'),
                            tone: SemanticTone.primary,
                            compact: true,
                          ),
                        if (emptyCount > 0)
                          SemanticChip(
                            label: '$emptyCount em branco',
                            tone: SemanticTone.warning,
                            compact: true,
                          ),
                      ],
                    ),
                  ],
                ),
                const SizedBox(height: 8),
                Wrap(
                  spacing: 6,
                  runSpacing: 6,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: <Widget>[
                    for (final tpl in widget.templates)
                      ChoiceChip(
                        visualDensity: VisualDensity.compact,
                        selected: widget.activeTemplateName == tpl.name,
                        label: Text(tpl.name),
                        onSelected: (_) => widget.onSelectTemplate(tpl),
                      ),
                    if (widget.onReloadTemplates != null)
                      TextButton.icon(
                        onPressed: widget.onReloadTemplates,
                        icon: const Icon(Icons.refresh, size: 16),
                        label: const Text('Recarregar'),
                      ),
                    if (widget.content.trim().isNotEmpty)
                      TextButton(
                        onPressed: widget.onClear,
                        child: const Text('Limpar'),
                      ),
                  ],
                ),
                const SizedBox(height: 4),
                Text(
                  'Revise os valores e preencha senhas/segredos antes de concluir. O arquivo é salvo cifrado na central.',
                  style: context.textTheme.bodySmall,
                ),
              ],
            ),
          ),
          const SizedBox(height: 10),
        ] else if (widget.hasRepository) ...<Widget>[
          NestedPanel(
            tone: SemanticTone.secondary,
            child: Row(
              children: <Widget>[
                Expanded(
                  child: Text(
                    'Nenhum .env.example encontrado na raiz do repositório. Adicione variáveis abaixo ou cole no modo Texto .env.',
                    style: context.textTheme.bodySmall,
                  ),
                ),
                if (widget.onReloadTemplates != null) ...<Widget>[
                  const SizedBox(width: 8),
                  TextButton(
                    onPressed: widget.onReloadTemplates,
                    child: const Text('Procurar'),
                  ),
                ],
              ],
            ),
          ),
          const SizedBox(height: 10),
        ],
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
                  label: Text('Modo lista', maxLines: 1, softWrap: false),
                ),
                ButtonSegment<bool>(
                  value: true,
                  label: Text('Texto .env', maxLines: 1, softWrap: false),
                ),
              ],
              selected: <bool>{_rawMode},
              onSelectionChanged: (set) => _switchMode(set.first),
            ),
            if (!_rawMode)
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
            maxLines: 12,
            style: const TextStyle(fontFamily: 'monospace', fontSize: 13),
            decoration: const InputDecoration(
              labelText: 'Arquivo .env inicial (opcional — cifrado na central)',
              hintText: 'APP_ENV=production\nDATABASE_URL=postgres://...',
              alignLabelWithHint: true,
            ),
            onChanged: _emit,
          )
        else ...<Widget>[
          if (variables.length > 4) ...<Widget>[
            TextField(
              controller: _searchController,
              decoration: const InputDecoration(
                hintText: 'Filtrar chave...',
                prefixIcon: Icon(Icons.search, size: 18),
                isDense: true,
              ),
              onChanged: (_) => setState(() {}),
            ),
            const SizedBox(height: 8),
          ],
          if (variables.isEmpty)
            NestedPanel(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  Text(
                    'Nenhuma variável adicionada (opcional)',
                    style: context.textTheme.titleSmall?.copyWith(
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                  const SizedBox(height: 4),
                  Text(
                    'Você pode adicionar variáveis agora, colar um arquivo .env pronto ou configurar depois no alvo.',
                    style: context.textTheme.bodySmall,
                  ),
                  const SizedBox(height: 10),
                  Wrap(
                    spacing: 8,
                    runSpacing: 8,
                    children: <Widget>[
                      OutlinedButton.icon(
                        onPressed: _addVariable,
                        icon: const Icon(Icons.add, size: 16),
                        label: const Text('Adicionar variável'),
                      ),
                      TextButton.icon(
                        onPressed: () => _switchMode(true),
                        icon: const Icon(Icons.paste, size: 16),
                        label: const Text('Colar texto .env'),
                      ),
                    ],
                  ),
                ],
              ),
            )
          else
            ConstrainedBox(
              constraints: const BoxConstraints(maxHeight: 340),
              child: ListView(
                shrinkWrap: true,
                children: <Widget>[
                  for (var i = 0; i < _lines.length; i++)
                    if (_lines[i].isVariable &&
                        (term.isEmpty ||
                            _lines[i].key.toLowerCase().contains(term)))
                      Padding(
                        key: ValueKey<int>(_lines[i].uid),
                        padding: const EdgeInsets.only(bottom: 8),
                        child: _buildVarRow(i, _lines[i]),
                      ),
                ],
              ),
            ),
        ],
      ],
    );
  }

  Widget _buildVarRow(int index, EnvLine line) {
    final secret = looksLikeSecretKey(line.key);
    final masked = secret && _maskedUids.contains(line.uid);
    final validKey = isValidEnvKey(line.key);

    final keyField = TextFormField(
      key: ValueKey<String>('setup-env-k-${line.uid}'),
      initialValue: line.key,
      style: const TextStyle(fontFamily: 'monospace', fontSize: 13),
      decoration: InputDecoration(
        labelText: 'Chave',
        isDense: true,
        errorText: validKey ? null : 'Chave inválida',
      ),
      onChanged: (v) => _updateVariable(index, key: v),
    );

    final valueField = TextFormField(
      key: ValueKey<String>('setup-env-v-${line.uid}'),
      initialValue: line.value,
      obscureText: masked,
      style: const TextStyle(fontFamily: 'monospace', fontSize: 13),
      decoration: InputDecoration(
        labelText: secret ? 'Valor (segredo)' : 'Valor',
        hintText: line.value.trim().isEmpty ? 'preencha o valor' : null,
        isDense: true,
        suffixIcon: secret
            ? IconButton(
                tooltip: masked ? 'Mostrar' : 'Ocultar',
                icon: Icon(
                  masked ? Icons.visibility : Icons.visibility_off,
                  size: 18,
                ),
                onPressed: () => setState(() {
                  if (masked) {
                    _maskedUids.remove(line.uid);
                  } else {
                    _maskedUids.add(line.uid);
                  }
                }),
              )
            : null,
      ),
      onChanged: (v) => _updateVariable(index, value: v),
    );

    final deleteButton = IconButton(
      tooltip: 'Remover variável',
      icon: const Icon(Icons.delete_outline, size: 20),
      onPressed: () => _removeLine(index),
    );

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
                    deleteButton,
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
              deleteButton,
            ],
          );
        },
      ),
    );
  }
}
