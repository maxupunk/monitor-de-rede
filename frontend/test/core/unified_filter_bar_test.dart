import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/shared/widgets/app_dropdown_form_field.dart';
import 'package:netmonitor_app/shared/widgets/choice_card.dart';
import 'package:netmonitor_app/shared/widgets/key_value_row.dart';
import 'package:netmonitor_app/shared/widgets/kpi_card.dart';
import 'package:netmonitor_app/shared/widgets/responsive_grid.dart';
import 'package:netmonitor_app/shared/widgets/section_card.dart';
import 'package:netmonitor_app/shared/widgets/unified_filter_bar.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

void main() {
  group('UnifiedFilterBar (360px mobile-first)', () {
    testWidgets(
      'supports search, clear button, active filter badge, filter sheet, sort menu, and removable active chips with zero overflow at 360px',
      (tester) async {
        tester.view.physicalSize = const Size(360, 800);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);

        final searchCtrl = TextEditingController();
        addTearDown(searchCtrl.dispose);

        var lastSearch = '';
        String? selectedType = 'router';
        String? selectedSite = 'Matriz';
        var activeSort = 'name_asc';
        var appliedCount = 0;

        await tester.pumpWidget(
          MaterialApp(
            theme: AppTheme.dark(),
            home: StatefulBuilder(
              builder: (context, setState) {
                final activeChips = <ActiveFilterChipData>[
                  if (selectedType != null)
                    ActiveFilterChipData(
                      label: 'Tipo: $selectedType',
                      icon: Icons.router_outlined,
                      onDeleted: () => setState(() => selectedType = null),
                    ),
                  if (selectedSite != null)
                    ActiveFilterChipData(
                      label: 'Site: $selectedSite',
                      onDeleted: () => setState(() => selectedSite = null),
                    ),
                ];

                return Scaffold(
                  body: SafeArea(
                    child: Padding(
                      padding: const EdgeInsets.all(12),
                      child: UnifiedFilterBar(
                        searchController: searchCtrl,
                        searchHint: 'Buscar dispositivo...',
                        onSearchChanged: (v) => setState(() => lastSearch = v),
                        activeFilterCount: activeChips.length,
                        filterSheetTitle: 'Filtros',
                        filterSheetBuilder: (sheetCtx, setSheetState) {
                          return Column(
                            crossAxisAlignment: CrossAxisAlignment.stretch,
                            children: <Widget>[
                              AppDropdownFormField<String?>(
                                initialValue: selectedType,
                                decoration: const InputDecoration(
                                  labelText: 'Tipo',
                                ),
                                items: const <DropdownMenuItem<String?>>[
                                  DropdownMenuItem<String?>(
                                    value: null,
                                    child: Text('Todos'),
                                  ),
                                  DropdownMenuItem<String?>(
                                    value: 'router',
                                    child: Text('Roteador'),
                                  ),
                                ],
                                onChanged: (v) {
                                  setState(() => selectedType = v);
                                  setSheetState(() {});
                                },
                              ),
                            ],
                          );
                        },
                        onApplyFilters: () => appliedCount++,
                        onClearAll: () => setState(() {
                          selectedType = null;
                          selectedSite = null;
                        }),
                        sortOptions: const <FilterSortOption>[
                          FilterSortOption(
                            value: 'name_asc',
                            label: 'Nome (A–Z)',
                          ),
                          FilterSortOption(
                            value: 'status',
                            label: 'Status',
                            icon: Icons.circle_outlined,
                          ),
                        ],
                        activeSort: activeSort,
                        onSortChanged: (v) => setState(() => activeSort = v),
                        quickChips: <Widget>[
                          FilterChip(
                            label: const Text('Todos (12)'),
                            selected: true,
                            onSelected: (_) {},
                          ),
                          FilterChip(
                            label: const Text('Online (10)'),
                            selected: false,
                            onSelected: (_) {},
                          ),
                        ],
                        activeChips: activeChips,
                      ),
                    ),
                  ),
                );
              },
            ),
          ),
        );
        await tester.pumpAndSettle();

        expect(tester.takeException(), isNull);
        // Badge mostra 2 filtros ativos + chips removíveis + botão Limpar.
        expect(find.text('2'), findsOneWidget);
        expect(find.text('Filtros'), findsOneWidget);
        expect(find.text('Tipo: router'), findsOneWidget);
        expect(find.text('Site: Matriz'), findsOneWidget);
        expect(find.text('Limpar'), findsOneWidget);

        // Digita na busca e limpa pelo botão X.
        await tester.enterText(find.byType(TextField), 'MikroTik');
        await tester.pumpAndSettle();
        expect(lastSearch, 'MikroTik');
        await tester.tap(find.byTooltip('Limpar busca'));
        await tester.pumpAndSettle();
        expect(lastSearch, '');
        expect(searchCtrl.text, '');

        // Abre menu de ordenação e seleciona Status.
        await tester.tap(find.byTooltip('Ordenar'));
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
        await tester.tap(find.text('Status').last);
        await tester.pumpAndSettle();
        expect(activeSort, 'status');

        // Abre bottom sheet de filtros e clica em Aplicar.
        await tester.tap(find.text('Filtros'));
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
        expect(find.text('Aplicar'), findsOneWidget);
        await tester.tap(find.text('Aplicar'));
        await tester.pumpAndSettle();
        expect(appliedCount, 1);

        // Remove um chip ativo clicando no ícone de deletar (rola a faixa horizontal primeiro).
        final deleteChipBtn = find.byTooltip('Remover filtro').first;
        await tester.ensureVisible(deleteChipBtn);
        await tester.pumpAndSettle();
        await tester.tap(deleteChipBtn);
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
        expect(find.text('Tipo: router'), findsNothing);
        expect(find.text('Site: Matriz'), findsOneWidget);
        expect(find.text('1'), findsOneWidget);
      },
    );
  });

  group('KpiCard 2x2 mobile grid (360px)', () {
    testWidgets(
      'renders 4 KpiCards in a symmetrical 2x2 grid at 360px with zero overflow',
      (tester) async {
        tester.view.physicalSize = const Size(360, 800);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);

        await tester.pumpWidget(
          MaterialApp(
            theme: AppTheme.dark(),
            home: const Scaffold(
              body: SafeArea(
                child: Padding(
                  padding: EdgeInsets.all(12),
                  child: ResponsiveGrid(
                    xs: 2,
                    sm: 2,
                    md: 4,
                    children: <Widget>[
                      KpiCard(
                        label: 'Dispositivos',
                        value: '42',
                        subtitle: '39 online',
                        icon: Icons.router_outlined,
                        tone: SemanticTone.primary,
                      ),
                      KpiCard(
                        label: 'Monitores ativos',
                        value: '128',
                        subtitle: '99.9% uptime',
                        icon: Icons.monitor_heart_outlined,
                        tone: SemanticTone.success,
                      ),
                      KpiCard(
                        label: 'Indisponíveis',
                        value: '3',
                        subtitle: '2 críticos',
                        icon: Icons.error_outline,
                        tone: SemanticTone.error,
                      ),
                      KpiCard(
                        label: 'Alertas abertos',
                        value: '5',
                        subtitle: 'Últimas 24h',
                        icon: Icons.notifications_active_outlined,
                        tone: SemanticTone.warning,
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();

        expect(tester.takeException(), isNull);
        expect(find.text('Dispositivos'), findsOneWidget);
        expect(find.text('Monitores ativos'), findsOneWidget);
        expect(find.text('Indisponíveis'), findsOneWidget);
        expect(find.text('Alertas abertos'), findsOneWidget);

        // Confirma grade simétrica 2x2 (cards 0 e 1 na mesma linha; 2 e 3 abaixo).
        final topLeft0 = tester.getTopLeft(find.byType(KpiCard).at(0));
        final topLeft1 = tester.getTopLeft(find.byType(KpiCard).at(1));
        final topLeft2 = tester.getTopLeft(find.byType(KpiCard).at(2));
        final topLeft3 = tester.getTopLeft(find.byType(KpiCard).at(3));
        expect(topLeft0.dy, equals(topLeft1.dy));
        expect(topLeft1.dx, greaterThan(topLeft0.dx));
        expect(topLeft2.dy, greaterThan(topLeft0.dy));
        expect(topLeft2.dy, equals(topLeft3.dy));
        expect(topLeft3.dx, greaterThan(topLeft2.dx));
      },
    );
  });

  group('SectionCard, ChoiceCard & KeyValueRow anti-crush at 360px', () {
    testWidgets(
      'SectionCard stacks header above wide Row/SegmentedButton trailing at 360px without crushing title vertically or overflowing',
      (tester) async {
        tester.view.physicalSize = const Size(360, 844);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);

        await tester.pumpWidget(
          MaterialApp(
            theme: AppTheme.dark(),
            home: Scaffold(
              body: SafeArea(
                child: SingleChildScrollView(
                  padding: const EdgeInsets.all(12),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: <Widget>[
                      SectionCard(
                        icon: Icons.dns_outlined,
                        title: 'Servidores DNS cadastrados',
                        subtitle:
                            'Configure servidores internos e externos para resolução',
                        trailing: Row(
                          mainAxisSize: MainAxisSize.min,
                          children: <Widget>[
                            OutlinedButton.icon(
                              onPressed: () {},
                              icon: const Icon(Icons.playlist_add_check),
                              label: const Text('Monitorar em lote'),
                            ),
                            const SizedBox(width: 8),
                            FilledButton.icon(
                              onPressed: () {},
                              icon: const Icon(Icons.settings),
                              label: const Text('Gerenciar'),
                            ),
                          ],
                        ),
                        child: const Text('Conteúdo DNS'),
                      ),
                      const SizedBox(height: 12),
                      SectionCard(
                        icon: Icons.speed,
                        title: 'Benchmark de Modelos IA',
                        subtitle:
                            'Compare latência e taxa de tokens entre provedores locais',
                        trailing: SegmentedButton<String>(
                          showSelectedIcon: false,
                          segments: const <ButtonSegment<String>>[
                            ButtonSegment<String>(
                              value: 'rapido',
                              label: Text('Rápido'),
                            ),
                            ButtonSegment<String>(
                              value: 'padrao',
                              label: Text('Padrão'),
                            ),
                            ButtonSegment<String>(
                              value: 'completo',
                              label: Text('Completo'),
                            ),
                          ],
                          selected: const <String>{'padrao'},
                          onSelectionChanged: (_) {},
                        ),
                        child: const Text('Resultados do benchmark'),
                      ),
                      const SizedBox(height: 12),
                      ChoiceCard(
                        icon: Icons.vpn_key_outlined,
                        title: 'Chave SSH dedicada (OpenWrt)',
                        subtitle:
                            'Provisiona conta dedicada sem guardar senha administrativa.',
                        badge: 'Recomendado',
                        onTap: () {},
                        trailing: FilledButton(
                          onPressed: () {},
                          child: const Text('Configurar agora'),
                        ),
                      ),
                      const SizedBox(height: 12),
                      KeyValueRow(
                        label: 'Endereço do servidor Syslog',
                        value: '192.168.100.254:514',
                        monospace: true,
                        copyable: true,
                        trailing: OutlinedButton(
                          onPressed: () {},
                          child: const Text('Testar conexão'),
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();

        expect(tester.takeException(), isNull);

        // Título do SectionCard recebe largura total e mantém altura compacta (não esmagado em 1 letra por linha).
        final dnsTitleSize = tester.getSize(
          find.text('Servidores DNS cadastrados'),
        );
        final benchTitleSize = tester.getSize(
          find.text('Benchmark de Modelos IA'),
        );
        expect(dnsTitleSize.width, greaterThan(180));
        expect(dnsTitleSize.height, lessThan(60));
        expect(benchTitleSize.width, greaterThan(180));
        expect(benchTitleSize.height, lessThan(60));

        // Trailing aparece abaixo do título no mobile (< 560px).
        final dnsTitleBottom = tester
            .getBottomLeft(find.text('Servidores DNS cadastrados'))
            .dy;
        final trailingBtnTop = tester
            .getTopLeft(find.text('Monitorar em lote'))
            .dy;
        expect(trailingBtnTop, greaterThan(dnsTitleBottom));
      },
    );
  });
}
