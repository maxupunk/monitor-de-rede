import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/clipboard_utils.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Bloco de código / comandos / logs com rolagem horizontal e botão de cópia.
///
/// Com [followTail] (console ao vivo), acompanha a última linha a cada
/// atualização de [code] enquanto quem lê estiver no fim; rolou para cima
/// para ler, o acompanhamento pausa e aparece "Ir para o fim", que o retoma.
class CodeViewer extends StatefulWidget {
  const CodeViewer({
    super.key,
    required this.code,
    this.title,
    this.maxHeight,
    this.copyable = true,
    this.followTail = false,
    this.actions = const <Widget>[],
  });

  final String code;
  final String? title;
  final double? maxHeight;
  final bool copyable;

  /// Console que cresce ao vivo: rola até a última linha quando chega linha nova.
  final bool followTail;

  /// Ações extras no cabeçalho, ao lado de "Copiar".
  final List<Widget> actions;

  @override
  State<CodeViewer> createState() => _CodeViewerState();
}

class _CodeViewerState extends State<CodeViewer> {
  /// Distância do fim (px) que ainda conta como "no fim".
  static const double _bottomSlack = 24;

  final ScrollController _vertical = ScrollController();
  final ScrollController _horizontal = ScrollController();

  /// Quem lê está no fim: linha nova rola junto.
  bool _following = true;

  /// Chegou linha nova enquanto o acompanhamento estava pausado.
  bool _unseen = false;

  @override
  void initState() {
    super.initState();
    if (widget.followTail) _scheduleJumpToEnd();
  }

  @override
  void didUpdateWidget(covariant CodeViewer oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!widget.followTail) return;
    if (!oldWidget.followTail) {
      _following = true;
      _unseen = false;
      _scheduleJumpToEnd();
      return;
    }
    if (widget.code == oldWidget.code) return;
    if (_following) {
      _scheduleJumpToEnd();
    } else {
      _unseen = true;
    }
  }

  @override
  void dispose() {
    _vertical.dispose();
    _horizontal.dispose();
    super.dispose();
  }

  /// Rola depois do layout, quando a extensão nova já é conhecida.
  void _scheduleJumpToEnd() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !_vertical.hasClients) return;
      _vertical.jumpTo(_vertical.position.maxScrollExtent);
    });
  }

  bool _onScroll(ScrollNotification notification) {
    if (!widget.followTail ||
        notification.depth != 0 ||
        notification.metrics.axis != Axis.vertical) {
      return false;
    }
    if (notification is ScrollUpdateNotification ||
        notification is ScrollEndNotification) {
      final atEnd = notification.metrics.extentAfter <= _bottomSlack;
      if (atEnd != _following || (atEnd && _unseen)) {
        setState(() {
          _following = atEnd;
          if (atEnd) _unseen = false;
        });
      }
    }
    return false;
  }

  Future<void> _resumeFollowing() async {
    setState(() {
      _following = true;
      _unseen = false;
    });
    if (!_vertical.hasClients) return;
    await _vertical.animateTo(
      _vertical.position.maxScrollExtent,
      duration: const Duration(milliseconds: 220),
      curve: Curves.easeOut,
    );
  }

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final showHeader =
        widget.title != null || widget.copyable || widget.actions.isNotEmpty;

    return Container(
      decoration: BoxDecoration(
        color: scheme.surfaceContainerLowest,
        borderRadius: AppTheme.borderRadius,
        border: Border.all(color: scheme.outlineVariant),
      ),
      clipBehavior: Clip.antiAlias,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          if (showHeader)
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
              decoration: BoxDecoration(
                color: scheme.surfaceContainer,
                border: Border(
                  bottom: BorderSide(color: scheme.outlineVariant),
                ),
              ),
              child: Row(
                children: <Widget>[
                  Expanded(
                    child: Text(
                      widget.title ?? 'Código',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: context.textTheme.labelMedium?.copyWith(
                        color: scheme.onSurfaceVariant,
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                  ),
                  ...widget.actions,
                  if (widget.copyable)
                    TextButton.icon(
                      onPressed: () => copyToClipboardWithFeedback(
                        context,
                        widget.code,
                        message: 'Conteúdo copiado.',
                      ),
                      icon: const Icon(Icons.copy, size: 15),
                      label: const Text('Copiar'),
                      style: TextButton.styleFrom(
                        visualDensity: VisualDensity.compact,
                        padding: const EdgeInsets.symmetric(horizontal: 8),
                      ),
                    ),
                ],
              ),
            ),
          ConstrainedBox(
            constraints: BoxConstraints(
              maxHeight: widget.maxHeight ?? double.infinity,
            ),
            // `passthrough`: a viewport ocupa a largura toda do console, e
            // não só a do texto — senão a barra vertical fica colada ao fim
            // da linha mais larga, no meio do console.
            child: Stack(
              fit: StackFit.passthrough,
              children: <Widget>[
                // As duas barras envolvem a viewport externa: a vertical fica
                // na borda direita e a horizontal (notificações de
                // profundidade 1, da rolagem interna) na borda de baixo da
                // área visível, qualquer que seja a largura das linhas.
                Scrollbar(
                  controller: _vertical,
                  child: Scrollbar(
                    controller: _horizontal,
                    notificationPredicate: (notification) =>
                        notification.depth == 1,
                    child: ScrollConfiguration(
                      // As barras são as de cima; sem as automáticas do
                      // desktop, que seguiriam o conteúdo.
                      behavior: ScrollConfiguration.of(
                        context,
                      ).copyWith(scrollbars: false),
                      child: NotificationListener<ScrollNotification>(
                        onNotification: _onScroll,
                        child: SingleChildScrollView(
                          controller: _vertical,
                          child: SingleChildScrollView(
                            controller: _horizontal,
                            scrollDirection: Axis.horizontal,
                            padding: const EdgeInsets.all(14),
                            child: SelectableText(
                              widget.code,
                              style: TextStyle(
                                fontFamily: 'monospace',
                                fontSize: 12.5,
                                height: 1.45,
                                color: scheme.onSurface,
                              ),
                            ),
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
                if (widget.followTail && !_following)
                  Positioned(
                    right: 12,
                    bottom: 12,
                    child: FilledButton.icon(
                      onPressed: _resumeFollowing,
                      icon: const Icon(Icons.vertical_align_bottom, size: 16),
                      label: Text(
                        _unseen
                            ? 'Novas linhas · Ir para o fim'
                            : 'Ir para o fim',
                      ),
                      style: FilledButton.styleFrom(
                        visualDensity: VisualDensity.compact,
                        padding: const EdgeInsets.symmetric(horizontal: 12),
                      ),
                    ),
                  ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
