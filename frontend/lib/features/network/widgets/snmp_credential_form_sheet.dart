import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/core/models/snmp_credential_models.dart';
import 'package:netmonitor_app/features/core/providers/snmp_credentials_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Abre o formulário de criação (sem `existing`) ou edição de uma credencial.
Future<SnmpCredentialModel?> showSnmpCredentialFormSheet({
  required BuildContext context,
  SnmpCredentialModel? existing,
}) {
  return showAdaptiveFormSheet<SnmpCredentialModel>(
    context: context,
    title: existing == null ? 'Nova credencial SNMP' : 'Editar credencial SNMP',
    subtitle: existing == null
        ? 'Comunidade (v1/v2c) ou usuário SNMPv3'
        : existing.name,
    builder: (_) => SnmpCredentialForm(existing: existing),
  );
}

/// O formulário em si. Os segredos nunca vêm preenchidos: na edição, campo em
/// branco mantém o que está gravado.
class SnmpCredentialForm extends ConsumerStatefulWidget {
  const SnmpCredentialForm({super.key, this.existing});

  final SnmpCredentialModel? existing;

  @override
  ConsumerState<SnmpCredentialForm> createState() => _SnmpCredentialFormState();
}

class _SnmpCredentialFormState extends ConsumerState<SnmpCredentialForm> {
  final _formKey = GlobalKey<FormState>();
  late final TextEditingController _nameCtrl;
  late final TextEditingController _communityCtrl;
  late final TextEditingController _usernameCtrl;
  late final TextEditingController _authKeyCtrl;
  late final TextEditingController _privKeyCtrl;
  late final TextEditingController _contextCtrl;
  final TextEditingController _testHostCtrl = TextEditingController();

  late String _version;
  String? _authProtocol;
  String? _privProtocol;
  late bool _isDefault;
  bool _saving = false;
  bool _testing = false;
  String? _error;
  String? _testMessage;
  bool _testOk = false;

  SnmpCredentialModel? get _existing => widget.existing;

  @override
  void initState() {
    super.initState();
    final e = _existing;
    _nameCtrl = TextEditingController(text: e?.name ?? '');
    _communityCtrl = TextEditingController();
    _usernameCtrl = TextEditingController(text: e?.username ?? '');
    _authKeyCtrl = TextEditingController();
    _privKeyCtrl = TextEditingController();
    _contextCtrl = TextEditingController(text: e?.contextName ?? '');
    _version = kSnmpVersionOptions.any((option) => option.$1 == e?.version)
        ? e!.version
        : 'v2c';
    _authProtocol = _validOption(kSnmpAuthProtocols, e?.authProtocol);
    _privProtocol = _authProtocol == null
        ? null
        : _validOption(kSnmpPrivProtocols, e?.privProtocol);
    _isDefault = e?.isDefault ?? false;
  }

  /// O valor só vale se estiver nas opções (AGENTS §2: o `initialValue` do
  /// dropdown precisa existir nos itens).
  static String? _validOption(List<(String, String)> options, String? value) =>
      options.any((option) => option.$1 == value) ? value : null;

  @override
  void dispose() {
    _nameCtrl.dispose();
    _communityCtrl.dispose();
    _usernameCtrl.dispose();
    _authKeyCtrl.dispose();
    _privKeyCtrl.dispose();
    _contextCtrl.dispose();
    _testHostCtrl.dispose();
    super.dispose();
  }

  bool get _isV3 => _version == 'v3';

  /// Na edição, o segredo já gravado pode ficar em branco.
  String? _keepHint(bool stored) =>
      _existing != null && stored ? 'Deixe em branco para manter' : null;

  String? _secretValidator(String? value, {required bool stored}) {
    final text = value ?? '';
    if (text.isEmpty) {
      return stored && _existing != null ? null : 'Obrigatória';
    }
    if (text.length < 8) return 'Mínimo de 8 caracteres';
    return null;
  }

  SnmpCredentialDraft _draft() => SnmpCredentialDraft(
    name: _nameCtrl.text,
    version: _version,
    community: _communityCtrl.text,
    username: _usernameCtrl.text,
    authProtocol: _authProtocol,
    authKey: _authKeyCtrl.text,
    privProtocol: _privProtocol,
    privKey: _privKeyCtrl.text,
    contextName: _contextCtrl.text,
    isDefault: _isDefault,
  );

  Future<void> _submit() async {
    if (!_formKey.currentState!.validate()) return;
    setState(() {
      _saving = true;
      _error = null;
    });
    final notifier = ref.read(snmpCredentialsProvider.notifier);
    try {
      final existing = _existing;
      final saved = existing == null
          ? await notifier.create(_draft())
          : await notifier.save(existing.id, _draft());
      if (mounted) Navigator.of(context).pop(saved);
    } catch (err) {
      if (mounted) setState(() => _error = describeError(err));
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  Future<void> _test() async {
    final existing = _existing;
    final host = _testHostCtrl.text.trim();
    if (existing == null || host.isEmpty) return;
    setState(() {
      _testing = true;
      _testMessage = null;
    });
    try {
      final result = await ref
          .read(snmpCredentialsProvider.notifier)
          .testAgainst(existing.id, host);
      final system = asJsonMap(result['system']);
      final ok = result['success'] == true;
      if (!mounted) return;
      setState(() {
        _testOk = ok;
        _testMessage = ok
            ? <String>[
                'SNMP OK',
                if (system['sysName'] != null) '${system['sysName']}',
                if (system['sysDescr'] != null) '${system['sysDescr']}',
              ].join(' · ')
            : (result['message'] ?? 'Sem resposta SNMP.').toString();
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _testOk = false;
        _testMessage = describeError(err);
      });
    } finally {
      if (mounted) setState(() => _testing = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final e = _existing;
    return Form(
      key: _formKey,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          TextFormField(
            controller: _nameCtrl,
            maxLength: 120,
            decoration: const InputDecoration(
              labelText: 'Nome',
              hintText: 'Ex.: Switches do core',
            ),
            validator: (value) =>
                (value ?? '').trim().isEmpty ? 'Informe um nome' : null,
          ),
          const SizedBox(height: 8),
          AppDropdownFormField<String>(
            initialValue: _version,
            decoration: const InputDecoration(labelText: 'Versão'),
            items: <DropdownMenuItem<String>>[
              for (final (value, label) in kSnmpVersionOptions)
                DropdownMenuItem<String>(value: value, child: Text(label)),
            ],
            onChanged: (value) {
              if (value != null) setState(() => _version = value);
            },
          ),
          const SizedBox(height: 12),
          if (!_isV3)
            TextFormField(
              controller: _communityCtrl,
              obscureText: true,
              autocorrect: false,
              enableSuggestions: false,
              decoration: InputDecoration(
                labelText: 'Comunidade',
                helperText: _keepHint(e?.hasCommunity ?? false),
              ),
              validator: (value) =>
                  (value ?? '').trim().isEmpty &&
                      !(e != null && e.hasCommunity && !e.isV3)
                  ? 'Informe a comunidade'
                  : null,
            )
          else ...<Widget>[
            TextFormField(
              controller: _usernameCtrl,
              autocorrect: false,
              decoration: const InputDecoration(labelText: 'Usuário (v3)'),
              validator: (value) =>
                  (value ?? '').trim().isEmpty ? 'Informe o usuário' : null,
            ),
            const SizedBox(height: 12),
            ResponsiveFieldRow(
              breakpoint: 480,
              spacing: 12,
              runSpacing: 12,
              children: <Widget>[
                AppDropdownFormField<String?>(
                  initialValue: _authProtocol,
                  decoration: const InputDecoration(labelText: 'Autenticação'),
                  items: <DropdownMenuItem<String?>>[
                    const DropdownMenuItem<String?>(
                      value: null,
                      child: Text('Sem autenticação'),
                    ),
                    for (final (value, label) in kSnmpAuthProtocols)
                      DropdownMenuItem<String?>(
                        value: value,
                        child: Text(label),
                      ),
                  ],
                  onChanged: (value) => setState(() {
                    _authProtocol = value;
                    if (value == null) _privProtocol = null;
                  }),
                ),
                if (_authProtocol != null)
                  TextFormField(
                    controller: _authKeyCtrl,
                    obscureText: true,
                    autocorrect: false,
                    enableSuggestions: false,
                    decoration: InputDecoration(
                      labelText: 'Senha de autenticação',
                      helperText: _keepHint(e?.hasAuthKey ?? false),
                    ),
                    validator: (value) =>
                        _secretValidator(value, stored: e?.hasAuthKey ?? false),
                  ),
              ],
            ),
            if (_authProtocol != null) ...<Widget>[
              const SizedBox(height: 12),
              ResponsiveFieldRow(
                breakpoint: 480,
                spacing: 12,
                runSpacing: 12,
                children: <Widget>[
                  AppDropdownFormField<String?>(
                    // A chave refaz o campo quando a autenticação é desligada
                    // e religada (a privacidade volta para "sem").
                    key: ValueKey<String?>('priv-$_authProtocol'),
                    initialValue: _privProtocol,
                    decoration: const InputDecoration(
                      labelText: 'Criptografia',
                    ),
                    items: <DropdownMenuItem<String?>>[
                      const DropdownMenuItem<String?>(
                        value: null,
                        child: Text('Sem criptografia'),
                      ),
                      for (final (value, label) in kSnmpPrivProtocols)
                        DropdownMenuItem<String?>(
                          value: value,
                          child: Text(label),
                        ),
                    ],
                    onChanged: (value) => setState(() => _privProtocol = value),
                  ),
                  if (_privProtocol != null)
                    TextFormField(
                      controller: _privKeyCtrl,
                      obscureText: true,
                      autocorrect: false,
                      enableSuggestions: false,
                      decoration: InputDecoration(
                        labelText: 'Senha de criptografia',
                        helperText: _keepHint(e?.hasPrivKey ?? false),
                      ),
                      validator: (value) => _secretValidator(
                        value,
                        stored: e?.hasPrivKey ?? false,
                      ),
                    ),
                ],
              ),
            ],
            const SizedBox(height: 12),
            TextFormField(
              controller: _contextCtrl,
              autocorrect: false,
              decoration: const InputDecoration(
                labelText: 'Contexto (opcional)',
                helperText: 'contextName do SNMPv3; em branco usa o padrão',
              ),
            ),
          ],
          const SizedBox(height: 8),
          Material(
            type: MaterialType.transparency,
            child: SwitchListTile(
              contentPadding: EdgeInsets.zero,
              title: const Text('Credencial padrão'),
              subtitle: const Text(
                'Usada por dispositivo com SNMP sem credencial escolhida e '
                'tentada primeiro na descoberta.',
              ),
              value: _isDefault,
              onChanged: (value) => setState(() => _isDefault = value),
            ),
          ),
          if (e != null) ...<Widget>[
            const SizedBox(height: 8),
            NestedPanel(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  Text(
                    'Testar a credencial gravada',
                    style: context.textTheme.titleSmall?.copyWith(
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                  const SizedBox(height: 8),
                  Row(
                    children: <Widget>[
                      Expanded(
                        child: TextField(
                          controller: _testHostCtrl,
                          decoration: const InputDecoration(
                            labelText: 'IP do equipamento',
                          ),
                        ),
                      ),
                      const SizedBox(width: 8),
                      FilledButton.tonalIcon(
                        onPressed: _testing ? null : _test,
                        icon: _testing
                            ? const SizedBox(
                                width: 14,
                                height: 14,
                                child: CircularProgressIndicator(
                                  strokeWidth: 2,
                                ),
                              )
                            : const Icon(Icons.network_check, size: 18),
                        label: const Text('Testar'),
                      ),
                    ],
                  ),
                  if (_testMessage != null) ...<Widget>[
                    const SizedBox(height: 8),
                    NestedPanel(
                      tone: _testOk ? SemanticTone.success : SemanticTone.error,
                      padding: const EdgeInsets.all(10),
                      child: Text(
                        _testMessage!,
                        style: context.textTheme.bodySmall,
                      ),
                    ),
                  ],
                ],
              ),
            ),
          ],
          if (_error != null) ...<Widget>[
            const SizedBox(height: 12),
            NestedPanel(
              tone: SemanticTone.error,
              padding: const EdgeInsets.all(10),
              child: Text(_error!, style: context.textTheme.bodySmall),
            ),
          ],
          const SizedBox(height: 16),
          SymmetricalActionBar(
            children: <Widget>[
              OutlinedButton(
                onPressed: _saving ? null : () => Navigator.of(context).pop(),
                child: const Text('Cancelar'),
              ),
              FilledButton.icon(
                onPressed: _saving ? null : _submit,
                icon: _saving
                    ? const SizedBox(
                        width: 14,
                        height: 14,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(Icons.save_outlined, size: 18),
                label: const Text('Salvar'),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
