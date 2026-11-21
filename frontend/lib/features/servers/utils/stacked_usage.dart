import 'dart:math' as math;

import 'package:flutter/material.dart';

/// Paleta categórica validada para temas claro e escuro (ordem fixa — a cor
/// segue a primeira aparição da série na janela, nunca o ranking dinâmico).
const List<Color> kStackedChartPalette = <Color>[
  Color(0xFF3987E5), // azul
  Color(0xFFD95926), // laranja
  Color(0xFF199E70), // esmeralda
  Color(0xFFC98500), // âmbar
  Color(0xFFD55181), // magenta
  Color(0xFF008300), // verde
  Color(0xFF9085E9), // violeta
];

/// Cor neutra do agrupamento "Outros" para não competir com as séries ativas.
const Color kOthersLayerColor = Color(0xFF757575);

/// Identificador estável do agrupamento "Outros".
const String kOthersLayerId = '__outros__';

/// Uma amostra temporal com o valor de cada container (`containerId -> valor`).
@immutable
class UsageSample {
  const UsageSample({required this.time, required this.values});

  final String time;
  final Map<String, double> values;
}

/// Uma faixa empilhada no gráfico (container individual ou agrupamento "Outros").
@immutable
class StackedLayer {
  const StackedLayer({
    required this.id,
    required this.label,
    required this.color,
    required this.values,
    required this.lower,
    required this.upper,
  });

  final String id;
  final String label;
  final Color color;

  /// Valor individual desta camada em cada amostra (`0` quando ausente).
  final List<double> values;

  /// Base acumulada da faixa em cada amostra.
  final List<double> lower;

  /// Topo acumulado da faixa em cada amostra.
  final List<double> upper;

  /// Valor mais recente registrado na janela.
  double get latestValue => values.isEmpty ? 0 : values.last;
}

/// Resultado completo do empilhamento para uma janela de amostras.
@immutable
class StackedUsage {
  const StackedUsage({
    required this.layers,
    required this.totals,
    required this.times,
    required this.peak,
  });

  final List<StackedLayer> layers;
  final List<double> totals;
  final List<String> times;
  final double peak;

  bool get isEmpty => times.isEmpty;
}

/// Escala vertical arredondada (`maxY` + `interval`) para rótulos humanos limpos
/// (ex.: `250 / 500 / 750 / 1000 MiB` ou `2% / 4% / 6% / 8%`, nunca `191 / 381 / 572 MiB`).
@immutable
class NiceAxisScale {
  const NiceAxisScale({
    required this.maxY,
    required this.interval,
    required this.divisions,
  });

  final double maxY;
  final double interval;
  final int divisions;
}

/// Teto redondo do eixo (`1, 2, 4, 5, 8, 10 × baseⁿ`) acima de [value].
/// Bytes usam `base = 1024`, garantindo marcas inteiras em KiB/MiB/GiB.
double niceCeiling(double value, {int base = 10}) {
  if (!value.isFinite || value <= 0) return 1;
  if (base == 1024) {
    final exp = math.max(0, (math.log(value) / math.log(1024)).floor());
    final unit = math.pow(1024, exp).toDouble();
    return niceCeiling(value / unit, base: 10) * unit;
  }
  final exp = (math.log(value) / math.ln10).floor();
  final magnitude = math.pow(10, exp).toDouble();
  final normalized = value / magnitude;
  const factors = <double>[1, 2, 4, 5, 8, 10];
  final factor = factors.firstWhere(
    (f) => f >= normalized - 1e-9,
    orElse: () => 10,
  );
  return factor * magnitude;
}

/// Calcula o teto e o passo uniforme para o eixo Y com 4 ou 5 divisões exatas.
NiceAxisScale niceAxisScale(
  double peak, {
  int base = 10,
  double minCeiling = 0,
}) {
  final target = math.max(peak.isFinite ? peak : 0.0, minCeiling);
  if (target <= 0) {
    return const NiceAxisScale(maxY: 4, interval: 1, divisions: 4);
  }

  if (base == 1024) {
    final exp = math.max(0, (math.log(target) / math.log(1024)).floor());
    final unit = math.pow(1024, exp).toDouble();
    final inner = niceAxisScale(target / unit, base: 10);
    return NiceAxisScale(
      maxY: inner.maxY * unit,
      interval: inner.interval * unit,
      divisions: inner.divisions,
    );
  }

  final ceiling = niceCeiling(target, base: 10);
  final exp = (math.log(ceiling) / math.ln10).floor();
  final magnitude = math.pow(10, exp).toDouble();
  final leading = (ceiling / magnitude).round();

  // 5 -> 5 divisões inteiras (1,2,3,4,5); demais (1,2,4,8,10) -> 4 divisões limpas.
  final divisions = leading == 5 ? 5 : 4;
  return NiceAxisScale(
    maxY: ceiling,
    interval: ceiling / divisions,
    divisions: divisions,
  );
}

/// Empilha as séries de consumo por container na janela.
///
/// As [maxSeries] de maior soma na janela recebem faixa própria; o restante é
/// agregado em `Outros (N)` (ou no nome real do container se restar apenas 1).
/// A cor segue a ordem de primeira aparição (`firstSeen`) para não piscar
/// quando o ranking muda entre amostras SSE.
StackedUsage buildStackedUsage(
  List<UsageSample> samples,
  Map<String, String> labels, {
  int maxSeries = 6,
}) {
  final times = samples.map((s) => s.time).toList(growable: false);
  if (samples.isEmpty) {
    return const StackedUsage(
      layers: <StackedLayer>[],
      totals: <double>[],
      times: <String>[],
      peak: 0,
    );
  }

  final firstSeen = <String>[];
  final sums = <String, double>{};
  for (final sample in samples) {
    final ids = sample.values.keys.toList()
      ..sort(
        (a, b) => (labels[a] ?? a).toLowerCase().compareTo(
          (labels[b] ?? b).toLowerCase(),
        ),
      );
    for (final id in ids) {
      if (!sums.containsKey(id)) firstSeen.add(id);
      final v = sample.values[id] ?? 0;
      sums[id] = (sums[id] ?? 0) + (v.isFinite && v > 0 ? v : 0);
    }
  }

  final limit = maxSeries.clamp(0, kStackedChartPalette.length);
  final ranked = sums.entries.toList()
    ..sort((a, b) => b.value.compareTo(a.value));
  final own = ranked.take(limit).map((e) => e.key).toSet();
  final shown = firstSeen.where(own.contains).toList(growable: false);
  final rest = firstSeen
      .where((id) => !own.contains(id))
      .toList(growable: false);

  final definitions =
      <({String id, String label, Color color, List<String> ids})>[
        for (var i = 0; i < shown.length; i++)
          (
            id: shown[i],
            label: labels[shown[i]] ?? shown[i],
            color: kStackedChartPalette[i % kStackedChartPalette.length],
            ids: <String>[shown[i]],
          ),
        if (rest.isNotEmpty)
          (
            id: kOthersLayerId,
            label: rest.length == 1
                ? (labels[rest.first] ?? rest.first)
                : 'Outros (${rest.length})',
            color: kOthersLayerColor,
            ids: rest,
          ),
      ];

  final base = List<double>.filled(samples.length, 0);
  final layers = <StackedLayer>[];
  for (final def in definitions) {
    final values = List<double>.generate(samples.length, (idx) {
      var sum = 0.0;
      final sampleValues = samples[idx].values;
      for (final id in def.ids) {
        final v = sampleValues[id] ?? 0;
        if (v.isFinite && v > 0) sum += v;
      }
      return sum;
    }, growable: false);

    final lower = List<double>.from(base, growable: false);
    final upper = List<double>.generate(
      samples.length,
      (idx) => lower[idx] + values[idx],
      growable: false,
    );
    for (var i = 0; i < samples.length; i++) {
      base[i] = upper[i];
    }

    layers.add(
      StackedLayer(
        id: def.id,
        label: def.label,
        color: def.color,
        values: values,
        lower: lower,
        upper: upper,
      ),
    );
  }

  var peak = 0.0;
  for (final total in base) {
    if (total > peak) peak = total;
  }

  return StackedUsage(
    layers: List<StackedLayer>.unmodifiable(layers),
    totals: List<double>.unmodifiable(base),
    times: times,
    peak: peak,
  );
}
