import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/areas/areas_provider.dart';
import 'package:netmonitor_app/core/auth/area_access.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/form_validators.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/core/models/admin_models.dart';
import 'package:netmonitor_app/features/core/providers/users_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Gestão de Usuários e Matriz de Acesso por Área (ADR 019 — paridade com `UsersPage.vue`).
class UsersPage extends ConsumerStatefulWidget {
  const UsersPage({super.key});

  @override
  ConsumerState<UsersPage> createState() => _UsersPageState();
}

class _UsersPageState extends ConsumerState<UsersPage> {
  final TextEditingController _searchCtrl = TextEditingController();

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) ref.read(usersProvider.notifier).fetchUsers();
    });
  }

  @override
  void dispose() {
    _searchCtrl.dispose();
    super.dispose();
  }

  Future<void> _openUserForm([ManagedUser? existing]) async {
    final nameCtrl = TextEditingController(text: existing?.fullName ?? '');
    final emailCtrl = TextEditingController(text: existing?.email ?? '');
    final passCtrl = TextEditingController();
    UserRole role = existing?.role ?? UserRole.operator;
    final Map<ProductArea, UserRole?> areaRoles = <ProductArea, UserRole?>{
      for (final a in ProductArea.optionalAreas) a: existing?.areaRoles[a],
    };
    bool active = existing?.active ?? true;
    final formKey = GlobalKey<FormState>();
    var saving = false;
    String? error;

    await showAdaptiveFormSheet<void>(
      context: context,
      title: existing == null ? 'Novo usuário' : 'Editar usuário',
      subtitle:
          'Defina o papel global e as permissões por área do sistema (ADR 019).',
      maxWidth: 620,
      builder: (sheetCtx) {
        final areasState = ref.watch(areasProvider);
        return StatefulBuilder(
          builder: (context, setModalState) {
            return Form(
              key: formKey,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  if (error != null) ...<Widget>[
                    NestedPanel(tone: SemanticTone.error, child: Text(error!)),
                    const SizedBox(height: 10),
                  ],
                  TextFormField(
                    controller: nameCtrl,
                    decoration: const InputDecoration(
                      labelText: 'Nome completo',
                      prefixIcon: Icon(Icons.person_outline),
                    ),
                    validator: FormValidators.required(
                      'Informe o nome completo.',
                    ),
                  ),
                  const SizedBox(height: 12),
                  TextFormField(
                    controller: emailCtrl,
                    keyboardType: TextInputType.emailAddress,
                    decoration: const InputDecoration(
                      labelText: 'E-mail',
                      prefixIcon: Icon(Icons.email_outlined),
                    ),
                    validator:
                        FormValidators.compose(<FormFieldValidator<String>>[
                          FormValidators.required('Informe o e-mail.'),
                          FormValidators.email(),
                        ]),
                  ),
                  const SizedBox(height: 12),
                  TextFormField(
                    controller: passCtrl,
                    obscureText: true,
                    decoration: InputDecoration(
                      labelText: existing == null
                          ? 'Senha'
                          : 'Nova senha (deixe em branco para manter)',
                      prefixIcon: const Icon(Icons.lock_outline),
                    ),
                    validator: existing == null
                        ? FormValidators.password()
                        : (v) {
                            if (v == null || v.isEmpty) return null;
                            return FormValidators.password()(v);
                          },
                  ),
                  const SizedBox(height: 12),
                  AppDropdownFormField<UserRole>(
                    initialValue: role,
                    decoration: const InputDecoration(
                      labelText: 'Papel global (Núcleo)',
                    ),
                    items: <DropdownMenuItem<UserRole>>[
                      for (final r in UserRole.values)
                        DropdownMenuItem<UserRole>(
                          value: r,
                          child: Text(
                            '${r.label} — ${r.description}',
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                    ],
                    onChanged: (v) {
                      if (v != null) setModalState(() => role = v);
                    },
                  ),
                  const SizedBox(height: 14),
                  if (role == UserRole.admin)
                    const NestedPanel(
                      tone: SemanticTone.info,
                      child: Text(
                        'Administradores possuem acesso total de leitura e escrita em todas '
                        'as áreas habilitadas do sistema.',
                      ),
                    )
                  else
                    NestedPanel(
                      child: LayoutBuilder(
                        builder: (context, panelConstraints) {
                          final narrowPanel = panelConstraints.maxWidth < 420;
                          return Column(
                            crossAxisAlignment: CrossAxisAlignment.stretch,
                            children: <Widget>[
                              Text(
                                'Acesso por área opcional',
                                style: context.textTheme.titleSmall?.copyWith(
                                  fontWeight: FontWeight.w700,
                                ),
                              ),
                              const SizedBox(height: 4),
                              Text(
                                'O Núcleo herda o papel global. Escolha o nível de acesso nas demais áreas:',
                                style: context.textTheme.bodySmall?.copyWith(
                                  color: context.colorScheme.onSurfaceVariant,
                                ),
                              ),
                              const SizedBox(height: 10),
                              for (final area
                                  in ProductArea.optionalAreas) ...<Widget>[
                                Padding(
                                  padding: const EdgeInsets.only(bottom: 10),
                                  child: Column(
                                    crossAxisAlignment:
                                        CrossAxisAlignment.start,
                                    children: <Widget>[
                                      Row(
                                        children: <Widget>[
                                          Icon(area.defaultIcon, size: 18),
                                          const SizedBox(width: 6),
                                          Expanded(
                                            child: Text(
                                              area.defaultTitle,
                                              maxLines: 1,
                                              overflow: TextOverflow.ellipsis,
                                              style: const TextStyle(
                                                fontWeight: FontWeight.w700,
                                              ),
                                            ),
                                          ),
                                          if (!areasState.isEnabled(
                                            area,
                                          )) ...<Widget>[
                                            const SizedBox(width: 8),
                                            const SemanticChip(
                                              label: 'Desligada',
                                              tone: SemanticTone.warning,
                                              compact: true,
                                            ),
                                          ],
                                        ],
                                      ),
                                      const SizedBox(height: 6),
                                      if (narrowPanel)
                                        Wrap(
                                          spacing: 6,
                                          runSpacing: 6,
                                          children: <Widget>[
                                            for (final (val, label)
                                                in const <(UserRole?, String)>[
                                                  (null, 'Sem acesso'),
                                                  (
                                                    UserRole.viewer,
                                                    'Visualizador',
                                                  ),
                                                  (
                                                    UserRole.operator,
                                                    'Operador',
                                                  ),
                                                ])
                                              ChoiceChip(
                                                visualDensity:
                                                    VisualDensity.compact,
                                                selected:
                                                    areaRoles[area] == val,
                                                label: Text(label),
                                                onSelected: (_) =>
                                                    setModalState(
                                                      () =>
                                                          areaRoles[area] = val,
                                                    ),
                                              ),
                                          ],
                                        )
                                      else
                                        SegmentedButton<UserRole?>(
                                          showSelectedIcon: false,
                                          style: const ButtonStyle(
                                            visualDensity:
                                                VisualDensity.compact,
                                          ),
                                          segments:
                                              const <ButtonSegment<UserRole?>>[
                                                ButtonSegment<UserRole?>(
                                                  value: null,
                                                  label: Text('Sem acesso'),
                                                ),
                                                ButtonSegment<UserRole?>(
                                                  value: UserRole.viewer,
                                                  label: Text('Visualizador'),
                                                ),
                                                ButtonSegment<UserRole?>(
                                                  value: UserRole.operator,
                                                  label: Text('Operador'),
                                                ),
                                              ],
                                          selected: <UserRole?>{
                                            areaRoles[area],
                                          },
                                          onSelectionChanged: (s) =>
                                              setModalState(
                                                () => areaRoles[area] = s.first,
                                              ),
                                        ),
                                    ],
                                  ),
                                ),
                              ],
                            ],
                          );
                        },
                      ),
                    ),
                  const SizedBox(height: 8),
                  SwitchListTile(
                    contentPadding: EdgeInsets.zero,
                    value: active,
                    onChanged: (v) => setModalState(() => active = v),
                    title: const Text('Conta ativa'),
                  ),
                  const SizedBox(height: 14),
                  FilledButton.icon(
                    onPressed: saving
                        ? null
                        : () async {
                            if (!formKey.currentState!.validate()) return;
                            setModalState(() {
                              saving = true;
                              error = null;
                            });
                            final serializedAreas = <String, String>{};
                            if (role != UserRole.admin) {
                              for (final entry in areaRoles.entries) {
                                if (entry.value != null) {
                                  serializedAreas[entry.key.id] =
                                      entry.value!.id;
                                }
                              }
                            }
                            final payload = <String, dynamic>{
                              'name': nameCtrl.text.trim(),
                              'fullName': nameCtrl.text.trim(),
                              'email': emailCtrl.text.trim(),
                              'password': passCtrl.text,
                              'role': role.id,
                              'areaRoles': serializedAreas,
                              'active': active,
                            };
                            final notifier = ref.read(usersProvider.notifier);
                            final ok = existing == null
                                ? await notifier.createUser(payload)
                                : await notifier.updateUser(
                                    existing.id,
                                    payload,
                                  );
                            if (!sheetCtx.mounted) return;
                            if (ok) {
                              Navigator.of(sheetCtx).pop();
                            } else {
                              setModalState(() {
                                saving = false;
                                error =
                                    ref.read(usersProvider).error ??
                                    'Erro ao salvar usuário.';
                              });
                            }
                          },
                    icon: saving
                        ? const SizedBox(
                            width: 18,
                            height: 18,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          )
                        : const Icon(Icons.save_outlined),
                    label: Text(
                      existing == null ? 'Criar usuário' : 'Salvar alterações',
                    ),
                  ),
                ],
              ),
            );
          },
        );
      },
    );

    nameCtrl.dispose();
    emailCtrl.dispose();
    passCtrl.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(usersProvider);
    final auth = ref.watch(authProvider);

    if (!auth.isAdmin) {
      return const EmptyStateView(
        title: 'Acesso restrito a administradores',
        description:
            'Somente administradores podem gerenciar usuários e permissões.',
        icon: Icons.lock_outline,
        tone: SemanticTone.error,
      );
    }

    final q = _searchCtrl.text.trim().toLowerCase();
    final filtered = state.users
        .where((u) {
          if (q.isEmpty) return true;
          return u.fullName.toLowerCase().contains(q) ||
              u.email.toLowerCase().contains(q);
        })
        .toList(growable: false);
    final adminCount = state.users
        .where((u) => u.role == UserRole.admin && u.active)
        .length;

    return LayoutBuilder(
      builder: (context, constraints) {
        final isCompact = constraints.maxWidth < 680;
        final scheme = context.colorScheme;

        return RefreshIndicator(
          onRefresh: () => ref.read(usersProvider.notifier).fetchUsers(),
          child: ListView(
            children: <Widget>[
              if (isCompact)
                Row(
                  children: <Widget>[
                    Expanded(
                      child: Text(
                        'Usuários e acessos',
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: context.textTheme.headlineSmall?.copyWith(
                          fontWeight: FontWeight.w800,
                          color: scheme.onSurface,
                        ),
                      ),
                    ),
                    const SizedBox(width: 8),
                    FilledButton.icon(
                      style: FilledButton.styleFrom(
                        visualDensity: VisualDensity.compact,
                        padding: const EdgeInsets.symmetric(
                          horizontal: 12,
                          vertical: 8,
                        ),
                      ),
                      onPressed: () => _openUserForm(),
                      icon: const Icon(Icons.person_add_outlined, size: 18),
                      label: const Text('Novo'),
                    ),
                  ],
                )
              else
                Wrap(
                  spacing: 12,
                  runSpacing: 10,
                  alignment: WrapAlignment.spaceBetween,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: <Widget>[
                    Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Text(
                          'Usuários e acessos',
                          style: context.textTheme.headlineSmall?.copyWith(
                            fontWeight: FontWeight.w800,
                            color: scheme.onSurface,
                          ),
                        ),
                        const SizedBox(height: 2),
                        Text(
                          'Papéis globais e permissões por área do produto (ADR 019)',
                          style: context.textTheme.bodyMedium?.copyWith(
                            color: scheme.onSurfaceVariant,
                          ),
                        ),
                      ],
                    ),
                    FilledButton.icon(
                      onPressed: () => _openUserForm(),
                      icon: const Icon(Icons.person_add_outlined, size: 18),
                      label: const Text('Novo usuário'),
                    ),
                  ],
                ),
              const SizedBox(height: 10),
              UnifiedFilterBar(
                searchController: _searchCtrl,
                searchHint: 'Buscar por nome ou e-mail...',
                onSearchChanged: (_) => setState(() {}),
              ),
              const SizedBox(height: 14),
              if (state.loading && state.users.isEmpty)
                const LoadingStateView(message: 'Carregando usuários...')
              else if (state.error != null && state.users.isEmpty)
                ErrorStateView(
                  message: state.error!,
                  onRetry: () => ref.read(usersProvider.notifier).fetchUsers(),
                )
              else
                for (final user in filtered) ...<Widget>[
                  Builder(
                    builder: (context) {
                      final isSelf = auth.user?.id == user.id;
                      final isLastAdmin =
                          user.role == UserRole.admin && adminCount <= 1;
                      final roleTone = switch (user.role) {
                        UserRole.admin => SemanticTone.primary,
                        UserRole.operator => SemanticTone.info,
                        UserRole.viewer => SemanticTone.secondary,
                      };

                      return SectionCard(
                        title: user.fullName,
                        subtitle: user.email,
                        icon: user.role.icon,
                        iconTone: roleTone,
                        trailing: SemanticChip(
                          label: user.active ? 'Ativo' : 'Inativo',
                          tone: user.active
                              ? SemanticTone.success
                              : SemanticTone.error,
                          compact: true,
                        ),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: <Widget>[
                            Wrap(
                              spacing: 6,
                              runSpacing: 6,
                              children: <Widget>[
                                SemanticChip(
                                  label: user.role.label,
                                  tone: roleTone,
                                  compact: true,
                                ),
                                if (user.role == UserRole.admin)
                                  const SemanticChip(
                                    label: 'Todas as áreas',
                                    tone: SemanticTone.success,
                                    compact: true,
                                  )
                                else
                                  for (final entry in user.areaRoles.entries)
                                    SemanticChip(
                                      label:
                                          '${entry.key.defaultTitle}: ${entry.value.label}',
                                      tone: entry.value == UserRole.operator
                                          ? SemanticTone.info
                                          : SemanticTone.secondary,
                                      compact: true,
                                    ),
                              ],
                            ),
                            const SizedBox(height: 8),
                            Text(
                              'Último acesso: ${formatRelativeTime(user.lastLoginAt)}',
                              style: context.textTheme.bodySmall?.copyWith(
                                color: context.colorScheme.onSurfaceVariant,
                              ),
                            ),
                            const SizedBox(height: 10),
                            SymmetricalActionBar(
                              spacing: 8,
                              runSpacing: 8,
                              children: <Widget>[
                                OutlinedButton.icon(
                                  onPressed: () => _openUserForm(user),
                                  icon: const Icon(
                                    Icons.edit_outlined,
                                    size: 18,
                                  ),
                                  label: const Text('Editar'),
                                ),
                                OutlinedButton.icon(
                                  onPressed: isSelf || isLastAdmin
                                      ? null
                                      : () async {
                                          final ok = await showConfirmActionDialog(
                                            context: context,
                                            title: 'Excluir usuário?',
                                            message:
                                                'A conta de "${user.fullName}" será removida permanentemente.',
                                            confirmLabel: 'Excluir',
                                          );
                                          if (ok) {
                                            await ref
                                                .read(usersProvider.notifier)
                                                .deleteUser(user.id);
                                          }
                                        },
                                  icon: Icon(
                                    Icons.delete_outline,
                                    size: 18,
                                    color: isSelf || isLastAdmin
                                        ? null
                                        : context.colorScheme.error,
                                  ),
                                  label: Text(
                                    'Excluir',
                                    style: TextStyle(
                                      color: isSelf || isLastAdmin
                                          ? null
                                          : context.colorScheme.error,
                                    ),
                                  ),
                                ),
                              ],
                            ),
                          ],
                        ),
                      );
                    },
                  ),
                  const SizedBox(height: 10),
                ],
            ],
          ),
        );
      },
    );
  }
}
