import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/form_validators.dart';
import 'package:netmonitor_app/shared/widgets/nested_panel.dart';
import 'package:netmonitor_app/shared/widgets/section_card.dart';
import 'package:netmonitor_app/shared/widgets/semantic_chip.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Cadastro do primeiro usuário administrador (paridade com `SetupPage.vue`).
class SetupPage extends ConsumerStatefulWidget {
  const SetupPage({super.key});

  @override
  ConsumerState<SetupPage> createState() => _SetupPageState();
}

class _SetupPageState extends ConsumerState<SetupPage> {
  final GlobalKey<FormState> _formKey = GlobalKey<FormState>();
  final TextEditingController _nameController = TextEditingController();
  final TextEditingController _emailController = TextEditingController();
  final TextEditingController _passwordController = TextEditingController();
  final TextEditingController _confirmController = TextEditingController();
  final TextEditingController _tokenController = TextEditingController();
  bool _obscurePassword = true;

  @override
  void dispose() {
    _nameController.dispose();
    _emailController.dispose();
    _passwordController.dispose();
    _confirmController.dispose();
    _tokenController.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    if (!_formKey.currentState!.validate()) return;
    final ok = await ref.read(authProvider.notifier).completeSetup(
          name: _nameController.text,
          email: _emailController.text,
          password: _passwordController.text,
          token: _tokenController.text,
        );
    if (!mounted || !ok) return;
    context.go('/');
  }

  @override
  Widget build(BuildContext context) {
    final auth = ref.watch(authProvider);
    final strength = FormValidators.evaluatePasswordStrength(_passwordController.text);
    final strengthTone = switch (strength.tone) {
      PasswordStrengthTone.weak => SemanticTone.error,
      PasswordStrengthTone.fair => SemanticTone.warning,
      PasswordStrengthTone.good => SemanticTone.info,
      PasswordStrengthTone.strong => SemanticTone.success,
      PasswordStrengthTone.none => SemanticTone.secondary,
    };

    return Scaffold(
      body: SafeArea(
        child: Center(
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(20),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 480),
              child: SectionCard(
                title: 'Instalação inicial',
                subtitle: 'Crie o primeiro administrador desta central NetMonitor.',
                icon: Icons.admin_panel_settings_outlined,
                child: Form(
                  key: _formKey,
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: <Widget>[
                      if (auth.error != null) ...<Widget>[
                        NestedPanel(
                          tone: SemanticTone.error,
                          child: Text(
                            auth.error!,
                            style: context.textTheme.bodySmall?.copyWith(
                              fontWeight: FontWeight.w600,
                            ),
                          ),
                        ),
                        const SizedBox(height: 14),
                      ],
                      TextFormField(
                        controller: _nameController,
                        textInputAction: TextInputAction.next,
                        decoration: const InputDecoration(
                          labelText: 'Nome completo',
                          prefixIcon: Icon(Icons.person_outline),
                        ),
                        validator: FormValidators.compose(<FormFieldValidator<String>>[
                          FormValidators.required('Informe seu nome.'),
                          FormValidators.minLength(
                            FormValidators.minNameLength,
                            'O nome precisa ter ao menos ${FormValidators.minNameLength} caracteres.',
                          ),
                        ]),
                      ),
                      const SizedBox(height: 12),
                      TextFormField(
                        controller: _emailController,
                        keyboardType: TextInputType.emailAddress,
                        textInputAction: TextInputAction.next,
                        decoration: const InputDecoration(
                          labelText: 'E-mail',
                          prefixIcon: Icon(Icons.email_outlined),
                        ),
                        validator: FormValidators.compose(<FormFieldValidator<String>>[
                          FormValidators.required('Informe seu e-mail.'),
                          FormValidators.email(),
                        ]),
                      ),
                      const SizedBox(height: 12),
                      TextFormField(
                        controller: _passwordController,
                        obscureText: _obscurePassword,
                        textInputAction: TextInputAction.next,
                        onChanged: (_) => setState(() {}),
                        decoration: InputDecoration(
                          labelText: 'Senha',
                          prefixIcon: const Icon(Icons.lock_outline),
                          suffixIcon: IconButton(
                            icon: Icon(
                              _obscurePassword
                                  ? Icons.visibility_outlined
                                  : Icons.visibility_off_outlined,
                            ),
                            onPressed: () =>
                                setState(() => _obscurePassword = !_obscurePassword),
                          ),
                        ),
                        validator: FormValidators.password(),
                      ),
                      if (strength.score > 0) ...<Widget>[
                        const SizedBox(height: 6),
                        Row(
                          children: <Widget>[
                            const Text('Força da senha: '),
                            SemanticChip(
                              label: strength.label,
                              tone: strengthTone,
                              compact: true,
                            ),
                          ],
                        ),
                      ],
                      const SizedBox(height: 12),
                      TextFormField(
                        controller: _confirmController,
                        obscureText: _obscurePassword,
                        textInputAction: TextInputAction.next,
                        decoration: const InputDecoration(
                          labelText: 'Confirmar senha',
                          prefixIcon: Icon(Icons.lock_reset_outlined),
                        ),
                        validator: FormValidators.matches(
                          () => _passwordController.text,
                          'As senhas não coincidem.',
                        ),
                      ),
                      const SizedBox(height: 12),
                      TextFormField(
                        controller: _tokenController,
                        textInputAction: TextInputAction.done,
                        onFieldSubmitted: (_) => _submit(),
                        decoration: const InputDecoration(
                          labelText: 'Token de instalação do servidor',
                          helperText: 'Exibido nos logs de inicialização do backend.',
                          prefixIcon: Icon(Icons.vpn_key_outlined),
                        ),
                        validator: FormValidators.required(
                          'Informe o token de instalação do servidor.',
                        ),
                      ),
                      const SizedBox(height: 18),
                      FilledButton.icon(
                        onPressed: auth.loading ? null : _submit,
                        icon: auth.loading
                            ? const SizedBox(
                                width: 18,
                                height: 18,
                                child: CircularProgressIndicator(strokeWidth: 2),
                              )
                            : const Icon(Icons.check_circle_outline),
                        label: const Text('Concluir instalação'),
                      ),
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
