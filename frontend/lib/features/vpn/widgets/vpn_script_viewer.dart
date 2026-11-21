import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/features/vpn/models/vpn_models.dart';
import 'package:netmonitor_app/features/vpn/providers/vpn_provider.dart';
import 'package:netmonitor_app/features/vpn/utils/file_export_helper.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';
import 'package:qr_flutter/qr_flutter.dart';

/// Abre o visualizador de artefato VPN (cofre de uso único da chave privada).
/// Limpa `vpnProvider.lastArtifact` imediatamente ao fechar.
Future<void> showVpnScriptViewer({
  required BuildContext context,
  required WidgetRef ref,
  required VpnArtifact artifact,
  int? peerId,
  String? deviceName,
}) async {
  try {
    await showAdaptiveFormSheet<void>(
      context: context,
      title: artifact.hasPrivateKey
          ? 'Configuração WireGuard${deviceName != null ? ' · $deviceName' : ''}'
          : 'Chave privada já entregue',
      subtitle: artifact.label,
      maxWidth: 720,
      builder: (_) => _VpnScriptViewerBody(
        initialArtifact: artifact,
        peerId: peerId ?? artifact.peerId,
      ),
    );
  } finally {
    ref.read(vpnProvider.notifier).clearLastArtifact();
  }
}

class _VpnScriptViewerBody extends ConsumerStatefulWidget {
  const _VpnScriptViewerBody({required this.initialArtifact, this.peerId});

  final VpnArtifact initialArtifact;
  final int? peerId;

  @override
  ConsumerState<_VpnScriptViewerBody> createState() =>
      _VpnScriptViewerBodyState();
}

class _VpnScriptViewerBodyState extends ConsumerState<_VpnScriptViewerBody> {
  late VpnArtifact _artifact;
  late String _selectedDocId;
  late bool _showSummary;
  late List<String> _agentAllow;
  late TextEditingController _dbTargetsCtrl;
  bool _updatingPolicy = false;
  bool _rotating = false;

  @override
  void initState() {
    super.initState();
    _artifact = widget.initialArtifact;
    _selectedDocId = _defaultDocId(_artifact);
    _showSummary = !_artifact.hasPrivateKey || !_artifact.supportsQrCode;
    _agentAllow = List<String>.from(
      _artifact.agentInstall?.policy.allow ?? const <String>[],
    );
    _dbTargetsCtrl = TextEditingController(
      text: _artifact.agentInstall?.policy.databaseTargets ?? '',
    );
  }

  @override
  void dispose() {
    _dbTargetsCtrl.dispose();
    super.dispose();
  }

  String _defaultDocId(VpnArtifact artifact) {
    if (artifact.language == 'ini' && artifact.variants.isNotEmpty) {
      return artifact.variants.first.id;
    }
    return 'base';
  }

  ({
    String id,
    String label,
    String? hint,
    String fileName,
    String content,
    List<String> instructions,
  })
  _currentDoc() {
    if (_selectedDocId != 'base') {
      for (final variant in _artifact.variants) {
        if (variant.id == _selectedDocId) {
          return (
            id: variant.id,
            label: variant.label,
            hint: variant.hint,
            fileName: variant.fileName,
            content: variant.content,
            instructions: variant.instructions,
          );
        }
      }
    }
    return (
      id: 'base',
      label: _artifact.language == 'ini'
          ? 'Configuração WireGuard'
          : 'Script padrão',
      hint: null,
      fileName: _artifact.fileName,
      content: _artifact.content,
      instructions: _artifact.instructions,
    );
  }

  Future<void> _applyPolicyUpdate() async {
    final effectivePeerId = widget.peerId ?? _artifact.peerId;
    final agentInstall = _artifact.agentInstall;
    if (effectivePeerId == null || agentInstall == null) return;
    setState(() => _updatingPolicy = true);
    try {
      final policy = VpnInstallPolicy(
        allow: List<String>.unmodifiable(_agentAllow),
        databaseTargets: _dbTargetsCtrl.text.trim(),
      );
      final ok = await ref
          .read(vpnProvider.notifier)
          .updatePeerAgentPolicy(
            peerId: effectivePeerId,
            enrollCode: agentInstall.enrollCode,
            policy: policy,
            confContent: _artifact.content,
          );
      if (!mounted) return;
      final refreshed = ref.read(vpnProvider).lastArtifact;
      if (ok && refreshed != null) {
        setState(() {
          _artifact = refreshed;
        });
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text('Script atualizado com as novas permissões.'),
          ),
        );
      }
    } finally {
      if (mounted) setState(() => _updatingPolicy = false);
    }
  }

  Future<void> _rotateKeys() async {
    final effectivePeerId = widget.peerId ?? _artifact.peerId;
    if (effectivePeerId == null) return;
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: 'Gerar novas chaves?',
      message:
          'A chave anterior será invalidada imediatamente. O dispositivo perderá o túnel até receber a nova configuração.',
      confirmLabel: 'Gerar novas chaves',
      tone: SemanticTone.warning,
      icon: Icons.key_outlined,
    );
    if (!confirmed || !mounted) return;
    setState(() => _rotating = true);
    try {
      final rotated = await ref
          .read(vpnProvider.notifier)
          .rotateKeys(
            effectivePeerId,
            agentPolicy: _artifact.agentInstall != null
                ? VpnInstallPolicy(
                    allow: _agentAllow,
                    databaseTargets: _dbTargetsCtrl.text.trim(),
                  )
                : null,
          );
      if (rotated != null && mounted) {
        setState(() {
          _artifact = rotated;
          _selectedDocId = _defaultDocId(rotated);
          _showSummary = !rotated.supportsQrCode;
        });
      }
    } finally {
      if (mounted) setState(() => _rotating = false);
    }
  }

  Future<void> _downloadCurrent() async {
    final doc = _currentDoc();
    final bytes = Uint8List.fromList(utf8.encode(doc.content));
    await exportFileBytes(
      bytes: bytes,
      fileName: doc.fileName,
      mimeType: 'text/plain; charset=utf-8',
    );
    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text('Arquivo ${doc.fileName} preparado.')),
    );
  }

  void _copyCurrent() {
    final doc = _currentDoc();
    Clipboard.setData(ClipboardData(text: doc.content));
    ScaffoldMessenger.of(context).showSnackBar(
      const SnackBar(
        content: Text('Configuração copiada para a área de transferência.'),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final hasKey = _artifact.hasPrivateKey;
    final scheme = context.colorScheme;
    final doc = _currentDoc();

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        if (hasKey)
          NestedPanel(
            tone: SemanticTone.warning,
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Icon(
                  Icons.lock_clock_outlined,
                  color: context.semanticColors
                      .resolve(SemanticTone.warning, scheme)
                      .onContainer,
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: Text(
                    'A chave privada aparece uma única vez e nunca é gravada no banco. '
                    'Salve o arquivo, copie ou leia o QR code agora; depois só gerando novas chaves.',
                    style: context.textTheme.bodySmall?.copyWith(
                      fontWeight: FontWeight.w600,
                      color: context.semanticColors
                          .resolve(SemanticTone.warning, scheme)
                          .onContainer,
                    ),
                  ),
                ),
              ],
            ),
          )
        else ...<Widget>[
          NestedPanel(
            tone: SemanticTone.info,
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Icon(
                  Icons.verified_user_outlined,
                  color: context.semanticColors
                      .resolve(SemanticTone.info, scheme)
                      .onContainer,
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: Text(
                    'A chave privada já foi entregue e removida do cofre em memória. '
                    'Para configurar o dispositivo novamente, gere um novo par de chaves.',
                    style: context.textTheme.bodySmall?.copyWith(
                      fontWeight: FontWeight.w600,
                      color: context.semanticColors
                          .resolve(SemanticTone.info, scheme)
                          .onContainer,
                    ),
                  ),
                ),
              ],
            ),
          ),
          if (_artifact.supportsQrCode) ...<Widget>[
            const SizedBox(height: 10),
            NestedPanel(
              tone: SemanticTone.warning,
              child: Text(
                'O QR code não pode ser exibido sem a chave privada. Gere novas chaves para obter um novo QR code.',
                style: context.textTheme.bodySmall?.copyWith(
                  color: context.semanticColors
                      .resolve(SemanticTone.warning, scheme)
                      .onContainer,
                ),
              ),
            ),
          ],
        ],
        if (hasKey && _artifact.supportsQrCode) ...<Widget>[
          const SizedBox(height: 16),
          Center(
            child: Container(
              padding: const EdgeInsets.all(16),
              decoration: BoxDecoration(
                color: Colors.white,
                borderRadius: AppTheme.borderRadius,
                border: Border.all(color: scheme.outlineVariant),
              ),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  QrImageView(
                    data: _artifact.content,
                    version: QrVersions.auto,
                    size: 220,
                    backgroundColor: Colors.white,
                    eyeStyle: const QrEyeStyle(
                      eyeShape: QrEyeShape.square,
                      color: Colors.black,
                    ),
                    dataModuleStyle: const QrDataModuleStyle(
                      dataModuleShape: QrDataModuleShape.square,
                      color: Colors.black,
                    ),
                  ),
                  const SizedBox(height: 8),
                  const Text(
                    'Leia com o aplicativo WireGuard',
                    style: TextStyle(
                      color: Color(0xFF0F172A),
                      fontWeight: FontWeight.w600,
                      fontSize: 12,
                    ),
                  ),
                ],
              ),
            ),
          ),
        ],
        if (_artifact.summary.isNotEmpty) ...<Widget>[
          const SizedBox(height: 16),
          NestedPanel(
            padding: EdgeInsets.zero,
            child: Material(
              type: MaterialType.transparency,
              child: ExpansionTile(
                initiallyExpanded: _showSummary,
                onExpansionChanged: (open) =>
                    setState(() => _showSummary = open),
                title: Text(
                  'Dados do túnel',
                  style: context.textTheme.titleSmall?.copyWith(
                    fontWeight: FontWeight.w700,
                  ),
                ),
                childrenPadding: const EdgeInsets.fromLTRB(14, 0, 14, 12),
                children: <Widget>[
                  for (final item in _artifact.summary)
                    KeyValueRow(
                      label: item.label,
                      value: item.value,
                      monospace: true,
                      copyable: true,
                    ),
                ],
              ),
            ),
          ),
        ],
        if (hasKey &&
            _artifact.agentInstall != null &&
            (widget.peerId ?? _artifact.peerId) != null) ...<Widget>[
          const SizedBox(height: 16),
          NestedPanel(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Text(
                  'Permissões do agente no script',
                  style: context.textTheme.titleSmall?.copyWith(
                    fontWeight: FontWeight.w700,
                  ),
                ),
                const SizedBox(height: 4),
                Text(
                  'Ajuste o que o agente remoto poderá executar. Reestampa o script sem consumir a chave privada.',
                  style: context.textTheme.bodySmall?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
                const SizedBox(height: 8),
                Wrap(
                  spacing: 8,
                  runSpacing: 6,
                  children: vpnAgentPermissions
                      .map((perm) {
                        final selected = _agentAllow.contains(perm.id);
                        return FilterChip(
                          label: Text(perm.label),
                          selected: selected,
                          onSelected: (value) {
                            setState(() {
                              if (value) {
                                _agentAllow.add(perm.id);
                              } else {
                                _agentAllow.remove(perm.id);
                              }
                            });
                          },
                        );
                      })
                      .toList(growable: false),
                ),
                if (_agentAllow.contains('database')) ...<Widget>[
                  const SizedBox(height: 10),
                  TextField(
                    controller: _dbTargetsCtrl,
                    decoration: const InputDecoration(
                      labelText: 'Destinos de banco permitidos (host:porta)',
                      hintText: '10.0.0.10:5432, 10.0.0.11:3306',
                    ),
                  ),
                ],
                const SizedBox(height: 10),
                Align(
                  alignment: Alignment.centerRight,
                  child: FilledButton.tonalIcon(
                    onPressed: _updatingPolicy ? null : _applyPolicyUpdate,
                    icon: _updatingPolicy
                        ? const SizedBox(
                            width: 16,
                            height: 16,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          )
                        : const Icon(Icons.refresh, size: 18),
                    label: const Text('Atualizar script'),
                  ),
                ),
              ],
            ),
          ),
        ],
        if (hasKey) ...<Widget>[
          const SizedBox(height: 16),
          if (_artifact.variants.isNotEmpty) ...<Widget>[
            SingleChildScrollView(
              scrollDirection: Axis.horizontal,
              child: SegmentedButton<String>(
                showSelectedIcon: false,
                style: SegmentedButton.styleFrom(
                  visualDensity: VisualDensity.compact,
                  padding: const EdgeInsets.symmetric(horizontal: 10),
                ),
                selected: <String>{_selectedDocId},
                onSelectionChanged: (selection) {
                  setState(() => _selectedDocId = selection.first);
                },
                segments: <ButtonSegment<String>>[
                  ButtonSegment<String>(
                    value: 'base',
                    label: Text(
                      _artifact.language == 'ini'
                          ? 'Configuração WireGuard'
                          : 'Script padrão',
                      maxLines: 1,
                      softWrap: false,
                    ),
                  ),
                  for (final variant in _artifact.variants)
                    ButtonSegment<String>(
                      value: variant.id,
                      label: Text(variant.label, maxLines: 1, softWrap: false),
                    ),
                ],
              ),
            ),
            const SizedBox(height: 10),
          ],
          if (doc.hint != null && doc.hint!.isNotEmpty) ...<Widget>[
            Text(
              doc.hint!,
              style: context.textTheme.bodySmall?.copyWith(
                color: scheme.onSurfaceVariant,
              ),
            ),
            const SizedBox(height: 8),
          ],
          if (doc.instructions.isNotEmpty) ...<Widget>[
            NestedPanel(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Text(
                    'Como aplicar',
                    style: context.textTheme.labelLarge?.copyWith(
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                  const SizedBox(height: 6),
                  for (var i = 0; i < doc.instructions.length; i++)
                    Padding(
                      padding: const EdgeInsets.only(bottom: 4),
                      child: Text(
                        '${i + 1}. ${doc.instructions[i]}',
                        style: context.textTheme.bodySmall,
                      ),
                    ),
                ],
              ),
            ),
            const SizedBox(height: 10),
          ],
          CodeViewer(title: doc.fileName, code: doc.content, maxHeight: 280),
          const SizedBox(height: 14),
          Wrap(
            alignment: WrapAlignment.end,
            spacing: 8,
            runSpacing: 8,
            children: <Widget>[
              OutlinedButton.icon(
                onPressed: _downloadCurrent,
                icon: const Icon(Icons.download_outlined, size: 18),
                label: Text('Baixar ${doc.fileName}'),
              ),
              FilledButton.icon(
                onPressed: _copyCurrent,
                icon: const Icon(Icons.copy, size: 18),
                label: Text(
                  _artifact.language == 'ini' ? 'Copiar tudo' : 'Copiar script',
                ),
              ),
            ],
          ),
        ] else ...<Widget>[
          const SizedBox(height: 16),
          Align(
            alignment: Alignment.centerRight,
            child: FilledButton.icon(
              onPressed: _rotating ? null : _rotateKeys,
              icon: _rotating
                  ? const SizedBox(
                      width: 16,
                      height: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(Icons.key_outlined),
              label: const Text('Gerar novas chaves'),
            ),
          ),
        ],
      ],
    );
  }
}
