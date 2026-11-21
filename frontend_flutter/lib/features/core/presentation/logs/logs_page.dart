import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/core/models/log_models.dart';
import 'package:netmonitor_app/features/core/providers/logs_provider.dart';
import 'package:netmonitor_app/features/core/providers/monitors_provider.dart';
import 'package:netmonitor_app/features/core/providers/settings_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Central de Logs Syslog (paridade 100% com `LogsPage.vue`).
class LogsPage extends ConsumerStatefulWidget {
  const LogsPage({super.key, this.deviceId, this.embedded = false});

  final int? deviceId;
  final bool embedded;

  @override
  ConsumerState<LogsPage> createState() => _LogsPageState();
}

class _LogsPageState extends ConsumerState<LogsPage> {
  final TextEditingController _searchCtrl = TextEditingController();

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      if (!mounted) return;
      final notifier = ref.read(logsProvider.notifier);
      if (widget.deviceId != null) {
        await notifier.applyFilters(LogFilters(deviceId: widget.deviceId));
      } else {
        await notifier.fetchLogs(reset: true);
      }
      await notifier.fetchSources();
      await ref.read(monitorsProvider.notifier).fetchReferenceCatalogs();
      await ref.read(coreSettingsProvider.notifier).fetchOperatingSystems();
    });
  }

  @override
  void dispose() {
    _searchCtrl.dispose();
    super.dispose();
  }

  Future<void> _openSourcesSheet() async {
    await ref.read(logsProvider.notifier).fetchSources();
    if (!mounted) return;

    await showAdaptiveFormSheet<void>(
      context: context,
      title: 'Origens de Syslog',
      subtitle: 'Vincule IPs ou hostnames desconhecidos aos dispositivos cadastrados.',
      maxWidth: 680,
      builder: (sheetCtx) {
        return Consumer(
          builder: (context, ref, _) {
            final logs = ref.watch(logsProvider);
            final devices = ref.watch(monitorsProvider).devices;
            final auth = ref.watch(authProvider);

            if (logs.sources.isEmpty) {
              return const EmptyStateView(
                title: 'Nenhuma origem recebida ainda',
                description: 'Configure o envio de syslog em um equipamento para visualizar aqui.',
                icon: Icons.router_outlined,
              );
            }

            return Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                for (final src in logs.sources) ...<Widget>[
                  NestedPanel(
                    tone: src.kind == 'unknown' ? SemanticTone.warning : null,
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: <Widget>[
                        Wrap(
                          spacing: 8,
                          runSpacing: 4,
                          crossAxisAlignment: WrapCrossAlignment.center,
                          children: <Widget>[
                            Text(
                              src.hostname != null && src.hostname!.isNotEmpty
                                  ? '${src.hostname} (${src.sourceIp})'
                                  : src.sourceIp,
                              style: const TextStyle(
                                fontFamily: 'monospace',
                                fontWeight: FontWeight.w700,
                              ),
                            ),
                            SemanticChip(
                              label: src.kind,
                              tone: src.kind == 'unknown'
                                  ? SemanticTone.warning
                                  : (src.kind == 'device'
                                        ? SemanticTone.success
                                        : SemanticTone.info),
                              compact: true,
                            ),
                            if (src.masked)
                              const SemanticChip(
                                label: 'NAT mascarado',
                                tone: SemanticTone.caution,
                                compact: true,
                              ),
                            if (src.droppedCount > 0)
                              SemanticChip(
                                label: '${src.droppedCount} descartadas',
                                tone: SemanticTone.error,
                                compact: true,
                              ),
                          ],
                        ),
                        if (src.lastMessage != null &&
                            src.lastMessage!.isNotEmpty) ...<Widget>[
                          const SizedBox(height: 6),
                          Text(
                            src.lastMessage!,
                            maxLines: 2,
                            overflow: TextOverflow.ellipsis,
                            style: context.textTheme.bodySmall?.copyWith(
                              fontFamily: 'monospace',
                            ),
                          ),
                        ],
                        if (auth.canWrite) ...<Widget>[
                          const SizedBox(height: 10),
                          DropdownButtonFormField<int?>(
                            initialValue: src.deviceId,
                            decoration: const InputDecoration(
                              labelText: 'Vincular ao dispositivo',
                              isDense: true,
                            ),
                            items: <DropdownMenuItem<int?>>[
                              const DropdownMenuItem<int?>(
                                value: null,
                                child: Text('Sem vínculo'),
                              ),
                              for (final d in devices)
                                DropdownMenuItem<int?>(
                                  value: d.id,
                                  child: Text(d.name),
                                ),
                            ],
                            onChanged: (devId) async {
                              try {
                                await ref
                                    .read(logsProvider.notifier)
                                    .bindSource(src.bindKey, devId);
                              } catch (err) {
                                if (!context.mounted) return;
                                ScaffoldMessenger.of(context).showSnackBar(
                                  SnackBar(content: Text(err.toString())),
                                );
                              }
                            },
                          ),
                        ],
                      ],
                    ),
                  ),
                  const SizedBox(height: 10),
                ],
              ],
            );
          },
        );
      },
    );
  }

  Future<void> _openSetupGuideSheet() async {
    await ref.read(coreSettingsProvider.notifier).fetchServerAddresses();
    await ref.read(logsProvider.notifier).fetchSetupGuide();
    if (!mounted) return;

    String? selectedAddress;

    await showAdaptiveFormSheet<void>(
      context: context,
      title: 'Configurar envio de Syslog',
      subtitle:
          'Comandos prontos por sistema operacional ou ativação automática.',
      maxWidth: 700,
      builder: (sheetCtx) {
        return StatefulBuilder(
          builder: (context, setModalState) {
            final logs = ref.watch(logsProvider);
            final settings = ref.watch(coreSettingsProvider);
            final auth = ref.watch(authProvider);
            final guide = logs.setupGuide;

            return Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                if (settings.usableAddresses.isNotEmpty) ...<Widget>[
                  DropdownButtonFormField<String?>(
                    initialValue: selectedAddress,
                    decoration: const InputDecoration(
                      labelText: 'Endereço deste servidor nos comandos',
                    ),
                    items: <DropdownMenuItem<String?>>[
                      const DropdownMenuItem<String?>(
                        value: null,
                        child: Text('Padrão do servidor'),
                      ),
                      for (final addr in settings.usableAddresses)
                        DropdownMenuItem<String?>(
                          value: addr.value,
                          child: Text('${addr.label} (${addr.value})'),
                        ),
                    ],
                    onChanged: (v) async {
                      setModalState(() => selectedAddress = v);
                      await ref
                          .read(logsProvider.notifier)
                          .fetchSetupGuide(address: v);
                    },
                  ),
                  const SizedBox(height: 12),
                ],
                if (auth.canWrite) ...<Widget>[
                  OutlinedButton.icon(
                    onPressed: () {
                      Navigator.of(sheetCtx).pop();
                      _openAutoProvisionSheet();
                    },
                    icon: const Icon(Icons.auto_fix_high, size: 18),
                    label: const Text(
                      'Ativar syslog automaticamente via SSH/Telnet',
                    ),
                  ),
                  const SizedBox(height: 14),
                ],
                if (guide == null)
                  const LoadingStateView(
                    message: 'Carregando guia de configuração...',
                  )
                else ...<Widget>[
                  KeyValueRow(
                    label: 'Destino Syslog',
                    value: '${guide.serverAddress}:${guide.port}',
                    monospace: true,
                    copyable: true,
                  ),
                  const SizedBox(height: 10),
                  for (final snippet in guide.snippets) ...<Widget>[
                    CodeViewer(title: snippet.label, code: snippet.commands),
                    if (snippet.notes != null &&
                        snippet.notes!.isNotEmpty) ...<Widget>[
                      const SizedBox(height: 4),
                      Text(
                        snippet.notes!,
                        style: context.textTheme.bodySmall?.copyWith(
                          color: context.colorScheme.onSurfaceVariant,
                        ),
                      ),
                    ],
                    const SizedBox(height: 12),
                  ],
                ],
              ],
            );
          },
        );
      },
    );
  }

  Future<void> _openAutoProvisionSheet() async {
    final devices = ref.read(monitorsProvider).devices;
    final systems = ref.read(coreSettingsProvider).operatingSystems;
    if (devices.isEmpty) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text(
            'Cadastre ao menos um dispositivo antes de provisionar.',
          ),
        ),
      );
      return;
    }

    int selectedDeviceId = widget.deviceId ?? devices.first.id;
    String protocol = 'ssh';
    String? osId;
    bool useSaved = false;
    bool saveCred = false;
    bool loadingHints = false;
    bool running = false;
    ProvisionHintsResponse? hints;
    ProvisionLoggingResponse? result;
    String? error;

    final portCtrl = TextEditingController(text: '22');
    final userCtrl = TextEditingController();
    final passCtrl = TextEditingController();
    final addrCtrl = TextEditingController();
    final macCtrl = TextEditingController();

    Future<void> loadHints(StateSetter setModalState, int devId) async {
      setModalState(() => loadingHints = true);
      final h = await ref
          .read(logsProvider.notifier)
          .fetchProvisionHints(devId);
      setModalState(() {
        hints = h;
        loadingHints = false;
        if (h != null) {
          if (h.serverAddress != null) addrCtrl.text = h.serverAddress!;
          if (h.macAddress != null) macCtrl.text = h.macAddress!;
          if (h.operatingSystem != null) osId = h.operatingSystem;
          if (h.sshOpen) {
            protocol = 'ssh';
            portCtrl.text = '22';
          } else if (h.telnetOpen) {
            protocol = 'telnet';
            portCtrl.text = '23';
          }
          useSaved = h.savedCredentials.contains(protocol);
        }
      });
    }

    await showAdaptiveFormSheet<void>(
      context: context,
      title: 'Ativação automática de Syslog',
      subtitle: 'A senha nunca é gravada nem exibida na transcrição (exceto se salvar credencial).',
      maxWidth: 640,
      builder: (sheetCtx) {
        return StatefulBuilder(
          builder: (context, setModalState) {
            if (hints == null && !loadingHints) {
              Future<void>.microtask(
                () => loadHints(setModalState, selectedDeviceId),
              );
            }
            return Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                DropdownButtonFormField<int>(
                  initialValue: selectedDeviceId,
                  decoration: const InputDecoration(labelText: 'Dispositivo'),
                  items: <DropdownMenuItem<int>>[
                    for (final d in devices)
                      DropdownMenuItem<int>(value: d.id, child: Text(d.name)),
                  ],
                  onChanged: (v) {
                    if (v != null) {
                      setModalState(() => selectedDeviceId = v);
                      loadHints(setModalState, v);
                    }
                  },
                ),
                const SizedBox(height: 12),
                Row(
                  children: <Widget>[
                    Expanded(
                      child: DropdownButtonFormField<String>(
                        initialValue: protocol,
                        decoration: const InputDecoration(
                          labelText: 'Protocolo',
                        ),
                        items: <DropdownMenuItem<String>>[
                          const DropdownMenuItem(
                            value: 'ssh',
                            child: Text('SSH'),
                          ),
                          const DropdownMenuItem(
                            value: 'telnet',
                            child: Text('Telnet'),
                          ),
                          if (hints?.layer2Reachable == true)
                            const DropdownMenuItem(
                              value: 'mac_telnet',
                              child: Text('MAC-Telnet'),
                            ),
                        ],
                        onChanged: (v) {
                          if (v != null) {
                            setModalState(() {
                              protocol = v;
                              portCtrl.text = v == 'telnet' ? '23' : '22';
                            });
                          }
                        },
                      ),
                    ),
                    const SizedBox(width: 10),
                    Expanded(
                      child: TextField(
                        controller: portCtrl,
                        keyboardType: TextInputType.number,
                        decoration: const InputDecoration(labelText: 'Porta'),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 12),
                if (systems.isNotEmpty) ...<Widget>[
                  DropdownButtonFormField<String?>(
                    initialValue: systems.any((s) => s.id == osId)
                        ? osId
                        : null,
                    decoration: const InputDecoration(
                      labelText: 'Sistema operacional',
                    ),
                    items: <DropdownMenuItem<String?>>[
                      const DropdownMenuItem<String?>(
                        value: null,
                        child: Text('Detectar automaticamente'),
                      ),
                      for (final s in systems.where((s) => s.supportsSyslog))
                        DropdownMenuItem<String?>(
                          value: s.id,
                          child: Text(s.label),
                        ),
                    ],
                    onChanged: (v) => setModalState(() => osId = v),
                  ),
                  const SizedBox(height: 12),
                ],
                TextField(
                  controller: addrCtrl,
                  decoration: const InputDecoration(
                    labelText:
                        'Endereço deste servidor (que o equipamento alcança)',
                  ),
                ),
                const SizedBox(height: 12),
                if (hints?.savedCredentials.isNotEmpty == true)
                  SwitchListTile(
                    contentPadding: EdgeInsets.zero,
                    value: useSaved,
                    onChanged: (v) => setModalState(() => useSaved = v),
                    title: const Text('Usar credencial salva do equipamento'),
                  ),
                if (!useSaved) ...<Widget>[
                  TextField(
                    controller: userCtrl,
                    decoration: const InputDecoration(labelText: 'Usuário'),
                  ),
                  const SizedBox(height: 10),
                  TextField(
                    controller: passCtrl,
                    obscureText: true,
                    decoration: const InputDecoration(labelText: 'Senha'),
                  ),
                  CheckboxListTile(
                    contentPadding: EdgeInsets.zero,
                    controlAffinity: ListTileControlAffinity.leading,
                    value: saveCred,
                    onChanged: (v) =>
                        setModalState(() => saveCred = v ?? false),
                    title: const Text(
                      'Salvar credencial no cofre do equipamento',
                    ),
                  ),
                ],
                if (error != null) ...<Widget>[
                  const SizedBox(height: 8),
                  NestedPanel(tone: SemanticTone.error, child: Text(error!)),
                ],
                if (result != null) ...<Widget>[
                  const SizedBox(height: 12),
                  NestedPanel(
                    tone: result!.confirmed == true
                        ? SemanticTone.success
                        : SemanticTone.info,
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Text(
                          result!.confirmed == true
                              ? 'Syslog configurado e confirmado!'
                              : 'Comandos aplicados com sucesso.',
                          style: const TextStyle(fontWeight: FontWeight.w700),
                        ),
                        if (result!.persistenceWarning != null) ...<Widget>[
                          const SizedBox(height: 4),
                          Text(result!.persistenceWarning!),
                        ],
                      ],
                    ),
                  ),
                  if (result!.transcript.isNotEmpty) ...<Widget>[
                    const SizedBox(height: 10),
                    CodeViewer(
                      title: 'Transcrição',
                      code: result!.transcript,
                      maxHeight: 220,
                    ),
                  ],
                ],
                const SizedBox(height: 16),
                FilledButton.icon(
                  onPressed: running
                      ? null
                      : () async {
                          setModalState(() {
                            running = true;
                            error = null;
                          });
                          try {
                            final res = await ref
                                .read(logsProvider.notifier)
                                .provisionDevice(selectedDeviceId, <
                                  String,
                                  dynamic
                                >{
                                  'protocol': protocol,
                                  'port': int.tryParse(portCtrl.text.trim()),
                                  'username': userCtrl.text.trim(),
                                  'password': passCtrl.text,
                                  'savedCredential': useSaved ? protocol : null,
                                  'saveCredential': !useSaved && saveCred,
                                  'operatingSystem': osId,
                                  'serverAddress': addrCtrl.text.trim().isEmpty
                                      ? null
                                      : addrCtrl.text.trim(),
                                  'macAddress': macCtrl.text.trim().isEmpty
                                      ? null
                                      : macCtrl.text.trim(),
                                });
                            setModalState(() {
                              running = false;
                              result = res;
                            });
                          } catch (err) {
                            setModalState(() {
                              running = false;
                              error = err.toString();
                            });
                          }
                        },
                  icon: running
                      ? const SizedBox(
                          width: 18,
                          height: 18,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        )
                      : const Icon(Icons.play_arrow),
                  label: const Text('Aplicar configuração'),
                ),
              ],
            );
          },
        );
      },
    );

    // Segurança: descarta a senha em memória ao fechar o modal.
    passCtrl.clear();
    portCtrl.dispose();
    userCtrl.dispose();
    passCtrl.dispose();
    addrCtrl.dispose();
    macCtrl.dispose();
  }

  Future<void> _openTemplateSheet(LogEntry entry, LogTemplateInfo info) async {
    final auth = ref.read(authProvider);
    var selectedCategory = info.category ?? 'system';

    await showAdaptiveFormSheet<void>(
      context: context,
      title: 'Classificação do padrão de log',
      subtitle: info.confirmed
          ? 'Confirmada pelo operador'
          : 'Sugestão automática do Decisor',
      builder: (sheetCtx) {
        return StatefulBuilder(
          builder: (context, setModalState) {
            return Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                CodeViewer(title: 'Padrão', code: info.template),
                const SizedBox(height: 12),
                DropdownButtonFormField<String>(
                  initialValue:
                      kLogCategories.any((c) => c.$1 == selectedCategory)
                      ? selectedCategory
                      : 'system',
                  decoration: const InputDecoration(labelText: 'Categoria'),
                  items: <DropdownMenuItem<String>>[
                    for (final (k, label) in kLogCategories)
                      DropdownMenuItem<String>(value: k, child: Text(label)),
                  ],
                  onChanged: auth.canWrite
                      ? (v) {
                          if (v != null) {
                            setModalState(() => selectedCategory = v);
                          }
                        }
                      : null,
                ),
                const SizedBox(height: 14),
                Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  children: <Widget>[
                    if (auth.canWrite)
                      FilledButton.icon(
                        onPressed: () async {
                          final err = await ref
                              .read(logsProvider.notifier)
                              .confirmTemplate(
                                info.templateHash,
                                selectedCategory,
                              );
                          if (!sheetCtx.mounted) return;
                          Navigator.of(sheetCtx).pop();
                          ScaffoldMessenger.of(context).showSnackBar(
                            SnackBar(
                              content: Text(err ?? 'Categoria confirmada.'),
                            ),
                          );
                        },
                        icon: const Icon(Icons.check, size: 18),
                        label: const Text('Confirmar categoria'),
                      ),
                    if (info.alertTemplate != null &&
                        info.alertTemplate!.isNotEmpty)
                      OutlinedButton.icon(
                        onPressed: () {
                          Navigator.of(sheetCtx).pop();
                          context.go(
                            '/alerts?tab=rules&catalog=${info.alertTemplate}',
                          );
                        },
                        icon: const Icon(
                          Icons.notifications_active_outlined,
                          size: 18,
                        ),
                        label: const Text('Criar regra de alerta'),
                      ),
                  ],
                ),
              ],
            );
          },
        );
      },
    );
  }

  Future<void> _exportLogs(String format) async {
    try {
      final result = await ref
          .read(logsProvider.notifier)
          .exportLogs(format: format);
      final text = utf8.decode(result.bytes, allowMalformed: true);
      await Clipboard.setData(ClipboardData(text: text));
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            'Exportação (${format.toUpperCase()}, ${formatBytes(result.bytes.length)}) copiada para a área de transferência.',
          ),
        ),
      );
    } catch (err) {
      if (!mounted) return;
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text(err.toString())));
    }
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(logsProvider);
    final devices = ref.watch(monitorsProvider).devices;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        SearchFilterHeader(
          title: widget.embedded ? null : 'Logs Syslog',
          subtitle: widget.embedded ? null : 'Busca cursor-paginada, classificação de padrões e tail em tempo real',
          searchController: _searchCtrl,
          searchHint: 'Buscar mensagem de log...',
          onSearchChanged: (v) {
            ref
                .read(logsProvider.notifier)
                .applyFilters(state.filters.copyWith(search: v));
          },
          actions: <Widget>[
            FilledButton.tonalIcon(
              onPressed: () => ref.read(logsProvider.notifier).toggleTail(),
              icon: Icon(
                state.tailing
                    ? Icons.stop_circle_outlined
                    : Icons.radio_button_checked,
                size: 18,
              ),
              label: Text(state.tailing ? 'Parar Ao Vivo' : 'Ao Vivo'),
            ),
            if (!widget.embedded)
              OutlinedButton.icon(
                onPressed: _openSourcesSheet,
                icon: const Icon(Icons.router_outlined, size: 18),
                label: Text(
                  state.unknownCount > 0
                      ? 'Origens (${state.unknownCount})'
                      : 'Origens',
                ),
              ),
            OutlinedButton.icon(
              onPressed: _openSetupGuideSheet,
              icon: const Icon(Icons.terminal, size: 18),
              label: const Text('Configurar Syslog'),
            ),
            PopupMenuButton<String>(
              tooltip: 'Exportar logs',
              icon: const Icon(Icons.download_outlined),
              onSelected: _exportLogs,
              itemBuilder: (_) => const <PopupMenuEntry<String>>[
                PopupMenuItem<String>(
                  value: 'txt',
                  child: Text('Exportar TXT'),
                ),
                PopupMenuItem<String>(
                  value: 'csv',
                  child: Text('Exportar CSV'),
                ),
              ],
            ),
          ],
          filters: <Widget>[
            if (!widget.embedded && devices.isNotEmpty)
              DropdownButton<int?>(
                value: state.filters.deviceId,
                hint: const Text('Dispositivo'),
                underline: const SizedBox.shrink(),
                items: <DropdownMenuItem<int?>>[
                  const DropdownMenuItem<int?>(
                    value: null,
                    child: Text('Todos dispositivos'),
                  ),
                  for (final d in devices)
                    DropdownMenuItem<int?>(value: d.id, child: Text(d.name)),
                ],
                onChanged: (v) => ref
                    .read(logsProvider.notifier)
                    .applyFilters(
                      v == null
                          ? state.filters.copyWith(clearDevice: true)
                          : state.filters.copyWith(deviceId: v),
                    ),
              ),
            DropdownButton<int?>(
              value: state.filters.severity,
              hint: const Text('Severidade'),
              underline: const SizedBox.shrink(),
              items: <DropdownMenuItem<int?>>[
                const DropdownMenuItem<int?>(
                  value: null,
                  child: Text('Todas severidades'),
                ),
                for (final (v, label) in kSyslogSeverityOptions)
                  DropdownMenuItem<int?>(value: v, child: Text(label)),
              ],
              onChanged: (v) => ref
                  .read(logsProvider.notifier)
                  .applyFilters(
                    v == null
                        ? state.filters.copyWith(clearSeverity: true)
                        : state.filters.copyWith(severity: v),
                  ),
            ),
            DropdownButton<String?>(
              value: state.filters.category,
              hint: const Text('Categoria'),
              underline: const SizedBox.shrink(),
              items: <DropdownMenuItem<String?>>[
                const DropdownMenuItem<String?>(
                  value: null,
                  child: Text('Todas categorias'),
                ),
                for (final (k, label) in kLogCategories)
                  DropdownMenuItem<String?>(value: k, child: Text(label)),
              ],
              onChanged: (v) => ref
                  .read(logsProvider.notifier)
                  .applyFilters(
                    v == null
                        ? state.filters.copyWith(clearCategory: true)
                        : state.filters.copyWith(category: v),
                  ),
            ),
            DropdownButton<int>(
              value: state.filters.hours ?? 24,
              underline: const SizedBox.shrink(),
              items: <DropdownMenuItem<int>>[
                for (final (h, label) in kSyslogWindowOptions)
                  DropdownMenuItem<int>(value: h, child: Text(label)),
              ],
              onChanged: (v) {
                if (v != null) {
                  ref
                      .read(logsProvider.notifier)
                      .applyFilters(state.filters.copyWith(hours: v));
                }
              },
            ),
            TextButton(
              onPressed: () {
                _searchCtrl.clear();
                ref.read(logsProvider.notifier).clearFilters();
              },
              child: const Text('Limpar'),
            ),
          ],
        ),
        const SizedBox(height: 10),
        if (!widget.embedded && state.unknownCount > 0) ...<Widget>[
          NestedPanel(
            tone: SemanticTone.warning,
            child: Row(
              children: <Widget>[
                const Icon(Icons.warning_amber_rounded),
                const SizedBox(width: 10),
                Expanded(
                  child: Text(
                    '${countNoun(state.unknownCount, 'origem desconhecida', 'origens desconhecidas')} enviando syslog sem vínculo (mensagens descartadas).',
                    style: const TextStyle(fontWeight: FontWeight.w600),
                  ),
                ),
                TextButton(
                  onPressed: _openSourcesSheet,
                  child: const Text('Vincular'),
                ),
              ],
            ),
          ),
          const SizedBox(height: 8),
        ],
        if (!widget.embedded && state.natMasking) ...<Widget>[
          NestedPanel(
            tone: SemanticTone.caution,
            child: Text(
              state.nat?.explanation ?? 'NAT mascarando IPs de origem: vincule cada equipamento pelo hostname em Origens.',
              style: context.textTheme.bodySmall?.copyWith(
                fontWeight: FontWeight.w600,
              ),
            ),
          ),
          const SizedBox(height: 8),
        ],
        Expanded(
          child: state.loading && state.entries.isEmpty
              ? const LoadingStateView(message: 'Carregando logs...')
              : state.error != null && state.entries.isEmpty
              ? ErrorStateView(
                  message: state.error!,
                  onRetry: () =>
                      ref.read(logsProvider.notifier).fetchLogs(reset: true),
                )
              : state.entries.isEmpty
              ? const EmptyStateView(
                  title: 'Nenhum log encontrado',
                  description: 'Ajuste os filtros ou ative o envio de syslog nos equipamentos.',
                  icon: Icons.article_outlined,
                )
              : RefreshIndicator(
                  onRefresh: () =>
                      ref.read(logsProvider.notifier).fetchLogs(reset: true),
                  child: ListView.builder(
                    itemCount: state.entries.length + (state.hasMore ? 1 : 0),
                    itemBuilder: (context, index) {
                      if (index >= state.entries.length) {
                        return Padding(
                          padding: const EdgeInsets.symmetric(vertical: 12),
                          child: OutlinedButton(
                            onPressed: state.loadingMore
                                ? null
                                : () => ref
                                      .read(logsProvider.notifier)
                                      .fetchLogs(reset: false),
                            child: Text(
                              state.loadingMore
                                  ? 'Carregando...'
                                  : 'Carregar mais logs',
                            ),
                          ),
                        );
                      }
                      final entry = state.entries[index];
                      final tmpl = state.templateOf(entry);
                      return Padding(
                        padding: const EdgeInsets.only(bottom: 6),
                        child: NestedPanel(
                          padding: const EdgeInsets.all(10),
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: <Widget>[
                              Wrap(
                                spacing: 6,
                                runSpacing: 4,
                                crossAxisAlignment: WrapCrossAlignment.center,
                                children: <Widget>[
                                  SemanticChip(
                                    label:
                                        (entry.severityLabel ??
                                                'SEV ${entry.severity ?? '-'}')
                                            .toUpperCase(),
                                    tone: entry.severityTone,
                                    compact: true,
                                  ),
                                  Text(
                                    formatShortDateTime(entry.receivedAt),
                                    style: context.textTheme.labelSmall
                                        ?.copyWith(
                                          fontFamily: 'monospace',
                                          color: context
                                              .colorScheme
                                              .onSurfaceVariant,
                                        ),
                                  ),
                                  Text(
                                    entry.deviceName ??
                                        entry.hostname ??
                                        entry.sourceIp,
                                    style: context.textTheme.labelMedium
                                        ?.copyWith(fontWeight: FontWeight.w700),
                                  ),
                                  if (entry.appName != null &&
                                      entry.appName!.isNotEmpty)
                                    SemanticChip(
                                      label: entry.appName!,
                                      tone: SemanticTone.secondary,
                                      compact: true,
                                    ),
                                  if (tmpl != null)
                                    SemanticChip(
                                      label: logCategoryLabel(tmpl.category),
                                      tone: tmpl.confirmed
                                          ? SemanticTone.primary
                                          : SemanticTone.info,
                                      compact: true,
                                      onTap: () =>
                                          _openTemplateSheet(entry, tmpl),
                                    ),
                                ],
                              ),
                              const SizedBox(height: 6),
                              SelectableText(
                                entry.message,
                                style: TextStyle(
                                  fontFamily: 'monospace',
                                  fontSize: 12.5,
                                  color: context.colorScheme.onSurface,
                                ),
                              ),
                            ],
                          ),
                        ),
                      );
                    },
                  ),
                ),
        ),
      ],
    );
  }
}
