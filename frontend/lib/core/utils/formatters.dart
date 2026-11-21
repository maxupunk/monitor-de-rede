import 'dart:math' as math;

import 'package:intl/intl.dart';

/// Fonte única dos formatadores de exibição (DRY absoluto — AGENTS.md §11).
/// 100% de paridade com `frontend/src/utils/formatters.ts`.

const List<String> _binaryByteUnits = <String>[
  'B',
  'KiB',
  'MiB',
  'GiB',
  'TiB',
  'PiB',
];
const List<String> _decimalByteUnits = <String>[
  'B',
  'KB',
  'MB',
  'GB',
  'TB',
  'PB',
];
const List<String> _bitRateUnits = <String>[
  'bps',
  'Kbps',
  'Mbps',
  'Gbps',
  'Tbps',
];

enum SpeedUnit { auto, kbps, mbps, gbps }

String _trimTrailingZeros(num value, int fractionDigits) {
  final fixed = value.toDouble().toStringAsFixed(fractionDigits);
  final parsed = double.parse(fixed);
  if (parsed == parsed.truncateToDouble()) {
    return parsed.toInt().toString();
  }
  return parsed.toString();
}

String _formatScaled(
  num? value,
  int base,
  List<String> units, {
  int fractionDigits = 2,
  String? fallback,
}) {
  final zeroLabel = '0 ${units.first}';
  if (value == null || !value.isFinite) {
    return fallback ?? zeroLabel;
  }
  if (value == 0) {
    return fallback ?? zeroLabel;
  }

  final magnitude = value.abs().toDouble();
  final rawIndex = magnitude < 1
      ? 0
      : (math.log(magnitude) / math.log(base)).floor();
  final index = rawIndex.clamp(0, units.length - 1);
  final scaled = value.toDouble() / math.pow(base, index);

  return '${_trimTrailingZeros(scaled, fractionDigits)} ${units[index]}';
}

/// Memória e armazenamento em base 1024: `1536` → `"1.5 KiB"`.
String formatBinaryBytes(
  num? value, {
  int fractionDigits = 2,
  String? fallback,
}) {
  return _formatScaled(
    value,
    1024,
    _binaryByteUnits,
    fractionDigits: fractionDigits,
    fallback: fallback,
  );
}

/// Contadores Docker em base decimal: `1500` → `"1.5 KB"`.
String formatDecimalBytes(
  num? value, {
  int fractionDigits = 2,
  String? fallback,
}) {
  return _formatScaled(
    value,
    1000,
    _decimalByteUnits,
    fractionDigits: fractionDigits,
    fallback: fallback,
  );
}

/// Alias padrão para armazenamento em base 1024.
String formatBytes(num? value, {int fractionDigits = 2, String? fallback}) {
  return formatBinaryBytes(
    value,
    fractionDigits: fractionDigits,
    fallback: fallback,
  );
}

/// Taxa de transmissão em base 1000: `1500000` → `"1.5 Mbps"`.
String formatBps(num? value, {int fractionDigits = 2, String? fallback}) {
  return _formatScaled(
    value,
    1000,
    _bitRateUnits,
    fractionDigits: fractionDigits,
    fallback: fallback,
  );
}

/// Velocidade negociada de link (`0` significa "não informado" → `"N/A"`).
String formatLinkSpeed(num? bps) {
  if (bps == null || !bps.isFinite || bps <= 0) return 'N/A';
  return formatBps(bps, fractionDigits: 1);
}

/// Resolve a unidade automática para velocidade em Mbps.
SpeedUnit resolveAutoUnit(num? speedMbps) {
  if (speedMbps == null || !speedMbps.isFinite || speedMbps <= 0) {
    return SpeedUnit.mbps;
  }
  final abs = speedMbps.abs();
  if (abs >= 1000) return SpeedUnit.gbps;
  if (abs < 1) return SpeedUnit.kbps;
  return SpeedUnit.mbps;
}

SpeedUnit resolveSpeedUnit(num? valueMbps, [SpeedUnit unit = SpeedUnit.auto]) {
  if (unit != SpeedUnit.auto) return unit;
  return resolveAutoUnit(valueMbps);
}

double convertSpeedFromMbps(num mbps, SpeedUnit unit) {
  final resolved = resolveSpeedUnit(mbps, unit);
  switch (resolved) {
    case SpeedUnit.kbps:
      return mbps.toDouble() * 1000;
    case SpeedUnit.gbps:
      return mbps.toDouble() / 1000;
    case SpeedUnit.mbps:
    case SpeedUnit.auto:
      return mbps.toDouble();
  }
}

String formatSpeedValue(
  num? value, {
  SpeedUnit unit = SpeedUnit.mbps,
  String fallback = '--',
}) {
  if (value == null || !value.isFinite) return fallback;
  final resolved = unit == SpeedUnit.auto ? SpeedUnit.mbps : unit;
  final fractionDigits = switch (resolved) {
    SpeedUnit.gbps => 2,
    SpeedUnit.kbps => 0,
    SpeedUnit.mbps || SpeedUnit.auto => 1,
  };
  return _trimTrailingZeros(value, fractionDigits);
}

String formatSpeedByUnit(
  num? valueMbps, {
  SpeedUnit unit = SpeedUnit.auto,
  String fallback = '--',
}) {
  if (valueMbps == null || !valueMbps.isFinite) return fallback;
  final resolved = resolveSpeedUnit(valueMbps, unit);
  final converted = convertSpeedFromMbps(valueMbps, resolved);
  return formatSpeedValue(converted, unit: resolved, fallback: fallback);
}

String getSpeedUnitLabel(SpeedUnit unit, {num? valueMbps}) {
  final resolved = resolveSpeedUnit(valueMbps, unit);
  switch (resolved) {
    case SpeedUnit.kbps:
      return 'Kbps';
    case SpeedUnit.gbps:
      return 'Gbps';
    case SpeedUnit.mbps:
    case SpeedUnit.auto:
      return 'Mbps';
  }
}

String formatSpeedMbps(num? value, {String fallback = '--'}) {
  return formatSpeedByUnit(value, unit: SpeedUnit.mbps, fallback: fallback);
}

/// Latência em milissegundos: `6.9038` → `"6.9 ms"`, `250.4` → `"250 ms"`.
String formatLatency(num? value, {String fallback = 'N/A'}) {
  if (value == null || !value.isFinite) return fallback;
  final digits = value.abs() >= 100 ? 0 : 1;
  return '${_trimTrailingZeros(value, digits)} ms';
}

String _formatPtBrNumber(
  num value, {
  int minimumFractionDigits = 0,
  int maximumFractionDigits = 1,
}) {
  final formatter = NumberFormat.decimalPattern('pt_BR')
    ..minimumFractionDigits = minimumFractionDigits
    ..maximumFractionDigits = maximumFractionDigits;
  return formatter.format(value);
}

/// Distância aproximada como faixa: `"~3–6 m"`.
String formatDistanceRange(num? minM, num? maxM, {String fallback = '—'}) {
  if (minM == null || maxM == null || !minM.isFinite || !maxM.isFinite) {
    return fallback;
  }
  final minStr = _formatPtBrNumber(minM.round(), maximumFractionDigits: 0);
  final maxStr = _formatPtBrNumber(maxM.round(), maximumFractionDigits: 0);
  return '~$minStr–$maxStr m';
}

/// Percentual com casas fixas em pt-BR: `"42,5%"`.
String formatPercent(num? value, {int digits = 1, String fallback = '—'}) {
  if (value == null || !value.isFinite) return fallback;
  final formatted = _formatPtBrNumber(
    value,
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  );
  return '$formatted%';
}

/// Inteiro com separador de milhar em pt-BR: `"2.210"`.
String formatInteger(num? value, {String fallback = '—'}) {
  if (value == null || !value.isFinite) return fallback;
  return _formatPtBrNumber(value.round(), maximumFractionDigits: 0);
}

/// Contagem compacta: `"850"`, `"1,2 mil"`, `"3,4 mi"`.
String formatCompactCount(num? value, {String fallback = '—'}) {
  if (value == null || !value.isFinite) return fallback;
  if (value.abs() < 1000) return value.round().toString();
  final isThousands = value.abs() < 1000000;
  final divisor = isThousands ? 1000 : 1000000;
  final suffix = isThousands ? 'mil' : 'mi';
  return '${_formatPtBrNumber(value / divisor, maximumFractionDigits: 1)} $suffix';
}

/// Janela de contexto de modelo local em k binário: `262144` → `"256k"`.
String formatContextWindow(num? tokens, {bool detailed = false}) {
  if (tokens == null || !tokens.isFinite || tokens <= 0) return '—';
  final intTokens = tokens.round();
  final short = intTokens < 1024 ? '$intTokens' : '${intTokens ~/ 1024}k';
  if (!detailed) return short;
  return '${formatInteger(intTokens)} tokens ($short)';
}

/// Hora cheia do dia (0–23): `8` → `"08:00"`.
String formatHourOfDay(int hour) {
  return '${hour.toString().padLeft(2, '0')}:00';
}

/// Velocidade de geração da IA: `"42,3 tok/s"`.
String formatTokenRate(num? value, {String fallback = '—'}) {
  if (value == null || !value.isFinite) return fallback;
  final digits = value.abs() >= 100 ? 0 : 1;
  return '${_formatPtBrNumber(value, maximumFractionDigits: digits)} tok/s';
}

/// Duração curta em ms: `"850 ms"`, `"4,2 s"`, `"1 min 12 s"`.
String formatElapsedMs(num? value, {String fallback = '—'}) {
  if (value == null || !value.isFinite || value < 0) return fallback;
  if (value < 1000) return '${value.round()} ms';
  final seconds = value / 1000;
  if (seconds < 60) {
    return '${_formatPtBrNumber(seconds, maximumFractionDigits: 1)} s';
  }
  final total = seconds.round();
  return '${total ~/ 60} min ${total % 60} s';
}

String formatMeasuredValue(Object? value, {String? unit}) {
  final numeric = value is num ? value : num.tryParse(value?.toString() ?? '');
  if (numeric == null || !numeric.isFinite) return value?.toString() ?? '';
  if (unit == 'bytes' || unit == 'B') return formatBytes(numeric);
  if (unit == 'bps') return formatBps(numeric);
  if (unit == 'ms') return formatLatency(numeric);
  return '$numeric${unit != null && unit.isNotEmpty ? ' $unit' : ''}';
}

DateTime? _toDate(Object? value) {
  if (value == null) return null;
  if (value is DateTime) return value.toLocal();
  if (value is String && value.trim().isNotEmpty) {
    return DateTime.tryParse(value)?.toLocal();
  }
  return null;
}

String _two(int n) => n.toString().padLeft(2, '0');

/// Data e hora completas em pt-BR: `"05/08/2026 14:32:07"`.
String formatDateTime(Object? value, {String fallback = '—'}) {
  final date = _toDate(value);
  if (date == null) {
    final raw = value?.toString();
    return (raw != null && raw.isNotEmpty) ? raw : fallback;
  }
  return '${_two(date.day)}/${_two(date.month)}/${date.year} '
      '${_two(date.hour)}:${_two(date.minute)}:${_two(date.second)}';
}

/// Somente a data em pt-BR: `"05/08/2026"`.
String formatDate(Object? value, {String fallback = '—'}) {
  final date = _toDate(value);
  if (date == null) {
    final raw = value?.toString();
    return (raw != null && raw.isNotEmpty) ? raw : fallback;
  }
  return '${_two(date.day)}/${_two(date.month)}/${date.year}';
}

/// Data e hora compactas sem ano: `"05/08 14:32:07"`.
String formatShortDateTime(Object? value, {String fallback = ''}) {
  final date = _toDate(value);
  if (date == null) {
    final raw = value?.toString();
    return (raw != null && raw.isNotEmpty) ? raw : fallback;
  }
  return '${_two(date.day)}/${_two(date.month)} '
      '${_two(date.hour)}:${_two(date.minute)}:${_two(date.second)}';
}

/// Somente o horário: `"14:32:07"`.
String formatClockTime([Object? value]) {
  final date = _toDate(value) ?? DateTime.now();
  return '${_two(date.hour)}:${_two(date.minute)}:${_two(date.second)}';
}

/// Rótulo do eixo X de gráfico: `"14:32"` (<= 36 h) ou `"05/08"` (> 36 h).
String formatAxisTime(Object value, int spanMs) {
  final date = _toDate(value);
  if (date == null) return value.toString();
  if (spanMs > 36 * 3600 * 1000) {
    return '${_two(date.day)}/${_two(date.month)}';
  }
  return '${_two(date.hour)}:${_two(date.minute)}';
}

/// Tempo decorrido em português: `"há 2 min"`, `"há 3 dias"`, `"nunca"`.
String formatRelativeTime(
  Object? value, {
  String emptyLabel = 'nunca',
  DateTime? now,
}) {
  final date = _toDate(value);
  if (date == null) return emptyLabel;
  final refNow = now ?? DateTime.now();
  final elapsedSeconds = refNow.difference(date).inSeconds;
  if (elapsedSeconds < 60) return 'há ${math.max(elapsedSeconds, 0)}s';
  if (elapsedSeconds < 3600) return 'há ${elapsedSeconds ~/ 60} min';
  if (elapsedSeconds < 86400) return 'há ${elapsedSeconds ~/ 3600} h';
  final days = elapsedSeconds ~/ 86400;
  return 'há $days ${days == 1 ? 'dia' : 'dias'}';
}

/// Tempo que falta em português: `"em 5 min"`, `"em 3 h"`, `"agora"`.
String formatTimeUntil(
  Object? value, {
  String emptyLabel = '—',
  DateTime? now,
}) {
  final date = _toDate(value);
  if (date == null) return emptyLabel;
  final refNow = now ?? DateTime.now();
  final remainingSeconds = date.difference(refNow).inSeconds;
  if (remainingSeconds < 60) return 'agora';
  if (remainingSeconds < 3600) return 'em ${remainingSeconds ~/ 60} min';
  if (remainingSeconds < 86400) return 'em ${remainingSeconds ~/ 3600} h';
  final days = remainingSeconds ~/ 86400;
  return 'em $days ${days == 1 ? 'dia' : 'dias'}';
}

/// Intervalo entre duas datas: `"15 dias e 4 horas"`, `"1 dia"`, `"menos de 1 minuto"`.
String formatTimeSpan(
  Object? startDate,
  Object? endDate, {
  String fallback = '—',
}) {
  final start = _toDate(startDate);
  final end = _toDate(endDate);
  if (start == null || end == null) return fallback;

  final diffMs = math.max(0, end.difference(start).inMilliseconds);
  final totalMinutes = diffMs ~/ (1000 * 60);
  final totalHours = totalMinutes ~/ 60;
  final days = totalHours ~/ 24;
  final hours = totalHours % 24;
  final minutes = totalMinutes % 60;

  if (days == 0 && totalHours == 0) {
    if (minutes == 0) return 'menos de 1 minuto';
    return '$minutes ${minutes == 1 ? 'minuto' : 'minutos'}';
  }

  if (days == 0) {
    if (minutes == 0) {
      return '$totalHours ${totalHours == 1 ? 'hora' : 'horas'}';
    }
    return '$totalHours ${totalHours == 1 ? 'hora' : 'horas'} e $minutes min';
  }

  final daysLabel = '$days ${days == 1 ? 'dia' : 'dias'}';
  if (hours == 0) return daysLabel;
  final hoursLabel = '$hours ${hours == 1 ? 'hora' : 'horas'}';
  return '$daysLabel e $hoursLabel';
}

/// Concordância simples de contagem: `countNoun(1, 'versão', 'versões')` → `"1 versão"`.
String countNoun(int count, String singular, [String? plural]) {
  final effectivePlural = plural ?? '${singular}s';
  return '$count ${count == 1 ? singular : effectivePlural}';
}
