import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/auth/area_access.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/config/server_config.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/registry_models.dart';
import 'package:netmonitor_app/features/servers/providers/registry_provider.dart';
import 'package:netmonitor_app/features/servers/utils/server_utils.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';
import 'package:url_launcher/url_launcher.dart';

/// Página do Container Registry (`/registry` — ADR 022 / ADR 023).
/// 4 abas: Imagens, Tokens de acesso, Vulnerabilidades e Configurações (admin).
class RegistryPage extends ConsumerStatefulWidget {
  const RegistryPage({super.key});

  @override
  ConsumerState<RegistryPage> createState() => _RegistryPageState();
}

class _RegistryPageState extends ConsumerState<RegistryPage> {
  int _tabIndex = 0;

  // Tokens
  List<RegistryTokenView> _tokens = const <RegistryTokenView>[];
  bool _loadingTokens = false;
  RegistryTokenCreated? _justCreatedToken;

  // Exceptions
  List<RegistryExceptionView> _exceptions = const <RegistryExceptionView>[];
  bool _loadingExceptions = false;

  // Settings
  final TextEditingController _keepVersionsCtrl = TextEditingController();
  final TextEditingController _quotaGibCtrl = TextEditingController();
  bool _settingsDirty = false;
  bool _savingSettings = false;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(_bootstrap);
  }

  @override
  void dispose() {
    _keepVersionsCtrl.dispose();
    _quotaGibCtrl.dispose();
    super.dispose();
  }

  Future<void> _bootstrap() async {
    await ref.read(registryProvider.notifier).fetchOverview();
    if (!mounted) return;
    _syncSettingsFields();
    await Future.wait<void>(<Future<void>>[_loadTokens(), _loadExceptions()]);
  }

  void _syncSettingsFields() {
    final settings = ref.read(registryProvider).overview?.settings;
    if (settings != null && !_settingsDirty) {
      _keepVersionsCtrl.text = settings.keepVersions.toString();
      _quotaGibCtrl.text = settings.quotaGib.toString();
    }
  }

  Future<void> _loadTokens() async {
    setState(() => _loadingTokens = true);
    try {
      final list = await ref.read(registryProvider.notifier).fetchTokens();
      if (!mounted) return;
      setState(() {
        _tokens = list;
        _loadingTokens = false;
      });
    } catch (_) {
      if (mounted) setState(() => _loadingTokens = false);
    }
  }

  Future<void> _loadExceptions() async {
    setState(() => _loadingExceptions = true);
    try {
      final list = await ref.read(registryProvider.notifier).fetchExceptions();
      if (!mounted) return;
      setState(() {
        _exceptions = list;
        _loadingExceptions = false;
      });
    } catch (_) {
      if (mounted) setState(() => _loadingExceptions = false);
    }
  }

  String _currentOrigin() =>
      kIsWeb ? Uri.base.origin : ref.read(serverConfigProvider).baseUrl;

  Future<void> _openDockerHelp() async {
    final origin = _currentOrigin();
    final host = registryHostFromUrl(origin);
    final isSecure = origin.startsWith('https://');
    final isInternal = looksPrivateUrl(origin);

    await showAdaptiveFormSheet<void>(
      context: context,
      title: 'Usar o Registry pelo Docker',
      subtitle: host,
      maxWidth: 680,
      builder: (_) => Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          if (!isInternal) ...<Widget>[
            const NestedPanel(
              tone: SemanticTone.warning,
              child: Text(
                'Atenção (ADR 023): tokens pessoais de acesso são recusados pela borda pública. '
                'Conecte-se pela VPN do NetMonitor ou pela rede interna para usar docker login.',
              ),
            ),
            const SizedBox(height: 12),
          ],
          if (!isSecure) ...<Widget>[
            CodeViewer(
              title: '/etc/docker/daemon.json (HTTP interno / VPN)',
              code: registryDaemonJson(host),
            ),
            const SizedBox(height: 12),
          ],
          CodeViewer(
            title: '1. Autenticar (cole o token como senha)',
            code: 'docker login $host -u token',
          ),
          const SizedBox(height: 12),
          CodeViewer(
            title: '2. Enviar ou baixar imagem',
            code:
                'docker tag minha-imagem $host/app/servico:v1\n'
                'docker push $host/app/servico:v1\n'
                'docker pull $host/app/servico:v1',
          ),
        ],
      ),
    );
  }

  Future<void> _openCreateTokenDialog() async {
    final nameCtrl = TextEditingController();
    final reposCtrl = TextEditingController(text: '*');
    final actions = <String>{'pull'};
    var days = 90;

    final created = await showAdaptiveFormSheet<RegistryTokenCreated>(
      context: context,
      title: 'Novo token de acesso ao Registry',
      subtitle:
          'Válido exclusivamente a partir da VPN ou rede interna (ADR 023)',
      builder: (ctx) => StatefulBuilder(
        builder: (innerCtx, setModalState) => Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            TextField(
              controller: nameCtrl,
              autofocus: true,
              decoration: const InputDecoration(
                labelText: 'Nome do token',
                hintText: 'ex.: ci-local',
              ),
            ),
            const SizedBox(height: 10),
            TextField(
              controller: reposCtrl,
              decoration: const InputDecoration(
                labelText: 'Repositórios (separados por vírgula)',
                hintText: '* ou app/* ou app/api',
              ),
            ),
            const SizedBox(height: 10),
            Wrap(
              spacing: 8,
              children: const <String>['pull', 'push', 'delete']
                  .map((act) {
                    return FilterChip(
                      selected: actions.contains(act),
                      label: Text(
                        '${act.toUpperCase()} (${registryActionLabel(act)})',
                      ),
                      onSelected: (checked) => setModalState(() {
                        if (checked) {
                          actions.add(act);
                        } else if (actions.length > 1) {
                          actions.remove(act);
                        }
                      }),
                    );
                  })
                  .toList(growable: false),
            ),
            const SizedBox(height: 10),
            AppDropdownFormField<int>(
              initialValue: days,
              decoration: const InputDecoration(labelText: 'Validade'),
              items: const <DropdownMenuItem<int>>[
                DropdownMenuItem(value: 30, child: Text('30 dias')),
                DropdownMenuItem(value: 90, child: Text('90 dias')),
                DropdownMenuItem(value: 180, child: Text('180 dias')),
                DropdownMenuItem(value: 365, child: Text('365 dias')),
              ],
              onChanged: (v) {
                if (v != null) setModalState(() => days = v);
              },
            ),
            const SizedBox(height: 16),
            Align(
              alignment: Alignment.centerRight,
              child: FilledButton(
                onPressed: () async {
                  if (nameCtrl.text.trim().isEmpty) return;
                  final repos = reposCtrl.text
                      .split(',')
                      .map((e) => e.trim())
                      .where((e) => e.isNotEmpty)
                      .toList(growable: false);
                  final res = await ref
                      .read(registryProvider.notifier)
                      .createToken(
                        name: nameCtrl.text.trim(),
                        repositories: repos.isEmpty
                            ? const <String>['*']
                            : repos,
                        actions: actions.toList(growable: false),
                        days: days,
                      );
                  if (innerCtx.mounted) Navigator.of(innerCtx).pop(res);
                },
                child: const Text('Criar token'),
              ),
            ),
          ],
        ),
      ),
    );

    nameCtrl.dispose();
    reposCtrl.dispose();
    if (created != null && mounted) {
      setState(() => _justCreatedToken = created);
      await _loadTokens();
    }
  }

  Future<void> _openCreateExceptionDialog() async {
    final advisoryCtrl = TextEditingController();
    final reasonCtrl = TextEditingController();
    var days = 30;
    final added = await showAdaptiveFormSheet<bool>(
      context: context,
      title: 'Nova exceção de vulnerabilidade',
      builder: (ctx) => StatefulBuilder(
        builder: (innerCtx, setModalState) => Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            TextField(
              controller: advisoryCtrl,
              decoration: const InputDecoration(
                labelText: 'Identificador OSV / CVE',
                hintText: 'ex.: GHSA-xxxx-xxxx-xxxx ou CVE-2026-1234',
              ),
            ),
            const SizedBox(height: 10),
            TextField(
              controller: reasonCtrl,
              maxLines: 3,
              decoration: const InputDecoration(
                labelText: 'Justificativa (mín. 10 caracteres)',
              ),
            ),
            const SizedBox(height: 10),
            AppDropdownFormField<int>(
              initialValue: days,
              decoration: const InputDecoration(labelText: 'Prazo'),
              items: const <DropdownMenuItem<int>>[
                DropdownMenuItem(value: 7, child: Text('7 dias')),
                DropdownMenuItem(value: 30, child: Text('30 dias')),
                DropdownMenuItem(value: 90, child: Text('90 dias')),
                DropdownMenuItem(value: 180, child: Text('180 dias')),
              ],
              onChanged: (v) {
                if (v != null) setModalState(() => days = v);
              },
            ),
            const SizedBox(height: 16),
            Align(
              alignment: Alignment.centerRight,
              child: FilledButton(
                onPressed: () async {
                  if (advisoryCtrl.text.trim().isEmpty ||
                      reasonCtrl.text.trim().length < 10) {
                    return;
                  }
                  await ref
                      .read(registryProvider.notifier)
                      .addException(
                        advisory: advisoryCtrl.text.trim(),
                        reason: reasonCtrl.text.trim(),
                        days: days,
                      );
                  if (innerCtx.mounted) Navigator.of(innerCtx).pop(true);
                },
                child: const Text('Registrar exceção'),
              ),
            ),
          ],
        ),
      ),
    );
    advisoryCtrl.dispose();
    reasonCtrl.dispose();
    if (added == true && mounted) await _loadExceptions();
  }

  Future<void> _saveSettings() async {
    setState(() => _savingSettings = true);
    try {
      await ref
          .read(registryProvider.notifier)
          .saveSettings(
            RegistrySettings(
              keepVersions: (int.tryParse(_keepVersionsCtrl.text.trim()) ?? 10)
                  .clamp(1, 200),
              quotaGib: (int.tryParse(_quotaGibCtrl.text.trim()) ?? 20).clamp(
                1,
                10000,
              ),
            ),
          );
      if (!mounted) return;
      setState(() {
        _settingsDirty = false;
        _savingSettings = false;
      });
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Configurações do Registry salvas.')),
      );
    } catch (err) {
      if (!mounted) return;
      setState(() => _savingSettings = false);
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text('Erro: $err')));
    }
  }

  Future<void> _collectGarbage() async {
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: 'Executar limpeza (garbage collection)?',
      message:
          'Remove versões antigas fora da retenção e camadas órfãs não usadas por deploys.',
      confirmLabel: 'Limpar agora',
      tone: SemanticTone.warning,
    );
    if (!confirmed || !mounted) return;
    final freed = await ref.read(registryProvider.notifier).collectGarbage();
    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text('Limpeza concluída: ${formatBytes(freed)} liberados.'),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final reg = ref.watch(registryProvider);
    final canAdmin =
        ref.watch(authProvider).roleIn(ProductArea.servers) == UserRole.admin;
    final overview = reg.overview;
    final isCompact = MediaQuery.sizeOf(context).width < 680;
    if (overview != null && !_settingsDirty && _keepVersionsCtrl.text.isEmpty) {
      _syncSettingsFields();
    }

    return RefreshIndicator(
      onRefresh: _bootstrap,
      child: ListView(
        padding: const EdgeInsets.all(16),
        children: <Widget>[
          if (isCompact)
            Row(
              children: <Widget>[
                Expanded(
                  child: Text(
                    'Registry de imagens',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: context.textTheme.titleLarge?.copyWith(
                      fontWeight: FontWeight.w800,
                    ),
                  ),
                ),
                IconButton(
                  tooltip: 'Atualizar',
                  onPressed: reg.loading ? null : _bootstrap,
                  icon: const Icon(Icons.refresh, size: 20),
                ),
                PopupMenuButton<String>(
                  tooltip: 'Mais ações',
                  onSelected: (cmd) {
                    if (cmd == 'docker_help') _openDockerHelp();
                  },
                  itemBuilder: (_) => const <PopupMenuEntry<String>>[
                    PopupMenuItem<String>(
                      value: 'docker_help',
                      child: ListTile(
                        dense: true,
                        leading: Icon(Icons.terminal),
                        title: Text('Usar pelo Docker'),
                      ),
                    ),
                  ],
                ),
              ],
            )
          else
            SearchFilterHeader(
              title: 'Registry de imagens',
              subtitle:
                  'Imagens compiladas com tags imutáveis, SBOM e análise de vulnerabilidades OSV.',
              actions: <Widget>[
                OutlinedButton.icon(
                  onPressed: _openDockerHelp,
                  icon: const Icon(Icons.terminal, size: 18),
                  label: const Text('Usar pelo Docker'),
                ),
                OutlinedButton.icon(
                  onPressed: reg.loading ? null : _bootstrap,
                  icon: const Icon(Icons.refresh, size: 18),
                  label: const Text('Atualizar'),
                ),
              ],
            ),
          const SizedBox(height: 12),

          SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: SegmentedButton<int>(
              showSelectedIcon: false,
              style: SegmentedButton.styleFrom(
                visualDensity: VisualDensity.compact,
                padding: const EdgeInsets.symmetric(horizontal: 10),
              ),
              segments: <ButtonSegment<int>>[
                const ButtonSegment(
                  value: 0,
                  label: Text('Imagens', maxLines: 1, softWrap: false),
                  icon: Icon(Icons.inventory_2_outlined),
                ),
                const ButtonSegment(
                  value: 1,
                  label: Text('Tokens', maxLines: 1, softWrap: false),
                  icon: Icon(Icons.vpn_key_outlined),
                ),
                const ButtonSegment(
                  value: 2,
                  label: Text('Vulnerabilidades', maxLines: 1, softWrap: false),
                  icon: Icon(Icons.shield_outlined),
                ),
                if (canAdmin)
                  const ButtonSegment(
                    value: 3,
                    label: Text('Configurações', maxLines: 1, softWrap: false),
                    icon: Icon(Icons.tune),
                  ),
              ],
              selected: <int>{_tabIndex > 2 && !canAdmin ? 0 : _tabIndex},
              onSelectionChanged: (set) =>
                  setState(() => _tabIndex = set.first),
            ),
          ),
          const SizedBox(height: 16),

          if (reg.error != null) ...<Widget>[
            NestedPanel(tone: SemanticTone.error, child: Text(reg.error!)),
            const SizedBox(height: 12),
          ],

          if (reg.loading && overview == null)
            const LoadingStateView(message: 'Carregando registry...')
          else if (_tabIndex == 0)
            _buildRepositoriesTab(overview, canAdmin)
          else if (_tabIndex == 1)
            _buildTokensTab()
          else if (_tabIndex == 2)
            _buildVulnerabilitiesTab(overview, canAdmin)
          else if (canAdmin)
            _buildSettingsTab(overview),
        ],
      ),
    );
  }

  Widget _buildRepositoriesTab(RegistryOverview? overview, bool canAdmin) {
    if (overview == null) {
      return const EmptyStateView(title: 'Registry indisponível');
    }
    final pct = overview.usedPercent.clamp(0.0, 100.0);
    final barColor = pct >= 95
        ? context.semanticColors.error
        : (pct >= 80
              ? context.semanticColors.warning
              : context.colorScheme.primary);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        SectionCard(
          title: 'Espaço do Registry',
          subtitle:
              '${formatBytes(overview.usedBytes)} de ${formatBytes(overview.quotaBytes)} (${formatPercent(pct)}) · '
              'Retenção: ${countNoun(overview.settings.keepVersions, 'versão', 'versões')}',
          icon: Icons.storage_outlined,
          iconTone: pct >= 95
              ? SemanticTone.error
              : (pct >= 80 ? SemanticTone.warning : SemanticTone.primary),
          child: ClipRRect(
            borderRadius: BorderRadius.circular(6),
            child: LinearProgressIndicator(
              value: pct / 100,
              minHeight: 10,
              color: barColor,
            ),
          ),
        ),
        const SizedBox(height: 14),
        if (overview.repositories.isEmpty)
          const EmptyStateView(
            title: 'Nenhuma imagem armazenada ainda',
            description:
                'Compile uma aplicação pelo Deploy Contínuo ou envie via docker push.',
            icon: Icons.inventory_2_outlined,
          )
        else
          ResponsiveGrid(
            xs: 1,
            sm: 2,
            md: 2,
            lg: 3,
            xl: 3,
            children: overview.repositories
                .map((repo) {
                  final scanTone = registryScanTone(repo.latestScan);
                  return ChoiceCard(
                    title: repo.name,
                    subtitle:
                        '${countNoun(repo.versions, 'versão', 'versões')} · '
                        '${formatDecimalBytes(repo.bytes)}'
                        '${repo.applicationName != null ? ' · App ${repo.applicationName}' : ''}\n'
                        '${registryScanSentence(repo.latestScan)}',
                    icon: Icons.inventory_2_outlined,
                    tone: scanTone,
                    badge: repo.lastPush != null
                        ? formatRelativeTime(repo.lastPush)
                        : null,
                    badgeTone: scanTone,
                    onTap: () => showAdaptiveFormSheet<void>(
                      context: context,
                      title: repo.name,
                      subtitle: 'Versões, tags imutáveis e varredura SBOM/OSV',
                      maxWidth: 820,
                      builder: (_) => _RepositoryDetailBody(
                        repositoryId: repo.id,
                        canAdmin: canAdmin,
                      ),
                    ),
                  );
                })
                .toList(growable: false),
          ),
      ],
    );
  }

  Widget _buildTokensTab() {
    final host = registryHostFromUrl(_currentOrigin());
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const NestedPanel(
          tone: SemanticTone.info,
          child: Text(
            'Segurança (ADR 023): tokens pessoais de acesso ao Registry só são aceitos de origem interna ou VPN — '
            'pela borda pública eles são recusados automaticamente.',
          ),
        ),
        const SizedBox(height: 12),
        if (_justCreatedToken != null) ...<Widget>[
          NestedPanel(
            tone: SemanticTone.success,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Text(
                  'Token "${_justCreatedToken!.token.name}" criado — copie agora, ele não será exibido novamente:',
                  style: const TextStyle(fontWeight: FontWeight.w700),
                ),
                const SizedBox(height: 8),
                CodeViewer(
                  title: 'Segredo do token',
                  code: _justCreatedToken!.secret,
                ),
                const SizedBox(height: 8),
                CodeViewer(
                  title: 'Login no Docker',
                  code: 'docker login $host -u token',
                ),
              ],
            ),
          ),
          const SizedBox(height: 12),
        ],
        Align(
          alignment: Alignment.centerRight,
          child: FilledButton.icon(
            onPressed: _openCreateTokenDialog,
            icon: const Icon(Icons.add),
            label: const Text('Novo token pessoal'),
          ),
        ),
        const SizedBox(height: 12),
        if (_loadingTokens)
          const LoadingStateView(message: 'Carregando tokens...')
        else if (_tokens.isEmpty)
          const EmptyStateView(
            title: 'Nenhum token pessoal criado',
            icon: Icons.vpn_key_outlined,
          )
        else
          ..._tokens.map(
            (t) => Padding(
              padding: const EdgeInsets.only(bottom: 10),
              child: NestedPanel(
                tone: t.revoked || t.expired ? SemanticTone.secondary : null,
                child: Row(
                  children: <Widget>[
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          Wrap(
                            spacing: 6,
                            runSpacing: 4,
                            children: <Widget>[
                              Text(
                                t.name,
                                style: const TextStyle(
                                  fontWeight: FontWeight.w700,
                                ),
                              ),
                              SemanticChip(
                                label: t.hint,
                                tone: SemanticTone.secondary,
                                compact: true,
                              ),
                              if (t.revoked)
                                const SemanticChip(
                                  label: 'Revogado',
                                  tone: SemanticTone.error,
                                  compact: true,
                                ),
                              if (t.expired)
                                const SemanticChip(
                                  label: 'Vencido',
                                  tone: SemanticTone.warning,
                                  compact: true,
                                ),
                            ],
                          ),
                          const SizedBox(height: 4),
                          Text(
                            'Escopo: ${t.repositories.join(', ')} · '
                            'Ações: ${t.actions.map(registryActionLabel).join(', ')} · '
                            'Último uso: ${formatRelativeTime(t.lastUsedAt)}',
                            style: context.textTheme.bodySmall,
                          ),
                        ],
                      ),
                    ),
                    if (!t.revoked)
                      IconButton(
                        tooltip: 'Revogar token',
                        icon: const Icon(Icons.delete_outline),
                        onPressed: () async {
                          final ok = await showConfirmActionDialog(
                            context: context,
                            title: 'Revogar token ${t.name}?',
                            message:
                                'O token deixará de autenticar imediatamente.',
                            confirmLabel: 'Revogar',
                            tone: SemanticTone.error,
                          );
                          if (!ok) return;
                          await ref
                              .read(registryProvider.notifier)
                              .revokeToken(t.id);
                          await _loadTokens();
                        },
                      ),
                  ],
                ),
              ),
            ),
          ),
      ],
    );
  }

  Widget _buildVulnerabilitiesTab(RegistryOverview? overview, bool canAdmin) {
    final feeds = overview?.feeds ?? const <RegistryFeedView>[];
    final isCompact = MediaQuery.sizeOf(context).width < 560;
    final syncBtn = canAdmin
        ? FilledButton.tonalIcon(
            onPressed: () async {
              try {
                await ref.read(registryProvider.notifier).syncAdvisories();
                if (!mounted) return;
                ScaffoldMessenger.of(context).showSnackBar(
                  const SnackBar(content: Text('Sincronização OSV iniciada.')),
                );
              } catch (err) {
                if (!mounted) return;
                ScaffoldMessenger.of(
                  context,
                ).showSnackBar(SnackBar(content: Text('Aviso: $err')));
              }
            },
            icon: const Icon(Icons.sync, size: 16),
            label: const Text('Atualizar agora'),
          )
        : null;
    final newExceptionBtn = canAdmin
        ? FilledButton.tonalIcon(
            onPressed: _openCreateExceptionDialog,
            icon: const Icon(Icons.add, size: 16),
            label: const Text('Nova exceção'),
          )
        : null;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        SectionCard(
          title: 'Bases locais de vulnerabilidades (OSV.dev)',
          subtitle:
              'Varredura offline por SBOM — nunca executa o conteúdo da imagem.',
          icon: Icons.shield_outlined,
          iconTone: SemanticTone.info,
          trailing: isCompact ? null : syncBtn,
          footer: isCompact && syncBtn != null
              ? Align(alignment: Alignment.centerRight, child: syncBtn)
              : null,
          child: feeds.isEmpty
              ? const EmptyStateView(title: 'Nenhuma base sincronizada ainda')
              : Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  children: feeds
                      .map((f) {
                        return SemanticChip(
                          label:
                              '${f.ecosystem}: ${formatCompactCount(f.entries)} (${formatRelativeTime(f.syncedAt)})',
                          tone: f.error != null
                              ? SemanticTone.error
                              : SemanticTone.success,
                        );
                      })
                      .toList(growable: false),
                ),
        ),
        const SizedBox(height: 14),
        SectionCard(
          title: 'Exceções de risco aceitas',
          icon: Icons.rule_outlined,
          iconTone: SemanticTone.warning,
          trailing: isCompact ? null : newExceptionBtn,
          footer: isCompact && newExceptionBtn != null
              ? Align(alignment: Alignment.centerRight, child: newExceptionBtn)
              : null,
          child: _loadingExceptions
              ? const LoadingStateView(message: 'Carregando exceções...')
              : _exceptions.isEmpty
              ? const EmptyStateView(title: 'Nenhuma exceção registrada')
              : Column(
                  children: _exceptions
                      .map((ex) {
                        return Padding(
                          padding: const EdgeInsets.only(bottom: 8),
                          child: NestedPanel(
                            child: Row(
                              children: <Widget>[
                                Expanded(
                                  child: Column(
                                    crossAxisAlignment:
                                        CrossAxisAlignment.start,
                                    children: <Widget>[
                                      Wrap(
                                        spacing: 6,
                                        children: <Widget>[
                                          Text(
                                            ex.advisory,
                                            style: const TextStyle(
                                              fontFamily: 'monospace',
                                              fontWeight: FontWeight.w700,
                                            ),
                                          ),
                                          SemanticChip(
                                            label:
                                                ex.application ??
                                                'Todas as aplicações',
                                            tone: SemanticTone.secondary,
                                            compact: true,
                                          ),
                                          if (ex.expired)
                                            const SemanticChip(
                                              label: 'Expirada',
                                              tone: SemanticTone.warning,
                                              compact: true,
                                            ),
                                        ],
                                      ),
                                      const SizedBox(height: 4),
                                      Text(
                                        ex.reason,
                                        style: context.textTheme.bodySmall,
                                      ),
                                      Text(
                                        'Validade: ${formatDate(ex.expiresAt)}',
                                        style: context.textTheme.bodySmall
                                            ?.copyWith(
                                              color: context
                                                  .colorScheme
                                                  .onSurfaceVariant,
                                            ),
                                      ),
                                    ],
                                  ),
                                ),
                                if (canAdmin)
                                  IconButton(
                                    icon: const Icon(Icons.delete_outline),
                                    onPressed: () async {
                                      await ref
                                          .read(registryProvider.notifier)
                                          .removeException(ex.id);
                                      await _loadExceptions();
                                    },
                                  ),
                              ],
                            ),
                          ),
                        );
                      })
                      .toList(growable: false),
                ),
        ),
      ],
    );
  }

  Widget _buildSettingsTab(RegistryOverview? overview) {
    return SectionCard(
      title: 'Retenção e cota do Registry',
      subtitle: 'Aviso em 80% da cota; novos pushes são recusados em 95%.',
      icon: Icons.tune,
      iconTone: SemanticTone.primary,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          ResponsiveFieldRow(
            breakpoint: 480,
            spacing: 12,
            runSpacing: 12,
            children: <Widget>[
              TextField(
                controller: _keepVersionsCtrl,
                keyboardType: TextInputType.number,
                decoration: const InputDecoration(
                  labelText: 'Versões mantidas (1–200)',
                ),
                onChanged: (_) => setState(() => _settingsDirty = true),
              ),
              TextField(
                controller: _quotaGibCtrl,
                keyboardType: TextInputType.number,
                decoration: const InputDecoration(labelText: 'Cota (GiB)'),
                onChanged: (_) => setState(() => _settingsDirty = true),
              ),
            ],
          ),
          const SizedBox(height: 16),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            alignment: WrapAlignment.end,
            children: <Widget>[
              OutlinedButton.icon(
                onPressed: _settingsDirty ? null : _collectGarbage,
                icon: const Icon(Icons.cleaning_services_outlined, size: 16),
                label: const Text('Limpar agora (GC)'),
              ),
              FilledButton.icon(
                onPressed: _savingSettings ? null : _saveSettings,
                icon: const Icon(Icons.save),
                label: const Text('Salvar configurações'),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

class _RepositoryDetailBody extends ConsumerStatefulWidget {
  const _RepositoryDetailBody({
    required this.repositoryId,
    required this.canAdmin,
  });
  final int repositoryId;
  final bool canAdmin;

  @override
  ConsumerState<_RepositoryDetailBody> createState() =>
      _RepositoryDetailBodyState();
}

class _RepositoryDetailBodyState extends ConsumerState<_RepositoryDetailBody> {
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
        _error = err.toString();
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
                            builder: (_) => _FindingsDialogBody(
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

class _FindingsDialogBody extends ConsumerStatefulWidget {
  const _FindingsDialogBody({
    required this.version,
    required this.applicationId,
    required this.canAdmin,
  });

  final RegistryVersionView version;
  final int? applicationId;
  final bool canAdmin;

  @override
  ConsumerState<_FindingsDialogBody> createState() =>
      _FindingsDialogBodyState();
}

class _FindingsDialogBodyState extends ConsumerState<_FindingsDialogBody> {
  List<RegistryFindingView> _findings = const <RegistryFindingView>[];
  bool _loading = true;
  bool _scanning = false;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(_load);
  }

  Future<void> _load() async {
    setState(() => _loading = true);
    try {
      final list = await ref
          .read(registryProvider.notifier)
          .versionFindings(widget.version.id);
      if (!mounted) return;
      setState(() {
        _findings = list;
        _loading = false;
      });
    } catch (_) {
      if (mounted) setState(() => _loading = false);
    }
  }

  Future<void> _rescan() async {
    setState(() => _scanning = true);
    try {
      final list = await ref
          .read(registryProvider.notifier)
          .rescanVersion(widget.version.id);
      if (!mounted) return;
      setState(() {
        _findings = list;
        _scanning = false;
      });
    } catch (_) {
      if (mounted) setState(() => _scanning = false);
    }
  }

  Future<void> _acceptRisk(RegistryFindingView f) async {
    final reasonCtrl = TextEditingController();
    final reason = await showDialog<String>(
      context: context,
      builder: (dlgCtx) => AlertDialog(
        title: Text('Aceitar risco · ${f.advisory}'),
        content: TextField(
          controller: reasonCtrl,
          maxLines: 3,
          decoration: const InputDecoration(
            labelText: 'Justificativa (mín. 5 caracteres)',
          ),
        ),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.of(dlgCtx).pop(),
            child: const Text('Cancelar'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(dlgCtx).pop(reasonCtrl.text.trim()),
            child: const Text('Aceitar por 30 dias'),
          ),
        ],
      ),
    );
    reasonCtrl.dispose();
    if (reason == null || reason.length < 5 || !mounted) return;
    await ref
        .read(registryProvider.notifier)
        .addException(
          applicationId: widget.applicationId,
          advisory: f.advisory,
          reason: reason,
          days: 30,
        );
    await _load();
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
