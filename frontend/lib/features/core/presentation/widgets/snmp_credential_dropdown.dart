import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/features/core/providers/snmp_credentials_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';

/// Seletor da credencial SNMP de um equipamento.
///
/// `null` é "a padrão do sistema" — o backend grava a padrão no cadastro.
/// O valor inicial é conferido contra a lista carregada (AGENTS §2): uma
/// credencial que não está mais lá vira "padrão" em vez de quebrar o campo.
class SnmpCredentialDropdown extends ConsumerStatefulWidget {
  const SnmpCredentialDropdown({
    super.key,
    required this.value,
    required this.onChanged,
    this.labelText = 'Credencial SNMP',
    this.helperText,
    this.enabled = true,
  });

  final int? value;
  final ValueChanged<int?> onChanged;
  final String labelText;
  final String? helperText;
  final bool enabled;

  @override
  ConsumerState<SnmpCredentialDropdown> createState() =>
      _SnmpCredentialDropdownState();
}

class _SnmpCredentialDropdownState
    extends ConsumerState<SnmpCredentialDropdown> {
  @override
  void initState() {
    super.initState();
    Future<void>.microtask(
      () => ref.read(snmpCredentialsProvider.notifier).fetchAll(),
    );
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(snmpCredentialsProvider);
    if (!state.loaded && state.loading) {
      return InputDecorator(
        decoration: InputDecoration(labelText: widget.labelText),
        child: const LinearProgressIndicator(minHeight: 2),
      );
    }
    final defaultName = state.defaultCredential?.name;
    final selected = state.byId(widget.value)?.id;
    final items = <DropdownMenuItem<int?>>[
      DropdownMenuItem<int?>(
        value: null,
        child: Text(
          defaultName == null
              ? 'Padrão do sistema (public, v2c)'
              : 'Padrão do sistema ($defaultName)',
        ),
      ),
      for (final credential in state.items)
        DropdownMenuItem<int?>(
          value: credential.id,
          child: Text('${credential.name} — ${credential.summary}'),
        ),
    ];
    return AppDropdownFormField<int?>(
      // A lista chega depois do primeiro quadro: a chave refaz o campo com o
      // valor certo quando ela muda.
      key: ValueKey<String>(
        '${state.items.map((item) => item.id).join(',')}|$selected',
      ),
      initialValue: selected,
      decoration: InputDecoration(
        labelText: widget.labelText,
        helperText:
            widget.helperText ??
            'Cadastre comunidades e usuários v3 em Configurações › Rede.',
        helperMaxLines: 2,
      ),
      items: items,
      onChanged: widget.enabled ? widget.onChanged : null,
    );
  }
}
