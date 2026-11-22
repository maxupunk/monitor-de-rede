import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/auth/area_access.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/config/server_config.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/registry_models.dart';
import 'package:netmonitor_app/features/servers/providers/registry_provider.dart';
import 'package:netmonitor_app/features/servers/utils/server_utils.dart';
import 'package:netmonitor_app/features/servers/widgets/registry/registry_modals_and_tabs.dart';
import 'package:netmonitor_app/features/servers/widgets/registry/registry_repository_sheets.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Página do Container Registry (`/registry` — ADR 022 / ADR 023).
/// 4 abas: Imagens, Tokens de acesso, Vulnerabilidades e Configurações (admin).
class RegistryPage extends ConsumerStatefulWidget {
  const RegistryPage({super.key});

  @override
  ConsumerState<RegistryPage> createState() => _RegistryPageState();
}

class _RegistryPageState extends ConsumerState<RegistryPage> {
  int _tabIndex = 0;

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
    final notifier = ref.read(registryProvider.notifier);
    await notifier.fetchOverview();
    if (!mounted) return;
    _syncSettingsFields();
    await Future.wait<void>(<Future<void>>[
      notifier.fetchTokens(),
      notifier.fetchExceptions(),
    ]);
  }

  void _syncSettingsFields() {
    final settings = ref.read(registryProvider).overview?.settings;
    if (settings != null && !_settingsDirty) {
      _keepVersionsCtrl.text = settings.keepVersions.toString();
      _quotaGibCtrl.text = settings.quotaGib.toString();
    }
  }

  String _currentOrigin() =>
      resolveCentralOrigin(ref.read(serverConfigProvider).baseUrl);

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
      ).showSnackBar(SnackBar(content: Text('Erro: ${describeError(err)}')));
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
        padding: isCompact
            ? const EdgeInsets.only(bottom: 16)
            : const EdgeInsets.all(16),
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
                    if (cmd == 'docker_help') {
                      showRegistryDockerHelpSheet(
                        context,
                        origin: _currentOrigin(),
                      );
                    }
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
                  onPressed: () => showRegistryDockerHelpSheet(
                    context,
                    origin: _currentOrigin(),
                  ),
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

          if (isCompact)
            SizedBox(
              width: double.infinity,
              child: SegmentedButton<int>(
                showSelectedIcon: false,
                style: SegmentedButton.styleFrom(
                  visualDensity: VisualDensity.compact,
                  padding: const EdgeInsets.symmetric(horizontal: 6),
                ),
                segments: <ButtonSegment<int>>[
                  const ButtonSegment<int>(
                    value: 0,
                    label: Text('Imagens', maxLines: 1, softWrap: false),
                  ),
                  const ButtonSegment<int>(
                    value: 1,
                    label: Text('Tokens', maxLines: 1, softWrap: false),
                  ),
                  const ButtonSegment<int>(
                    value: 2,
                    label: Text('Segurança', maxLines: 1, softWrap: false),
                  ),
                  if (canAdmin)
                    const ButtonSegment<int>(
                      value: 3,
                      label: Text('Ajustes', maxLines: 1, softWrap: false),
                    ),
                ],
                selected: <int>{_tabIndex > 2 && !canAdmin ? 0 : _tabIndex},
                onSelectionChanged: (set) =>
                    setState(() => _tabIndex = set.first),
              ),
            )
          else
            SingleChildScrollView(
              scrollDirection: Axis.horizontal,
              child: SegmentedButton<int>(
                showSelectedIcon: false,
                style: SegmentedButton.styleFrom(
                  visualDensity: VisualDensity.compact,
                  padding: const EdgeInsets.symmetric(horizontal: 10),
                ),
                segments: <ButtonSegment<int>>[
                  const ButtonSegment<int>(
                    value: 0,
                    label: Text('Imagens', maxLines: 1, softWrap: false),
                    icon: Icon(Icons.inventory_2_outlined),
                  ),
                  const ButtonSegment<int>(
                    value: 1,
                    label: Text('Tokens', maxLines: 1, softWrap: false),
                    icon: Icon(Icons.vpn_key_outlined),
                  ),
                  const ButtonSegment<int>(
                    value: 2,
                    label: Text(
                      'Vulnerabilidades',
                      maxLines: 1,
                      softWrap: false,
                    ),
                    icon: Icon(Icons.shield_outlined),
                  ),
                  if (canAdmin)
                    const ButtonSegment<int>(
                      value: 3,
                      label: Text(
                        'Configurações',
                        maxLines: 1,
                        softWrap: false,
                      ),
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
            RegistryTokensTab(origin: _currentOrigin())
          else if (_tabIndex == 2)
            RegistryVulnerabilitiesTab(overview: overview, canAdmin: canAdmin)
          else if (canAdmin)
            _buildSettingsTab(),
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
                    onTap: () => showRegistryRepositoryDetailSheet(
                      context,
                      repository: repo,
                      canAdmin: canAdmin,
                    ),
                  );
                })
                .toList(growable: false),
          ),
      ],
    );
  }

  Widget _buildSettingsTab() {
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
