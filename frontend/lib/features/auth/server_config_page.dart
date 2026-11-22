import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/config/server_config.dart';
import 'package:netmonitor_app/core/utils/form_validators.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Configuração de servidor NetMonitor (principalmente para Android nativo).
class ServerConfigPage extends ConsumerStatefulWidget {
  const ServerConfigPage({super.key});

  @override
  ConsumerState<ServerConfigPage> createState() => _ServerConfigPageState();
}

class _ServerConfigPageState extends ConsumerState<ServerConfigPage> {
  final GlobalKey<FormState> _formKey = GlobalKey<FormState>();
  late final TextEditingController _urlController;
  bool _testing = false;
  ServerProbeResult? _probeResult;

  @override
  void initState() {
    super.initState();
    _urlController = TextEditingController(
      text: ref.read(serverConfigProvider).baseUrl,
    );
  }

  @override
  void dispose() {
    _urlController.dispose();
    super.dispose();
  }

  Future<void> _testConnection() async {
    if (!_formKey.currentState!.validate()) return;
    setState(() {
      _testing = true;
      _probeResult = null;
    });
    final result = await ref
        .read(serverConfigProvider.notifier)
        .probeServer(_urlController.text);
    if (!mounted) return;
    setState(() {
      _testing = false;
      _probeResult = result;
    });
  }

  Future<void> _saveAndContinue([String? explicitUrl]) async {
    final raw = explicitUrl ?? _urlController.text;
    if (explicitUrl == null && !_formKey.currentState!.validate()) return;
    setState(() => _testing = true);
    final result = await ref
        .read(serverConfigProvider.notifier)
        .probeServer(raw);
    if (!mounted) return;
    setState(() {
      _testing = false;
      _probeResult = result;
    });
    if (!result.ok) return;

    await ref
        .read(serverConfigProvider.notifier)
        .setServerUrl(result.normalizedUrl);
    await ref.read(authProvider.notifier).refreshSetupStatus();
    if (!mounted) return;
    if (result.needsSetup == true) {
      context.go('/setup');
    } else {
      context.go('/login');
    }
  }

  @override
  Widget build(BuildContext context) {
    final config = ref.watch(serverConfigProvider);
    final scheme = context.colorScheme;

    return Scaffold(
      body: SafeArea(
        child: Center(
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(20),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 480),
              child: SectionCard(
                title: 'Conectar ao servidor NetMonitor',
                subtitle: 'Informe o endereço IP ou domínio da sua central.',
                icon: Icons.dns_outlined,
                child: Form(
                  key: _formKey,
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: <Widget>[
                      TextFormField(
                        controller: _urlController,
                        keyboardType: TextInputType.url,
                        autocorrect: false,
                        decoration: const InputDecoration(
                          labelText: 'Endereço do servidor',
                          hintText:
                              'https://netmonitor.empresa.local ou http://10.8.0.1:3333',
                          prefixIcon: Icon(Icons.link),
                        ),
                        validator:
                            FormValidators.compose(<FormFieldValidator<String>>[
                              FormValidators.required(
                                'Informe o endereço do servidor.',
                              ),
                              (value) {
                                final normalized = normalizeServerUrl(
                                  value ?? '',
                                );
                                return FormValidators.url()(normalized);
                              },
                            ]),
                      ),
                      const SizedBox(height: 14),
                      if (_probeResult != null) ...<Widget>[
                        NestedPanel(
                          tone: _probeResult!.ok
                              ? SemanticTone.success
                              : SemanticTone.error,
                          child: Row(
                            children: <Widget>[
                              Icon(
                                _probeResult!.ok
                                    ? Icons.check_circle_outline
                                    : Icons.error_outline,
                                color: context.semanticColors
                                    .resolve(
                                      _probeResult!.ok
                                          ? SemanticTone.success
                                          : SemanticTone.error,
                                      scheme,
                                    )
                                    .onContainer,
                              ),
                              const SizedBox(width: 10),
                              Expanded(
                                child: Text(
                                  _probeResult!.ok
                                      ? 'Servidor acessível (${_probeResult!.latencyMs ?? 0} ms).'
                                      : (_probeResult!.error ??
                                            'Falha ao conectar.'),
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
                      SymmetricalActionBar(
                        spacing: 10,
                        runSpacing: 10,
                        children: <Widget>[
                          OutlinedButton.icon(
                            onPressed: _testing ? null : _testConnection,
                            icon: const Icon(Icons.network_ping),
                            label: const Text('Testar'),
                          ),
                          FilledButton.icon(
                            onPressed: _testing
                                ? null
                                : () => _saveAndContinue(),
                            icon: _testing
                                ? const SizedBox(
                                    width: 16,
                                    height: 16,
                                    child: CircularProgressIndicator(
                                      strokeWidth: 2,
                                    ),
                                  )
                                : const Icon(Icons.arrow_forward),
                            label: const Text('Conectar'),
                          ),
                        ],
                      ),
                      if (config.recentServers.isNotEmpty) ...<Widget>[
                        const SizedBox(height: 20),
                        Text(
                          'Servidores recentes',
                          style: context.textTheme.labelLarge?.copyWith(
                            fontWeight: FontWeight.w700,
                            color: scheme.onSurfaceVariant,
                          ),
                        ),
                        const SizedBox(height: 8),
                        for (final server in config.recentServers)
                          Padding(
                            padding: const EdgeInsets.only(bottom: 6),
                            child: NestedPanel(
                              padding: const EdgeInsets.symmetric(
                                horizontal: 12,
                                vertical: 8,
                              ),
                              child: Row(
                                children: <Widget>[
                                  Expanded(
                                    child: InkWell(
                                      onTap: _testing
                                          ? null
                                          : () {
                                              _urlController.text = server;
                                              _saveAndContinue(server);
                                            },
                                      child: Row(
                                        children: <Widget>[
                                          const Icon(Icons.history, size: 18),
                                          const SizedBox(width: 8),
                                          Expanded(
                                            child: Text(
                                              server,
                                              overflow: TextOverflow.ellipsis,
                                              style: const TextStyle(
                                                fontWeight: FontWeight.w600,
                                              ),
                                            ),
                                          ),
                                        ],
                                      ),
                                    ),
                                  ),
                                  if (server == config.baseUrl) ...<Widget>[
                                    const SizedBox(width: 6),
                                    const SemanticChip(
                                      label: 'Atual',
                                      tone: SemanticTone.primary,
                                      compact: true,
                                    ),
                                  ],
                                  IconButton(
                                    tooltip: 'Remover',
                                    visualDensity: VisualDensity.compact,
                                    icon: const Icon(Icons.close, size: 18),
                                    onPressed: () => ref
                                        .read(serverConfigProvider.notifier)
                                        .removeRecentServer(server),
                                  ),
                                ],
                              ),
                            ),
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
