import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/features/ai/models/mcp_models.dart';
import 'package:netmonitor_app/features/ai/providers/mcp_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

const List<int> _mcpTtlDayOptions = <int>[30, 90, 180, 365];
const Set<String> _deployScopeIds = <String>{
  'deploys:read',
  'deploys:write',
  'deploys:approve',
};

enum _McpTokenPreset { projectDeploy, readOnly, custom }

/// Abre o formulário adaptativo para criar um token pessoal MCP com escopos granulares.
Future<McpTokenCreated?> showCreateMcpTokenSheet(
  BuildContext context, {
  required McpOverview overview,
}) {
  return showAdaptiveFormSheet<McpTokenCreated>(
    context: context,
    title: 'Novo token MCP',
    subtitle: 'Conceda apenas os escopos necessários para o agente externo.',
    maxWidth: 620,
    builder: (_) => _CreateMcpTokenForm(overview: overview),
  );
}

class _CreateMcpTokenForm extends ConsumerStatefulWidget {
  const _CreateMcpTokenForm({required this.overview});

  final McpOverview overview;

  @override
  ConsumerState<_CreateMcpTokenForm> createState() =>
      _CreateMcpTokenFormState();
}

class _CreateMcpTokenFormState extends ConsumerState<_CreateMcpTokenForm> {
  final TextEditingController _nameCtrl = TextEditingController();
  late final Set<String> _selectedScopes;
  late _McpTokenPreset _preset;
  bool _allApplications = true;
  final Set<int> _selectedApplicationIds = <int>{};
  int _ttlDays = 90;
  bool _submitting = false;
  String? _error;

  bool _isScopeGranted(String scopeId) {
    final desc = widget.overview.scopeById(scopeId);
    return desc != null && desc.grantedByRole;
  }

  bool get _canUseProjectDeployPreset =>
      _isScopeGranted('deploys:read') && _isScopeGranted('deploys:write');

  Set<String> _readOnlyPresetScopes() {
    final target = <String>{};
    if (_isScopeGranted('read')) target.add('read');
    if (_isScopeGranted('deploys:read')) target.add('deploys:read');
    if (target.isEmpty) {
      for (final scope in widget.overview.scopes) {
        if (scope.grantedByRole) {
          target.add(scope.id);
          break;
        }
      }
    }
    return target;
  }

  bool get _hasDeployScopeSelected =>
      _selectedScopes.any(_deployScopeIds.contains);

  @override
  void initState() {
    super.initState();
    final readOnlyScopes = _readOnlyPresetScopes();
    if (readOnlyScopes.isNotEmpty) {
      _selectedScopes = Set<String>.from(readOnlyScopes);
      _preset = _McpTokenPreset.readOnly;
    } else {
      _selectedScopes = <String>{};
      _preset = _McpTokenPreset.custom;
    }
  }

  @override
  void dispose() {
    _nameCtrl.dispose();
    super.dispose();
  }

  void _applyPreset(_McpTokenPreset preset) {
    setState(() {
      _preset = preset;
      _error = null;
      switch (preset) {
        case _McpTokenPreset.projectDeploy:
          if (_canUseProjectDeployPreset) {
            _selectedScopes
              ..clear()
              ..addAll(const <String>['deploys:read', 'deploys:write']);
          }
        case _McpTokenPreset.readOnly:
          _selectedScopes
            ..clear()
            ..addAll(_readOnlyPresetScopes());
        case _McpTokenPreset.custom:
          break;
      }
    });
  }

  void _toggleScope(String scopeId, bool selected) {
    setState(() {
      _error = null;
      if (selected) {
        _selectedScopes.add(scopeId);
      } else {
        _selectedScopes.remove(scopeId);
      }
      _preset = _detectMatchingPreset();
    });
  }

  _McpTokenPreset _detectMatchingPreset() {
    if (_canUseProjectDeployPreset &&
        _selectedScopes.length == 2 &&
        _selectedScopes.contains('deploys:read') &&
        _selectedScopes.contains('deploys:write')) {
      return _McpTokenPreset.projectDeploy;
    }
    final ro = _readOnlyPresetScopes();
    if (ro.isNotEmpty &&
        _selectedScopes.length == ro.length &&
        _selectedScopes.containsAll(ro)) {
      return _McpTokenPreset.readOnly;
    }
    return _McpTokenPreset.custom;
  }

  Future<void> _submit() async {
    if (_submitting) return;
    final name = _nameCtrl.text.trim();
    if (name.isEmpty) {
      setState(() => _error = 'Informe um nome para identificar o token.');
      return;
    }
    if (_selectedScopes.isEmpty) {
      setState(() => _error = 'Selecione ao menos um escopo para o token.');
      return;
    }
    final showAppFilter =
        _hasDeployScopeSelected &&
        widget.overview.deployApplications.isNotEmpty;
    if (showAppFilter && !_allApplications && _selectedApplicationIds.isEmpty) {
      setState(
        () => _error =
            'Selecione ao menos uma aplicação de deploy ou permita todas.',
      );
      return;
    }

    setState(() {
      _submitting = true;
      _error = null;
    });

    try {
      final orderedScopes = widget.overview.scopes
          .where((s) => _selectedScopes.contains(s.id))
          .map((s) => s.id)
          .toList(growable: false);
      final List<int>? allowedAppIds = showAppFilter && !_allApplications
          ? (_selectedApplicationIds.toList()..sort())
          : null;

      final created = await ref
          .read(mcpProvider.notifier)
          .createToken(
            name: name,
            scopes: orderedScopes.isNotEmpty
                ? orderedScopes
                : _selectedScopes.toList(growable: false),
            allowedApplicationIds: allowedAppIds,
            ttlDays: _ttlDays,
          );
      if (!mounted) return;
      Navigator.of(context).pop(created);
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _submitting = false;
        _error = describeError(err, fallback: 'Erro ao criar token MCP.');
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final hasAdminLockedScopes = widget.overview.scopes.any(
      (scope) => !scope.grantedByRole && scope.requiresAdmin,
    );
    final hasOtherLockedScopes = widget.overview.scopes.any(
      (scope) => !scope.grantedByRole && !scope.requiresAdmin,
    );
    final hasLockedScopes = hasAdminLockedScopes || hasOtherLockedScopes;
    final showAppRestriction =
        _hasDeployScopeSelected &&
        widget.overview.deployApplications.isNotEmpty;
    final safeTtl = _mcpTtlDayOptions.contains(_ttlDays) ? _ttlDays : 90;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        TextField(
          controller: _nameCtrl,
          autofocus: true,
          textInputAction: TextInputAction.next,
          decoration: const InputDecoration(
            labelText: 'Nome do token',
            hintText: 'Antigravity — meu-projeto',
          ),
        ),
        const SizedBox(height: 14),
        Text(
          'Perfil rápido',
          style: context.textTheme.labelLarge?.copyWith(
            fontWeight: FontWeight.w700,
          ),
        ),
        const SizedBox(height: 8),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: <Widget>[
            ChoiceChip(
              label: const Text('Deploy de projeto'),
              selected: _preset == _McpTokenPreset.projectDeploy,
              onSelected: _canUseProjectDeployPreset
                  ? (_) => _applyPreset(_McpTokenPreset.projectDeploy)
                  : null,
            ),
            ChoiceChip(
              label: const Text('Somente leitura'),
              selected: _preset == _McpTokenPreset.readOnly,
              onSelected: _readOnlyPresetScopes().isNotEmpty
                  ? (_) => _applyPreset(_McpTokenPreset.readOnly)
                  : null,
            ),
            ChoiceChip(
              label: const Text('Personalizado'),
              selected: _preset == _McpTokenPreset.custom,
              onSelected: (_) => _applyPreset(_McpTokenPreset.custom),
            ),
          ],
        ),
        const SizedBox(height: 14),
        Text(
          'Escopos permitidos',
          style: context.textTheme.labelLarge?.copyWith(
            fontWeight: FontWeight.w700,
          ),
        ),
        const SizedBox(height: 8),
        if (widget.overview.scopes.isEmpty)
          const NestedPanel(
            tone: SemanticTone.warning,
            child: Text('Nenhum escopo MCP disponível para o seu usuário.'),
          )
        else
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: widget.overview.scopes
                .map((scope) {
                  final granted = scope.grantedByRole;
                  final selected =
                      granted && _selectedScopes.contains(scope.id);
                  final lockReason = scope.requiresAdmin
                      ? 'Requer papel Admin'
                      : 'Indisponível para o seu papel ou área atual';
                  final chipSuffix = scope.requiresAdmin
                      ? 'Requer Admin'
                      : 'Indisponível';
                  final tooltipMsg = granted
                      ? (scope.description.isNotEmpty
                            ? scope.description
                            : scope.id)
                      : '$lockReason${scope.description.isNotEmpty ? ' — ${scope.description}' : ''}';
                  return Tooltip(
                    message: tooltipMsg,
                    child: FilterChip(
                      selected: selected,
                      label: Text(
                        granted ? scope.label : '${scope.label} ($chipSuffix)',
                      ),
                      onSelected: granted
                          ? (checked) => _toggleScope(scope.id, checked)
                          : null,
                    ),
                  );
                })
                .toList(growable: false),
          ),
        if (hasLockedScopes) ...<Widget>[
          const SizedBox(height: 6),
          Text(
            hasAdminLockedScopes && hasOtherLockedScopes
                ? 'Escopos marcados com "Requer Admin" ou "Indisponível" estão bloqueados para o seu papel ou permissões de área atuais.'
                : hasAdminLockedScopes
                ? 'Escopos marcados com "Requer Admin" estão indisponíveis para o seu papel atual.'
                : 'Escopos marcados com "Indisponível" não estão liberados para o seu papel ou permissões de área atuais.',
            style: context.textTheme.bodySmall?.copyWith(
              color: scheme.onSurfaceVariant,
            ),
          ),
        ],
        if (_selectedScopes.isNotEmpty) ...<Widget>[
          const SizedBox(height: 10),
          NestedPanel(
            padding: const EdgeInsets.all(10),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: widget.overview.scopes
                  .where((s) => _selectedScopes.contains(s.id))
                  .map(
                    (s) => Padding(
                      padding: const EdgeInsets.only(bottom: 4),
                      child: Text(
                        '• ${s.label} (${s.id})${s.description.isNotEmpty ? ': ${s.description}' : ''}',
                        style: context.textTheme.bodySmall,
                      ),
                    ),
                  )
                  .toList(growable: false),
            ),
          ),
        ],
        if (showAppRestriction) ...<Widget>[
          const SizedBox(height: 14),
          NestedPanel(
            padding: const EdgeInsets.fromLTRB(12, 8, 12, 12),
            child: Material(
              type: MaterialType.transparency,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  Text(
                    'Restringir a aplicações de deploy',
                    style: context.textTheme.labelLarge?.copyWith(
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                  SwitchListTile(
                    contentPadding: EdgeInsets.zero,
                    dense: true,
                    title: const Text('Todas as aplicações permitidas'),
                    subtitle: Text(
                      _allApplications
                          ? 'O token poderá operar em qualquer aplicação de deploy.'
                          : 'Selecione abaixo apenas as aplicações que este token pode acessar.',
                    ),
                    value: _allApplications,
                    onChanged: (value) => setState(() {
                      _allApplications = value;
                      _error = null;
                    }),
                  ),
                  if (!_allApplications) ...<Widget>[
                    const SizedBox(height: 6),
                    Wrap(
                      spacing: 8,
                      runSpacing: 8,
                      children: widget.overview.deployApplications
                          .map((app) {
                            final selected = _selectedApplicationIds.contains(
                              app.id,
                            );
                            return Tooltip(
                              message: app.repository.isNotEmpty
                                  ? app.repository
                                  : app.slug,
                              child: FilterChip(
                                selected: selected,
                                avatar: const Icon(
                                  Icons.rocket_launch_outlined,
                                  size: 16,
                                ),
                                label: Text(app.name),
                                onSelected: (checked) => setState(() {
                                  _error = null;
                                  if (checked) {
                                    _selectedApplicationIds.add(app.id);
                                  } else {
                                    _selectedApplicationIds.remove(app.id);
                                  }
                                }),
                              ),
                            );
                          })
                          .toList(growable: false),
                    ),
                  ],
                ],
              ),
            ),
          ),
        ],
        const SizedBox(height: 14),
        AppDropdownFormField<int>(
          initialValue: safeTtl,
          decoration: const InputDecoration(labelText: 'Validade do token'),
          items: _mcpTtlDayOptions
              .map(
                (days) => DropdownMenuItem<int>(
                  value: days,
                  child: Text('$days dias'),
                ),
              )
              .toList(growable: false),
          onChanged: (value) {
            if (value != null) {
              setState(() => _ttlDays = value);
            }
          },
        ),
        if (_error != null) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
        ],
        const SizedBox(height: 16),
        Align(
          alignment: Alignment.centerRight,
          child: FilledButton.icon(
            onPressed: _submitting ? null : _submit,
            icon: _submitting
                ? const SizedBox(
                    width: 16,
                    height: 16,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.vpn_key_outlined, size: 18),
            label: const Text('Criar token'),
          ),
        ),
      ],
    );
  }
}
