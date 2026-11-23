import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/config/server_config.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/features/servers/models/deploy_models.dart';
import 'package:netmonitor_app/features/servers/providers/deploys_provider.dart';
import 'package:netmonitor_app/features/servers/utils/github_manifest_submit.dart';
import 'package:netmonitor_app/features/servers/utils/server_utils.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';
import 'package:url_launcher/url_launcher.dart';

/// Diálogo de Conexões GitHub (GitHub App via manifest + tokens PAT — ADR 021).
Future<void> showDeployConnectionsDialog(BuildContext context) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Conexões com o GitHub',
    subtitle: 'GitHub App da central (recomendado) ou token clássico PAT',
    maxWidth: 740,
    builder: (_) => const _DeployConnectionsBody(),
  );
}

class _DeployConnectionsBody extends ConsumerStatefulWidget {
  const _DeployConnectionsBody();

  @override
  ConsumerState<_DeployConnectionsBody> createState() =>
      _DeployConnectionsBodyState();
}

class _DeployConnectionsBodyState
    extends ConsumerState<_DeployConnectionsBody> {
  final TextEditingController _orgCtrl = TextEditingController();
  bool _allowWrite = true;
  bool _creatingManifest = false;
  DeployGithubManifestView? _manifestReady;

  final TextEditingController _credNameCtrl = TextEditingController();
  final TextEditingController _credRegistryCtrl = TextEditingController(
    text: 'ghcr.io',
  );
  final TextEditingController _credUserCtrl = TextEditingController();
  final TextEditingController _credSecretCtrl = TextEditingController();
  bool _savingCred = false;
  String? _error;

  @override
  void dispose() {
    _orgCtrl.dispose();
    _credNameCtrl.dispose();
    _credRegistryCtrl.dispose();
    _credUserCtrl.dispose();
    _credSecretCtrl.dispose();
    super.dispose();
  }

  Future<void> _startGithubManifest() async {
    setState(() {
      _creatingManifest = true;
      _error = null;
    });
    try {
      final origin = resolveCentralOrigin(
        ref.read(serverConfigProvider).baseUrl,
      );
      final manifest = await ref
          .read(deploysProvider.notifier)
          .githubManifest(
            origin: origin,
            organization: _orgCtrl.text.trim(),
            allowWrite: _allowWrite,
          );
      if (!mounted) return;
      setState(() {
        _manifestReady = manifest;
        _creatingManifest = false;
      });
      final submitted = submitGithubManifestForm(
        actionUrl: manifest.actionUrl,
        manifestJson: manifest.manifest,
      );
      if (!submitted) {
        final uri = Uri.tryParse(manifest.actionUrl);
        if (uri != null) {
          await launchUrl(uri, mode: LaunchMode.externalApplication);
        }
      }
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = describeError(err, fallback: 'Erro ao gerar manifest GitHub.');
        _creatingManifest = false;
      });
    }
  }

  Future<void> _saveCredential() async {
    if (_credNameCtrl.text.trim().isEmpty ||
        _credUserCtrl.text.trim().isEmpty) {
      setState(() => _error = 'Informe nome e usuário da credencial.');
      return;
    }
    setState(() {
      _savingCred = true;
      _error = null;
    });
    try {
      await ref
          .read(deploysProvider.notifier)
          .saveCredential(
            null,
            DeployCredentialInput(
              name: _credNameCtrl.text.trim(),
              registry: _credRegistryCtrl.text.trim(),
              username: _credUserCtrl.text.trim(),
              secret: _credSecretCtrl.text,
            ),
          );
      if (!mounted) return;
      _credNameCtrl.clear();
      _credUserCtrl.clear();
      _credSecretCtrl.clear();
      setState(() => _savingCred = false);
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = describeError(err, fallback: 'Erro ao salvar credencial.');
        _savingCred = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final deploys = ref.watch(deploysProvider);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Text(
          'GitHub App da central',
          style: context.textTheme.titleSmall?.copyWith(
            fontWeight: FontWeight.w800,
          ),
        ),
        const SizedBox(height: 6),
        Text(
          'A chave privada do GitHub App nunca sai da central; o agente recebe apenas tokens de 1h somente leitura.',
          style: context.textTheme.bodySmall?.copyWith(
            color: context.colorScheme.onSurfaceVariant,
          ),
        ),
        const SizedBox(height: 10),
        if (deploys.githubConnections.isNotEmpty) ...<Widget>[
          ...deploys.githubConnections.map(
            (conn) => Padding(
              padding: const EdgeInsets.only(bottom: 8),
              child: NestedPanel(
                tone: conn.installed
                    ? SemanticTone.success
                    : SemanticTone.warning,
                child: Row(
                  children: <Widget>[
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          Text(
                            '${conn.account} (${conn.appSlug})',
                            style: const TextStyle(fontWeight: FontWeight.w700),
                          ),
                          Text(
                            conn.installed
                                ? 'Instalado nos repositórios'
                                : 'Falta instalar nos repositórios',
                            style: context.textTheme.bodySmall,
                          ),
                        ],
                      ),
                    ),
                    Wrap(
                      spacing: 4,
                      children: <Widget>[
                        if (!conn.installed && conn.installUrl.isNotEmpty)
                          FilledButton.tonal(
                            onPressed: () => launchUrl(
                              Uri.parse(conn.installUrl),
                              mode: LaunchMode.externalApplication,
                            ),
                            child: const Text('Instalar'),
                          ),
                        IconButton(
                          tooltip: 'Atualizar',
                          icon: const Icon(Icons.refresh, size: 18),
                          onPressed: () => ref
                              .read(deploysProvider.notifier)
                              .refreshGithub(conn.id),
                        ),
                        IconButton(
                          tooltip: 'Remover',
                          icon: const Icon(Icons.delete_outline, size: 18),
                          onPressed: () => ref
                              .read(deploysProvider.notifier)
                              .removeGithub(conn.id),
                        ),
                      ],
                    ),
                  ],
                ),
              ),
            ),
          ),
          const SizedBox(height: 10),
        ],
        NestedPanel(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              TextField(
                controller: _orgCtrl,
                decoration: const InputDecoration(
                  labelText:
                      'Organização GitHub (opcional — deixe em branco para conta pessoal)',
                ),
              ),
              const SizedBox(height: 6),
              SwitchListTile(
                contentPadding: EdgeInsets.zero,
                value: _allowWrite,
                title: const Text('Permitir abrir PR do workflow'),
                subtitle: const Text(
                  'Sempre em branch própria do NetMonitor, nunca na branch padrão.',
                ),
                onChanged: (v) => setState(() => _allowWrite = v),
              ),
              Align(
                alignment: Alignment.centerRight,
                child: FilledButton.icon(
                  onPressed: _creatingManifest ? null : _startGithubManifest,
                  icon: const Icon(Icons.open_in_new, size: 16),
                  label: const Text('Conectar GitHub App'),
                ),
              ),
              if (_manifestReady != null) ...<Widget>[
                const SizedBox(height: 10),
                CodeViewer(
                  title: 'Manifest gerado (${_manifestReady!.actionUrl})',
                  code: _manifestReady!.manifest,
                  maxHeight: 160,
                ),
              ],
            ],
          ),
        ),
        const Divider(height: 28),
        Text(
          'Tokens clássicos (PAT / ghcr.io)',
          style: context.textTheme.titleSmall?.copyWith(
            fontWeight: FontWeight.w800,
          ),
        ),
        const SizedBox(height: 8),
        ...deploys.credentials.map(
          (cred) => ListTile(
            contentPadding: EdgeInsets.zero,
            leading: const Icon(Icons.vpn_key_outlined),
            title: Text(
              cred.name,
              style: const TextStyle(fontWeight: FontWeight.w700),
            ),
            subtitle: Text('${cred.username} · ${cred.registry ?? 'GitHub'}'),
            trailing: IconButton(
              icon: const Icon(Icons.delete_outline),
              onPressed: () =>
                  ref.read(deploysProvider.notifier).removeCredential(cred.id),
            ),
          ),
        ),
        const SizedBox(height: 8),
        ResponsiveFieldRow(
          breakpoint: 480,
          spacing: 8,
          runSpacing: 8,
          children: <Widget>[
            TextField(
              controller: _credNameCtrl,
              decoration: const InputDecoration(
                labelText: 'Nome',
                isDense: true,
              ),
            ),
            TextField(
              controller: _credRegistryCtrl,
              decoration: const InputDecoration(
                labelText: 'Registry',
                isDense: true,
              ),
            ),
          ],
        ),
        const SizedBox(height: 8),
        ResponsiveFieldRow(
          breakpoint: 480,
          spacing: 8,
          runSpacing: 8,
          children: <Widget>[
            TextField(
              controller: _credUserCtrl,
              decoration: const InputDecoration(
                labelText: 'Usuário',
                isDense: true,
              ),
            ),
            TextField(
              controller: _credSecretCtrl,
              obscureText: true,
              decoration: const InputDecoration(
                labelText: 'Token / PAT',
                isDense: true,
              ),
            ),
          ],
        ),
        if (_error != null) ...<Widget>[
          const SizedBox(height: 10),
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
        ],
        const SizedBox(height: 10),
        Align(
          alignment: Alignment.centerRight,
          child: OutlinedButton.icon(
            onPressed: _savingCred ? null : _saveCredential,
            icon: const Icon(Icons.add),
            label: const Text('Adicionar token'),
          ),
        ),
      ],
    );
  }
}
