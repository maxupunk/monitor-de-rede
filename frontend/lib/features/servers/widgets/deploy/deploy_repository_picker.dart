import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/deploy_models.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Filtra repositórios por `fullName` ou `description` (case-insensitive).
List<DeployRepositoryView> filterDeployRepositories(
  List<DeployRepositoryView> repositories,
  String query,
) {
  final term = normalizeGithubRepository(query).toLowerCase();
  if (term.isEmpty) return repositories;
  return repositories
      .where((repo) {
        if (repo.fullName.toLowerCase().contains(term)) return true;
        final desc = repo.description;
        return desc != null && desc.toLowerCase().contains(term);
      })
      .toList(growable: false);
}

/// Normaliza entradas coladas como URL do GitHub (`https://github.com/dono/repo`,
/// `git@github.com:dono/repo.git`, `github.com/dono/repo`) para `dono/repo`.
String normalizeGithubRepository(String raw) {
  var value = raw.trim();
  if (value.isEmpty) return '';

  final sshMatch = RegExp(
    r'^git@github\.com:([^/\s]+/[^/\s]+?)(?:\.git)?/?$',
    caseSensitive: false,
  ).firstMatch(value);
  if (sshMatch != null) {
    return sshMatch.group(1)!;
  }

  final httpsMatch = RegExp(
    r'^(?:https?://)?(?:www\.)?github\.com/([^/\s]+/[^/\s#?]+)',
    caseSensitive: false,
  ).firstMatch(value);
  if (httpsMatch != null) {
    value = httpsMatch.group(1)!;
  }

  value = value.replaceFirst(RegExp(r'\.git$', caseSensitive: false), '');
  return value.replaceFirst(RegExp(r'/+$'), '');
}

/// Sugere um nome válido de projeto Docker Compose (minúsculo, letras/números/`-`)
/// a partir do nome da aplicação ou do repositório.
String suggestComposeProjectName({
  required String appName,
  required String repository,
}) {
  final normalizedRepo = normalizeGithubRepository(repository);
  final repoLeaf = normalizedRepo.contains('/')
      ? normalizedRepo.split('/').last
      : normalizedRepo;
  final source = appName.trim().isNotEmpty ? appName.trim() : repoLeaf;
  if (source.isEmpty) return '';

  final slug = sanitizeFileSlug(source, fallback: '')
      .toLowerCase()
      .replaceAll(RegExp(r'[^a-z0-9_-]+'), '-')
      .replaceAll(RegExp(r'-{2,}'), '-')
      .replaceAll(RegExp(r'^[-_]+|[-_]+$'), '');
  return slug;
}

/// Seletor pesquisável de repositórios do GitHub App (estilo Dokploy), usado no
/// assistente de deploy contínuo.
class DeployRepositoryPicker extends StatelessWidget {
  const DeployRepositoryPicker({
    super.key,
    required this.repositories,
    required this.query,
    required this.selectedRepository,
    required this.loading,
    required this.expanded,
    required this.accountLabel,
    required this.connectionInstalled,
    required this.onSelect,
    required this.onRefresh,
    required this.onToggleExpanded,
    this.error,
  });

  final List<DeployRepositoryView> repositories;
  final String query;
  final String selectedRepository;
  final bool loading;
  final String? error;
  final bool expanded;
  final String accountLabel;
  final bool connectionInstalled;
  final ValueChanged<DeployRepositoryView> onSelect;
  final VoidCallback onRefresh;
  final VoidCallback onToggleExpanded;

  DeployRepositoryView? _findSelected() {
    final normalized = selectedRepository.trim().toLowerCase();
    if (normalized.isEmpty) return null;
    for (final repo in repositories) {
      if (repo.fullName.toLowerCase() == normalized) return repo;
    }
    return null;
  }

  @override
  Widget build(BuildContext context) {
    if (!connectionInstalled) {
      return NestedPanel(
        tone: SemanticTone.warning,
        child: Text(
          'Conclua a instalação do GitHub App em $accountLabel para listar os repositórios automaticamente.',
          style: context.textTheme.bodySmall,
        ),
      );
    }

    if (loading && repositories.isEmpty) {
      return NestedPanel(
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
                'Carregando repositórios de $accountLabel...',
                style: context.textTheme.bodySmall,
              ),
            ),
          ],
        ),
      );
    }

    if (error != null && repositories.isEmpty) {
      return NestedPanel(
        tone: SemanticTone.warning,
        child: Row(
          children: <Widget>[
            Expanded(child: Text(error!, style: context.textTheme.bodySmall)),
            const SizedBox(width: 8),
            TextButton(
              onPressed: loading ? null : onRefresh,
              child: const Text('Tentar novamente'),
            ),
          ],
        ),
      );
    }

    final selected = _findSelected();
    if (!expanded && selected != null) {
      return NestedPanel(
        tone: SemanticTone.success,
        child: Row(
          children: <Widget>[
            Icon(
              selected.isPrivate ? Icons.lock_outline : Icons.public,
              size: 16,
            ),
            const SizedBox(width: 8),
            Expanded(
              child: Text(
                '${selected.fullName} · branch ${selected.defaultBranch}',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: context.textTheme.bodySmall?.copyWith(
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
            const SizedBox(width: 8),
            TextButton(
              onPressed: onToggleExpanded,
              child: const Text('Trocar'),
            ),
          ],
        ),
      );
    }

    final filtered = filterDeployRepositories(repositories, query);
    final scheme = context.colorScheme;
    final countText = filtered.length == repositories.length
        ? '${repositories.length}'
        : '${filtered.length} de ${repositories.length}';

    return Container(
      decoration: BoxDecoration(
        color: scheme.surface,
        borderRadius: AppTheme.borderRadius,
        border: Border.all(color: scheme.outlineVariant),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Padding(
            padding: const EdgeInsets.fromLTRB(12, 6, 6, 6),
            child: Row(
              children: <Widget>[
                Expanded(
                  child: Text(
                    'Repositórios de $accountLabel ($countText)',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: context.textTheme.labelMedium?.copyWith(
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                ),
                if (selected != null)
                  TextButton(
                    onPressed: onToggleExpanded,
                    child: const Text('Ocultar'),
                  ),
                IconButton(
                  tooltip: 'Atualizar repositórios',
                  visualDensity: VisualDensity.compact,
                  onPressed: loading ? null : onRefresh,
                  icon: loading
                      ? const SizedBox(
                          width: 15,
                          height: 15,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        )
                      : const Icon(Icons.refresh, size: 18),
                ),
              ],
            ),
          ),
          Divider(height: 1, color: scheme.outlineVariant),
          if (repositories.isEmpty)
            Padding(
              padding: const EdgeInsets.all(12),
              child: Text(
                'Nenhum repositório liberado para este GitHub App. Ajuste o acesso nas configurações do app no GitHub ou digite dono/repo manualmente acima.',
                style: context.textTheme.bodySmall,
              ),
            )
          else if (filtered.isEmpty)
            Padding(
              padding: const EdgeInsets.all(12),
              child: Text(
                'Nenhum repositório corresponde a "${query.trim()}". Você também pode digitar dono/repo manualmente acima.',
                style: context.textTheme.bodySmall,
              ),
            )
          else
            ConstrainedBox(
              constraints: const BoxConstraints(maxHeight: 220),
              child: ListView.separated(
                shrinkWrap: true,
                itemCount: filtered.length,
                separatorBuilder: (_, _) =>
                    Divider(height: 1, color: scheme.outlineVariant),
                itemBuilder: (context, index) {
                  final repo = filtered[index];
                  final isSelected =
                      repo.fullName.toLowerCase() ==
                      selectedRepository.trim().toLowerCase();
                  final metaParts = <String>[
                    'branch ${repo.defaultBranch}',
                    if (repo.pushedAt != null && repo.pushedAt!.isNotEmpty)
                      'push ${formatRelativeTime(repo.pushedAt)}',
                  ];
                  return InkWell(
                    onTap: () => onSelect(repo),
                    child: Container(
                      color: isSelected
                          ? scheme.primaryContainer.withValues(alpha: 0.35)
                          : null,
                      padding: const EdgeInsets.symmetric(
                        horizontal: 12,
                        vertical: 8,
                      ),
                      child: Row(
                        children: <Widget>[
                          Icon(
                            repo.isPrivate ? Icons.lock_outline : Icons.public,
                            size: 16,
                            color: isSelected
                                ? scheme.primary
                                : scheme.onSurfaceVariant,
                          ),
                          const SizedBox(width: 10),
                          Expanded(
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              mainAxisSize: MainAxisSize.min,
                              children: <Widget>[
                                Text(
                                  repo.fullName,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: context.textTheme.bodyMedium?.copyWith(
                                    fontWeight: isSelected
                                        ? FontWeight.w700
                                        : FontWeight.w600,
                                  ),
                                ),
                                if (repo.description != null &&
                                    repo.description!.isNotEmpty)
                                  Text(
                                    repo.description!,
                                    maxLines: 1,
                                    overflow: TextOverflow.ellipsis,
                                    style: context.textTheme.bodySmall,
                                  ),
                                Text(
                                  metaParts.join(' · '),
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: context.textTheme.labelSmall?.copyWith(
                                    color: scheme.onSurfaceVariant,
                                  ),
                                ),
                              ],
                            ),
                          ),
                          const SizedBox(width: 8),
                          SemanticChip(
                            label: repo.isPrivate ? 'Privado' : 'Público',
                            tone: repo.isPrivate
                                ? SemanticTone.warning
                                : SemanticTone.info,
                            compact: true,
                          ),
                          if (isSelected) ...<Widget>[
                            const SizedBox(width: 6),
                            Icon(
                              Icons.check_circle,
                              size: 16,
                              color: scheme.primary,
                            ),
                          ],
                        ],
                      ),
                    ),
                  );
                },
              ),
            ),
        ],
      ),
    );
  }
}
