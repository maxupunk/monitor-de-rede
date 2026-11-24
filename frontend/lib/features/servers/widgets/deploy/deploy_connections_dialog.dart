import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/network_address_utils.dart';
import 'package:netmonitor_app/core/config/server_config.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/features/servers/models/deploy_models.dart';
import 'package:netmonitor_app/features/servers/providers/deploys_provider.dart';
import 'package:netmonitor_app/features/servers/utils/github_manifest_submit.dart';
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

Future<void> _openExternal(String url) async {
  final uri = Uri.tryParse(url);
  if (uri == null) return;
  await launchUrl(uri, mode: LaunchMode.externalApplication);
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

  /// Com uma conexão feita, o formulário de outra fica recolhido.
  bool _showConnectForm = false;
  final Set<int> _busyConnections = <int>{};

  final TextEditingController _credNameCtrl = TextEditingController();
  final TextEditingController _credRegistryCtrl = TextEditingController(
    text: 'ghcr.io',
  );
  final TextEditingController _credUserCtrl = TextEditingController();
  final TextEditingController _credSecretCtrl = TextEditingController();
  bool _savingCred = false;
  String? _error;
  String? _notice;

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
      _notice = null;
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
      // Na Web o formulário troca esta página pela do GitHub, que volta a
      // `/deploys/github` com o código; fora dela, abre no navegador.
      final submitted = submitGithubManifestForm(
        actionUrl: manifest.actionUrl,
        manifestJson: manifest.manifest,
      );
      if (!submitted) {
        await _openExternal(manifest.actionUrl);
        if (!mounted) return;
        setState(() {
          _creatingManifest = false;
          _notice =
              'Conclua a criação do app no navegador. A conexão aparece aqui '
              'quando o GitHub devolver você para a central.';
        });
      }
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = describeError(err, fallback: 'Erro ao gerar manifest GitHub.');
        _creatingManifest = false;
      });
    }
  }

  Future<void> _refreshConnection(DeployGithubConnectionView conn) async {
    setState(() {
      _busyConnections.add(conn.id);
      _error = null;
      _notice = null;
    });
    try {
      final updated = await ref
          .read(deploysProvider.notifier)
          .refreshGithub(conn.id);
      if (!mounted) return;
      setState(
        () => _notice = updated.installed
            ? 'Conexão com ${updated.account} conferida: instalada e pronta.'
            : 'O app de ${updated.account} ainda não foi instalado em nenhum '
                  'repositório.',
      );
    } catch (err) {
      if (!mounted) return;
      setState(
        () => _error = describeError(
          err,
          fallback: 'Erro ao conferir a conexão com o GitHub.',
        ),
      );
    } finally {
      if (mounted) setState(() => _busyConnections.remove(conn.id));
    }
  }

  Future<void> _removeConnection(DeployGithubConnectionView conn) async {
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: 'Remover a conexão com ${conn.account}?',
      message:
          'As aplicações ligadas a ela deixam de ler o repositório e de abrir '
          'o PR do workflow. O app continua existindo no GitHub: para apagá-lo, '
          'use as configurações dele por lá.',
      confirmLabel: 'Remover',
    );
    if (!confirmed || !mounted) return;
    setState(() {
      _busyConnections.add(conn.id);
      _error = null;
      _notice = null;
    });
    try {
      await ref.read(deploysProvider.notifier).removeGithub(conn.id);
    } catch (err) {
      if (!mounted) return;
      setState(
        () => _error = describeError(
          err,
          fallback: 'Erro ao remover a conexão com o GitHub.',
        ),
      );
    } finally {
      if (mounted) setState(() => _busyConnections.remove(conn.id));
    }
  }

  Future<void> _removeCredential(DeployCredentialView cred) async {
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: 'Remover o token ${cred.name}?',
      message:
          'Aplicações e registries que usam este token deixam de autenticar '
          'até receberem outro.',
      confirmLabel: 'Remover',
    );
    if (!confirmed || !mounted) return;
    try {
      await ref.read(deploysProvider.notifier).removeCredential(cred.id);
    } catch (err) {
      if (!mounted) return;
      setState(
        () => _error = describeError(err, fallback: 'Erro ao remover token.'),
      );
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

  Widget _sectionTitle(String text) {
    return Text(
      text,
      style: context.textTheme.titleSmall?.copyWith(
        fontWeight: FontWeight.w800,
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final deploys = ref.watch(deploysProvider);
    final connections = deploys.githubConnections;
    final formVisible = connections.isEmpty || _showConnectForm;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        _sectionTitle('GitHub App da central'),
        const SizedBox(height: 6),
        Text(
          'A chave privada do GitHub App nunca sai da central; o agente recebe '
          'apenas tokens de 1h somente leitura.',
          style: context.textTheme.bodySmall?.copyWith(
            color: context.colorScheme.onSurfaceVariant,
          ),
        ),
        const SizedBox(height: 10),
        for (final conn in connections)
          Padding(
            padding: const EdgeInsets.only(bottom: 8),
            child: _GithubConnectionTile(
              connection: conn,
              busy: _busyConnections.contains(conn.id),
              onRefresh: () => _refreshConnection(conn),
              onRemove: () => _removeConnection(conn),
            ),
          ),
        if (_notice != null) ...<Widget>[
          NestedPanel(tone: SemanticTone.info, child: Text(_notice!)),
          const SizedBox(height: 8),
        ],
        if (formVisible)
          _ConnectGithubForm(
            orgCtrl: _orgCtrl,
            allowWrite: _allowWrite,
            onAllowWriteChanged: (v) => setState(() => _allowWrite = v),
            submitting: _creatingManifest,
            anotherAccount: connections.isNotEmpty,
            onSubmit: _startGithubManifest,
          )
        else
          Align(
            alignment: Alignment.centerLeft,
            child: TextButton.icon(
              onPressed: () => setState(() => _showConnectForm = true),
              icon: const Icon(Icons.add_link, size: 18),
              label: const Text('Conectar outra conta ou organização'),
            ),
          ),
        const Divider(height: 28),
        _sectionTitle('Tokens clássicos (PAT / ghcr.io)'),
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
              tooltip: 'Remover token',
              icon: const Icon(Icons.delete_outline),
              onPressed: () => _removeCredential(cred),
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

/// Uma conexão: em que pé ela está e o próximo passo, em destaque.
class _GithubConnectionTile extends StatelessWidget {
  const _GithubConnectionTile({
    required this.connection,
    required this.busy,
    required this.onRefresh,
    required this.onRemove,
  });

  final DeployGithubConnectionView connection;
  final bool busy;
  final VoidCallback onRefresh;
  final VoidCallback onRemove;

  @override
  Widget build(BuildContext context) {
    final conn = connection;
    final hasInstallUrl = conn.installUrl.isNotEmpty;
    return NestedPanel(
      tone: conn.installed ? SemanticTone.success : SemanticTone.warning,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Row(
            children: <Widget>[
              Icon(
                conn.organization ? Icons.corporate_fare : Icons.person_outline,
                size: 20,
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  conn.account,
                  style: context.textTheme.titleSmall?.copyWith(
                    fontWeight: FontWeight.w800,
                  ),
                ),
              ),
              if (busy)
                const SizedBox(
                  width: 18,
                  height: 18,
                  child: CircularProgressIndicator(strokeWidth: 2),
                ),
            ],
          ),
          const SizedBox(height: 8),
          Wrap(
            spacing: 6,
            runSpacing: 6,
            children: <Widget>[
              SemanticChip(
                compact: true,
                tone: conn.installed
                    ? SemanticTone.success
                    : SemanticTone.warning,
                icon: conn.installed
                    ? Icons.check_circle_outline
                    : Icons.pending_outlined,
                label: conn.installed ? 'Conectado' : 'Falta instalar',
              ),
              SemanticChip(
                compact: true,
                tone: conn.canWrite ? SemanticTone.primary : SemanticTone.info,
                icon: conn.canWrite ? Icons.edit_note : Icons.visibility,
                label: conn.canWrite ? 'Lê e abre PR' : 'Só leitura',
              ),
              SemanticChip(
                compact: true,
                tone: SemanticTone.info,
                icon: Icons.apps,
                label: 'App ${conn.appSlug}',
              ),
            ],
          ),
          const SizedBox(height: 8),
          Text(
            conn.installed
                ? 'Pronto: a central já enxerga os repositórios escolhidos na '
                      'instalação.'
                : 'O app foi criado. Falta instalá-lo e escolher os '
                      'repositórios que a central pode ler.',
            style: context.textTheme.bodySmall,
          ),
          const SizedBox(height: 10),
          Wrap(
            alignment: WrapAlignment.end,
            spacing: 8,
            runSpacing: 6,
            children: <Widget>[
              if (hasInstallUrl && !conn.installed)
                FilledButton.icon(
                  onPressed: busy ? null : () => _openExternal(conn.installUrl),
                  icon: const Icon(Icons.download_done, size: 16),
                  label: const Text('Instalar nos repositórios'),
                ),
              if (hasInstallUrl && conn.installed)
                OutlinedButton.icon(
                  onPressed: busy ? null : () => _openExternal(conn.installUrl),
                  icon: const Icon(Icons.folder_copy_outlined, size: 16),
                  label: const Text('Escolher repositórios'),
                ),
              OutlinedButton.icon(
                onPressed: busy ? null : onRefresh,
                icon: const Icon(Icons.refresh, size: 16),
                label: const Text('Conferir'),
              ),
              if (conn.settingsUrl.isNotEmpty)
                IconButton(
                  tooltip: 'Configurações do app no GitHub',
                  onPressed: () => _openExternal(conn.settingsUrl),
                  icon: const Icon(Icons.open_in_new, size: 18),
                ),
              IconButton(
                tooltip: 'Remover conexão',
                color: context.colorScheme.error,
                onPressed: busy ? null : onRemove,
                icon: const Icon(Icons.delete_outline, size: 18),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

/// O "Conectar GitHub App": os três passos do fluxo e o que se escolhe antes.
class _ConnectGithubForm extends StatelessWidget {
  const _ConnectGithubForm({
    required this.orgCtrl,
    required this.allowWrite,
    required this.onAllowWriteChanged,
    required this.submitting,
    required this.anotherAccount,
    required this.onSubmit,
  });

  final TextEditingController orgCtrl;
  final bool allowWrite;
  final ValueChanged<bool> onAllowWriteChanged;
  final bool submitting;
  final bool anotherAccount;
  final VoidCallback onSubmit;

  static const List<String> _steps = <String>[
    'No GitHub, confirme o nome do app e clique em "Create GitHub App".',
    'De volta à central, você é levado à instalação: escolha os '
        'repositórios.',
    'Pronto: a conexão aparece aqui como "Conectado".',
  ];

  @override
  Widget build(BuildContext context) {
    return NestedPanel(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          if (anotherAccount) ...<Widget>[
            Text(
              'Conectar outra conta ou organização',
              style: context.textTheme.titleSmall?.copyWith(
                fontWeight: FontWeight.w700,
              ),
            ),
            const SizedBox(height: 8),
          ],
          for (final (index, step) in _steps.indexed)
            Padding(
              padding: const EdgeInsets.only(bottom: 4),
              child: Text(
                '${index + 1}. $step',
                style: context.textTheme.bodySmall,
              ),
            ),
          const SizedBox(height: 8),
          TextField(
            controller: orgCtrl,
            decoration: const InputDecoration(
              labelText:
                  'Organização GitHub (opcional — deixe em branco para conta pessoal)',
            ),
          ),
          const SizedBox(height: 6),
          // O painel tem cor de fundo: o tile precisa do próprio Material
          // para o toque aparecer.
          Material(
            type: MaterialType.transparency,
            child: SwitchListTile(
              contentPadding: EdgeInsets.zero,
              value: allowWrite,
              title: const Text('Permitir abrir PR do workflow'),
              subtitle: const Text(
                'Sempre em branch própria do NetMonitor, nunca na branch padrão.',
              ),
              onChanged: onAllowWriteChanged,
            ),
          ),
          Align(
            alignment: Alignment.centerRight,
            child: FilledButton.icon(
              onPressed: submitting ? null : onSubmit,
              icon: submitting
                  ? const SizedBox(
                      width: 16,
                      height: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(Icons.open_in_new, size: 16),
              label: Text(
                submitting ? 'Abrindo o GitHub…' : 'Conectar GitHub App',
              ),
            ),
          ),
        ],
      ),
    );
  }
}
