import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/core/models/snmp_credential_models.dart';
import 'package:netmonitor_app/features/core/providers/snmp_credentials_provider.dart';
import 'package:netmonitor_app/features/network/widgets/snmp_credential_form_sheet.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Configurações › Rede › Credenciais SNMP: as comunidades e usuários v3 que
/// os dispositivos, os monitores e a descoberta usam.
class SnmpCredentialsCard extends ConsumerStatefulWidget {
  const SnmpCredentialsCard({super.key});

  @override
  ConsumerState<SnmpCredentialsCard> createState() =>
      _SnmpCredentialsCardState();
}

class _SnmpCredentialsCardState extends ConsumerState<SnmpCredentialsCard> {
  @override
  void initState() {
    super.initState();
    Future<void>.microtask(
      () => ref.read(snmpCredentialsProvider.notifier).fetchAll(),
    );
  }

  Future<void> _delete(SnmpCredentialModel credential) async {
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: 'Excluir credencial SNMP',
      message:
          'Excluir "${credential.name}"? Dispositivos que a usam impedem a '
          'exclusão — troque a credencial deles antes.',
      confirmLabel: 'Excluir',
      icon: Icons.delete_outline,
    );
    if (!confirmed || !mounted) return;
    try {
      await ref.read(snmpCredentialsProvider.notifier).remove(credential.id);
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Credencial "${credential.name}" excluída.')),
      );
    } catch (err) {
      if (!mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(describeError(err))));
    }
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(snmpCredentialsProvider);
    final canWrite = ref.watch(authProvider).canWriteIn(ProductArea.network);
    final addButton = canWrite
        ? FilledButton.icon(
            onPressed: () => showSnmpCredentialFormSheet(context: context),
            icon: const Icon(Icons.add, size: 18),
            label: const Text('Nova credencial'),
          )
        : null;

    return LayoutBuilder(
      builder: (context, constraints) {
        final isCompact = constraints.maxWidth < 520;
        return SectionCard(
          title: 'Credenciais SNMP',
          subtitle:
              'Comunidades (v1/v2c) e usuários v3 usados na coleta, nos '
              'monitores e na descoberta. Os segredos ficam cifrados e não '
              'voltam para a tela.',
          icon: Icons.key_outlined,
          trailing: isCompact ? null : addButton,
          footer: isCompact && addButton != null
              ? SymmetricalActionBar(children: <Widget>[addButton])
              : null,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              if (state.loading && !state.loaded)
                const LoadingStateView(message: 'Carregando credenciais...')
              else if (state.error != null && state.items.isEmpty)
                NestedPanel(tone: SemanticTone.error, child: Text(state.error!))
              else if (state.items.isEmpty)
                const NestedPanel(
                  tone: SemanticTone.info,
                  child: Text(
                    'Nenhuma credencial cadastrada. Sem nenhuma, o sistema usa '
                    'a comunidade "public" em v2c e cria a credencial padrão '
                    'no primeiro dispositivo com SNMP.',
                  ),
                )
              else
                for (final credential in state.items) ...<Widget>[
                  _CredentialTile(
                    credential: credential,
                    canWrite: canWrite,
                    onEdit: () => showSnmpCredentialFormSheet(
                      context: context,
                      existing: credential,
                    ),
                    onDelete: () => _delete(credential),
                  ),
                  const SizedBox(height: 8),
                ],
            ],
          ),
        );
      },
    );
  }
}

class _CredentialTile extends StatelessWidget {
  const _CredentialTile({
    required this.credential,
    required this.canWrite,
    required this.onEdit,
    required this.onDelete,
  });

  final SnmpCredentialModel credential;
  final bool canWrite;
  final VoidCallback onEdit;
  final VoidCallback onDelete;

  @override
  Widget build(BuildContext context) {
    final secure = credential.securityLevel == 'authPriv';
    return NestedPanel(
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text(
                  credential.name,
                  style: context.textTheme.titleSmall?.copyWith(
                    fontWeight: FontWeight.w700,
                  ),
                ),
                const SizedBox(height: 6),
                Wrap(
                  spacing: 6,
                  runSpacing: 6,
                  children: <Widget>[
                    SemanticChip(
                      label: credential.version.toUpperCase(),
                      tone: SemanticTone.primary,
                      compact: true,
                    ),
                    SemanticChip(
                      label: snmpSecurityLevelLabel(credential.securityLevel),
                      tone: secure ? SemanticTone.success : SemanticTone.info,
                      compact: true,
                    ),
                    if (credential.isDefault)
                      const SemanticChip(
                        label: 'Padrão',
                        tone: SemanticTone.secondary,
                        icon: Icons.star_outline,
                        compact: true,
                      ),
                    SemanticChip(
                      label: countNoun(
                        credential.deviceCount,
                        'dispositivo',
                        'dispositivos',
                      ),
                      tone: SemanticTone.info,
                      compact: true,
                    ),
                  ],
                ),
                if (credential.isV3 && credential.username != null) ...<Widget>[
                  const SizedBox(height: 6),
                  Text(
                    'Usuário: ${credential.username}',
                    style: context.textTheme.bodySmall,
                  ),
                ],
              ],
            ),
          ),
          if (canWrite) ...<Widget>[
            IconButton(
              tooltip: 'Editar',
              color: context.colorScheme.primary,
              onPressed: onEdit,
              icon: const Icon(Icons.edit_outlined),
            ),
            IconButton(
              tooltip: 'Excluir',
              color: context.colorScheme.error,
              onPressed: onDelete,
              icon: const Icon(Icons.delete_outline),
            ),
          ],
        ],
      ),
    );
  }
}
