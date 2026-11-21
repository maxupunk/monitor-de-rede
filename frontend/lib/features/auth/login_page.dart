import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/config/server_config.dart';
import 'package:netmonitor_app/core/utils/form_validators.dart';
import 'package:netmonitor_app/shared/widgets/nested_panel.dart';
import 'package:netmonitor_app/shared/widgets/section_card.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Valida destino pós-login contra open-redirect (paridade com `LoginPage.vue`).
String sanitizeLoginRedirect(String? redirect) {
  if (redirect == null || redirect.isEmpty) return '/';
  if (!redirect.startsWith('/') || redirect.startsWith('//')) return '/';
  if (redirect == '/login' || redirect.startsWith('/login?')) return '/';
  return redirect;
}

/// Tela de login do NetMonitor (paridade com `LoginPage.vue`).
class LoginPage extends ConsumerStatefulWidget {
  const LoginPage({super.key, this.redirectTo, this.restored});

  final String? redirectTo;
  final bool? restored;

  @override
  ConsumerState<LoginPage> createState() => _LoginPageState();
}

class _LoginPageState extends ConsumerState<LoginPage> {
  final GlobalKey<FormState> _formKey = GlobalKey<FormState>();
  final TextEditingController _emailController = TextEditingController();
  final TextEditingController _passwordController = TextEditingController();
  bool _rememberMe = true;
  bool _obscurePassword = true;

  @override
  void dispose() {
    _emailController.dispose();
    _passwordController.dispose();
    super.dispose();
  }

  bool _isRestored(BuildContext context) {
    if (widget.restored != null) return widget.restored!;
    try {
      return GoRouterState.of(context).uri.queryParameters['restaurado'] == '1';
    } catch (_) {
      return false;
    }
  }

  Future<void> _submit() async {
    if (!_formKey.currentState!.validate()) return;
    final ok = await ref
        .read(authProvider.notifier)
        .login(
          _emailController.text,
          _passwordController.text,
          rememberMe: _rememberMe,
        );
    if (!mounted || !ok) return;
    context.go(sanitizeLoginRedirect(widget.redirectTo));
  }

  @override
  Widget build(BuildContext context) {
    final auth = ref.watch(authProvider);
    final serverConfig = ref.watch(serverConfigProvider);
    final scheme = context.colorScheme;
    final restored = _isRestored(context);

    return Scaffold(
      body: SafeArea(
        child: Center(
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(20),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 440),
              child: SectionCard(
                title: 'NetMonitor',
                subtitle: 'Acesse o monitoramento da sua infraestrutura',
                icon: Icons.hub_outlined,
                child: Form(
                  key: _formKey,
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: <Widget>[
                      if (restored) ...<Widget>[
                        NestedPanel(
                          tone: SemanticTone.success,
                          child: Row(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: <Widget>[
                              Icon(
                                Icons.check_circle_outline,
                                color: context.semanticColors
                                    .resolve(SemanticTone.success, scheme)
                                    .onContainer,
                              ),
                              const SizedBox(width: 10),
                              Expanded(
                                child: Text(
                                  'Backup completo restaurado. As contas de usuário '
                                  'foram substituídas pelas do backup — entre novamente '
                                  'com uma conta válida daquela cópia.',
                                  style: context.textTheme.bodySmall?.copyWith(
                                    fontWeight: FontWeight.w600,
                                  ),
                                ),
                              ),
                            ],
                          ),
                        ),
                        const SizedBox(height: 14),
                      ],
                      if (auth.error != null) ...<Widget>[
                        NestedPanel(
                          tone: SemanticTone.error,
                          child: Row(
                            children: <Widget>[
                              Icon(
                                Icons.error_outline,
                                color: context.semanticColors
                                    .resolve(SemanticTone.error, scheme)
                                    .onContainer,
                              ),
                              const SizedBox(width: 10),
                              Expanded(
                                child: Text(
                                  auth.error!,
                                  style: context.textTheme.bodySmall?.copyWith(
                                    fontWeight: FontWeight.w600,
                                  ),
                                ),
                              ),
                            ],
                          ),
                        ),
                        const SizedBox(height: 14),
                      ],
                      TextFormField(
                        controller: _emailController,
                        keyboardType: TextInputType.emailAddress,
                        textInputAction: TextInputAction.next,
                        autofillHints: const <String>[AutofillHints.email],
                        decoration: const InputDecoration(
                          labelText: 'E-mail',
                          prefixIcon: Icon(Icons.email_outlined),
                        ),
                        validator:
                            FormValidators.compose(<FormFieldValidator<String>>[
                              FormValidators.required('Informe seu e-mail.'),
                              FormValidators.email(),
                            ]),
                      ),
                      const SizedBox(height: 14),
                      TextFormField(
                        controller: _passwordController,
                        obscureText: _obscurePassword,
                        textInputAction: TextInputAction.done,
                        autofillHints: const <String>[AutofillHints.password],
                        onFieldSubmitted: (_) => _submit(),
                        decoration: InputDecoration(
                          labelText: 'Senha',
                          prefixIcon: const Icon(Icons.lock_outline),
                          suffixIcon: IconButton(
                            tooltip: _obscurePassword
                                ? 'Mostrar senha'
                                : 'Ocultar senha',
                            icon: Icon(
                              _obscurePassword
                                  ? Icons.visibility_outlined
                                  : Icons.visibility_off_outlined,
                            ),
                            onPressed: () => setState(
                              () => _obscurePassword = !_obscurePassword,
                            ),
                          ),
                        ),
                        validator: FormValidators.required(
                          'Informe sua senha.',
                        ),
                      ),
                      const SizedBox(height: 8),
                      CheckboxListTile(
                        contentPadding: EdgeInsets.zero,
                        controlAffinity: ListTileControlAffinity.leading,
                        value: _rememberMe,
                        onChanged: (v) =>
                            setState(() => _rememberMe = v ?? true),
                        title: const Text('Permanecer logado'),
                      ),
                      const SizedBox(height: 10),
                      FilledButton.icon(
                        onPressed: auth.loading ? null : _submit,
                        icon: auth.loading
                            ? const SizedBox(
                                width: 18,
                                height: 18,
                                child: CircularProgressIndicator(
                                  strokeWidth: 2,
                                ),
                              )
                            : const Icon(Icons.login),
                        label: const Text('Entrar'),
                      ),
                      if (!kIsWeb) ...<Widget>[
                        const SizedBox(height: 14),
                        const Divider(height: 1),
                        const SizedBox(height: 10),
                        Row(
                          children: <Widget>[
                            Expanded(
                              child: Text(
                                serverConfig.baseUrl.isNotEmpty
                                    ? 'Servidor: ${serverConfig.baseUrl}'
                                    : 'Nenhum servidor selecionado',
                                overflow: TextOverflow.ellipsis,
                                style: context.textTheme.bodySmall?.copyWith(
                                  color: scheme.onSurfaceVariant,
                                ),
                              ),
                            ),
                            TextButton(
                              onPressed: () => context.go('/server-config'),
                              child: const Text('Trocar servidor'),
                            ),
                          ],
                        ),
                      ],
                    ],
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
