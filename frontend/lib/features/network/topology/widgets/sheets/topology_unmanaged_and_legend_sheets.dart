import 'package:flutter/material.dart';
import 'package:netmonitor_app/features/network/models/site_network_models.dart';
import 'package:netmonitor_app/features/network/topology/engine/topology_engine.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Abre a folha para cadastrar um Switch Não Gerenciável na topologia.
Future<void> showUnmanagedSwitchSheet({
  required BuildContext context,
  required List<SiteModel> sites,
  int? initialSiteId,
  required Future<bool> Function(Map<String, dynamic> payload) onSubmit,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Novo Switch Não Gerenciável',
    subtitle: 'Adicione um switch/hub sem SNMP para mapear derivações físicas',
    builder: (ctx) => _UnmanagedSwitchSheetBody(
      sites: sites,
      initialSiteId: initialSiteId,
      onSubmit: (payload) async {
        final ok = await onSubmit(payload);
        if (ctx.mounted && ok) {
          Navigator.of(ctx).pop();
        }
        return ok;
      },
    ),
  );
}

class _UnmanagedSwitchSheetBody extends StatefulWidget {
  const _UnmanagedSwitchSheetBody({
    required this.sites,
    this.initialSiteId,
    required this.onSubmit,
  });

  final List<SiteModel> sites;
  final int? initialSiteId;
  final Future<bool> Function(Map<String, dynamic> payload) onSubmit;

  @override
  State<_UnmanagedSwitchSheetBody> createState() =>
      _UnmanagedSwitchSheetBodyState();
}

class _UnmanagedSwitchSheetBodyState extends State<_UnmanagedSwitchSheetBody> {
  final TextEditingController _nameCtrl = TextEditingController();
  final TextEditingController _vendorCtrl = TextEditingController();
  final TextEditingController _portsCtrl = TextEditingController(text: '8');
  int? _selectedSiteId;
  bool _saving = false;

  @override
  void initState() {
    super.initState();
    _selectedSiteId = widget.sites.any((s) => s.id == widget.initialSiteId)
        ? widget.initialSiteId
        : null;
  }

  @override
  void dispose() {
    _nameCtrl.dispose();
    _vendorCtrl.dispose();
    _portsCtrl.dispose();
    super.dispose();
  }

  Future<void> _handleSubmit() async {
    if (_nameCtrl.text.trim().isEmpty || _saving) return;
    setState(() => _saving = true);
    final ok = await widget.onSubmit(<String, dynamic>{
      'name': _nameCtrl.text.trim(),
      if (_vendorCtrl.text.trim().isNotEmpty) 'vendor': _vendorCtrl.text.trim(),
      'portCount': int.tryParse(_portsCtrl.text.trim()) ?? 8,
      'siteId': _selectedSiteId,
    });
    if (mounted && !ok) {
      setState(() => _saving = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        TextField(
          controller: _nameCtrl,
          decoration: const InputDecoration(labelText: 'Nome do switch *'),
        ),
        const SizedBox(height: 12),
        ResponsiveFieldRow(
          breakpoint: 480,
          children: <Widget>[
            TextField(
              controller: _vendorCtrl,
              decoration: const InputDecoration(labelText: 'Fabricante'),
            ),
            TextField(
              controller: _portsCtrl,
              keyboardType: TextInputType.number,
              decoration: const InputDecoration(labelText: 'Portas'),
            ),
          ],
        ),
        if (widget.sites.isNotEmpty) ...<Widget>[
          const SizedBox(height: 12),
          AppDropdownFormField<int?>(
            initialValue: _selectedSiteId,
            decoration: const InputDecoration(labelText: 'Site'),
            items: <DropdownMenuItem<int?>>[
              const DropdownMenuItem<int?>(
                value: null,
                child: Text('Nenhum', overflow: TextOverflow.ellipsis),
              ),
              for (final s in widget.sites)
                DropdownMenuItem<int?>(
                  value: s.id,
                  child: Text(s.name, overflow: TextOverflow.ellipsis),
                ),
            ],
            onChanged: (v) => setState(() => _selectedSiteId = v),
          ),
        ],
        const SizedBox(height: 16),
        SymmetricalActionBar(
          children: <Widget>[
            OutlinedButton(
              onPressed: _saving ? null : () => Navigator.of(context).pop(),
              child: const Text('Cancelar'),
            ),
            FilledButton.icon(
              onPressed: _saving ? null : _handleSubmit,
              icon: _saving
                  ? const SizedBox(
                      width: 16,
                      height: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(Icons.hub_outlined),
              label: const Text(
                'Adicionar switch',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
            ),
          ],
        ),
      ],
    );
  }
}

/// Abre a legenda interativa da topologia (cores, enlaces, ícones e gestos).
Future<void> showTopologyLegendSheet(BuildContext context) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Legenda da Topologia',
    subtitle: 'Status, utilização de enlaces, tipos de mídia e gestos',
    maxWidth: 600,
    builder: (ctx) => const _TopologyLegendBody(),
  );
}

class _TopologyLegendBody extends StatelessWidget {
  const _TopologyLegendBody();

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const _LegendSectionTitle('Status dos Equipamentos'),
        const SizedBox(height: 8),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: <Widget>[
            for (final entry in kTopologyNodeStatusLegend)
              SemanticChip(
                label: entry.label,
                tone: entry.tone,
                icon: entry.icon,
              ),
          ],
        ),
        const Divider(height: 24),
        const _LegendSectionTitle('Utilização e Saúde dos Enlaces'),
        const SizedBox(height: 8),
        for (final entry in kTopologyUtilizationLegend) ...<Widget>[
          _LegendLineRow(entry: entry),
          const SizedBox(height: 6),
        ],
        const Divider(height: 24),
        const _LegendSectionTitle('Espessura por Velocidade Nominal'),
        const SizedBox(height: 8),
        for (final entry in kTopologySpeedTierLegend) ...<Widget>[
          _LegendLineRow(entry: entry),
          const SizedBox(height: 6),
        ],
        const Divider(height: 24),
        const _LegendSectionTitle('Tipos de Conexão'),
        const SizedBox(height: 8),
        for (final entry in kTopologyLinkKindLegend) ...<Widget>[
          _LegendLineRow(entry: entry),
          const SizedBox(height: 6),
        ],
        const Divider(height: 24),
        const _LegendSectionTitle('Gestos e Atalhos'),
        const SizedBox(height: 8),
        NestedPanel(
          padding: const EdgeInsets.all(12),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              _TipRow(
                icon: Icons.pinch_outlined,
                text:
                    'Pinça ou roda do mouse para dar zoom; arraste o fundo para mover o mapa.',
                color: scheme.primary,
              ),
              const SizedBox(height: 6),
              _TipRow(
                icon: Icons.touch_app_outlined,
                text:
                    'Toque em um nó ou linha de enlace para abrir o inspetor detalhado.',
                color: scheme.primary,
              ),
              const SizedBox(height: 6),
              _TipRow(
                icon: Icons.lock_outline,
                text:
                    'Destrave o cadeado (canto inferior direito) para reposicionar nós sem acidentes.',
                color: scheme.primary,
              ),
              const SizedBox(height: 6),
              _TipRow(
                icon: Icons.keyboard_outlined,
                text:
                    'Teclado (desktop): + / - (zoom), 0 (ajustar à tela), Delete (remover enlace selecionado), Ctrl+Z (desfazer), Esc (limpar seleção).',
                color: scheme.primary,
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _LegendSectionTitle extends StatelessWidget {
  const _LegendSectionTitle(this.text);

  final String text;

  @override
  Widget build(BuildContext context) {
    return Text(
      text,
      style: context.textTheme.titleSmall?.copyWith(
        fontWeight: FontWeight.w800,
      ),
    );
  }
}

class _LegendLineRow extends StatelessWidget {
  const _LegendLineRow({required this.entry});

  final TopologyLegendEntry entry;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final colors = context.semanticColors.resolve(entry.tone, scheme);
    final stroke = entry.strokeWidth ?? 2.6;

    return Row(
      children: <Widget>[
        SizedBox(
          width: 44,
          height: 16,
          child: CustomPaint(
            painter: _LegendLineSamplePainter(
              color: colors.foreground,
              lineStyle: entry.lineStyle ?? TopologyLineStyle.solid,
              strokeWidth: stroke,
            ),
          ),
        ),
        const SizedBox(width: 10),
        if (entry.icon != null) ...<Widget>[
          Icon(entry.icon, size: 16, color: colors.foreground),
          const SizedBox(width: 6),
        ],
        Expanded(
          child: Text(
            entry.description != null
                ? '${entry.label} · ${entry.description}'
                : entry.label,
            style: context.textTheme.bodySmall?.copyWith(
              fontWeight: FontWeight.w600,
            ),
          ),
        ),
      ],
    );
  }
}

class _LegendLineSamplePainter extends CustomPainter {
  const _LegendLineSamplePainter({
    required this.color,
    required this.lineStyle,
    required this.strokeWidth,
  });

  final Color color;
  final TopologyLineStyle lineStyle;
  final double strokeWidth;

  @override
  void paint(Canvas canvas, Size size) {
    final y = size.height / 2;
    final paint = Paint()
      ..color = color
      ..strokeWidth = strokeWidth
      ..strokeCap = StrokeCap.round
      ..style = PaintingStyle.stroke;

    switch (lineStyle) {
      case TopologyLineStyle.solid:
        canvas.drawLine(Offset(0, y), Offset(size.width, y), paint);
      case TopologyLineStyle.dashed:
        const dash = 7.0;
        const gap = 5.0;
        var x = 0.0;
        while (x < size.width) {
          final end = (x + dash).clamp(0.0, size.width);
          canvas.drawLine(Offset(x, y), Offset(end, y), paint);
          x += dash + gap;
        }
      case TopologyLineStyle.dotted:
        const step = 6.0;
        var x = 2.0;
        while (x < size.width) {
          canvas.drawCircle(
            Offset(x, y),
            strokeWidth * 0.55,
            Paint()..color = color,
          );
          x += step;
        }
    }
  }

  @override
  bool shouldRepaint(covariant _LegendLineSamplePainter oldDelegate) =>
      oldDelegate.color != color ||
      oldDelegate.lineStyle != lineStyle ||
      oldDelegate.strokeWidth != strokeWidth;
}

class _TipRow extends StatelessWidget {
  const _TipRow({required this.icon, required this.text, required this.color});

  final IconData icon;
  final String text;
  final Color color;

  @override
  Widget build(BuildContext context) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Icon(icon, size: 16, color: color),
        const SizedBox(width: 8),
        Expanded(child: Text(text, style: context.textTheme.bodySmall)),
      ],
    );
  }
}
