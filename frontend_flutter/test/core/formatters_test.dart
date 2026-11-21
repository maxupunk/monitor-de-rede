import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';

void main() {
  group('formatBytes / formatBinaryBytes / formatDecimalBytes', () {
    test('formats binary bytes with base 1024 and strips trailing zeros', () {
      expect(formatBytes(0), '0 B');
      expect(formatBytes(null), '0 B');
      expect(formatBytes(null, fallback: '—'), '—');
      expect(formatBinaryBytes(1536), '1.5 KiB');
      expect(formatBinaryBytes(1024 * 1024), '1 MiB');
      expect(formatBinaryBytes(1024 * 1024 * 1024 * 2.5), '2.5 GiB');
    });

    test('formats decimal bytes with base 1000', () {
      expect(formatDecimalBytes(1500), '1.5 KB');
      expect(formatDecimalBytes(1000 * 1000), '1 MB');
      expect(formatDecimalBytes(double.nan, fallback: 'N/A'), 'N/A');
    });
  });

  group('formatBps / formatLinkSpeed / speed units', () {
    test('formats bitrates with base 1000', () {
      expect(formatBps(0), '0 bps');
      expect(formatBps(1500000), '1.5 Mbps');
      expect(formatBps(1000000000), '1 Gbps');
    });

    test('formats negotiated link speed with N/A for 0 or negative', () {
      expect(formatLinkSpeed(null), 'N/A');
      expect(formatLinkSpeed(0), 'N/A');
      expect(formatLinkSpeed(-100), 'N/A');
      expect(formatLinkSpeed(1000000000), '1 Gbps');
    });

    test('resolves and converts speed units accurately', () {
      expect(resolveAutoUnit(0.5), SpeedUnit.kbps);
      expect(resolveAutoUnit(100), SpeedUnit.mbps);
      expect(resolveAutoUnit(2500), SpeedUnit.gbps);
      expect(formatSpeedByUnit(0.5), '500');
      expect(getSpeedUnitLabel(SpeedUnit.auto, valueMbps: 0.5), 'Kbps');
      expect(formatSpeedByUnit(1500), '1.5');
      expect(getSpeedUnitLabel(SpeedUnit.auto, valueMbps: 1500), 'Gbps');
      expect(formatSpeedMbps(12.34), '12.3');
      expect(formatSpeedMbps(null), '--');
    });
  });

  group('formatLatency / formatPercent / counts / elapsed', () {
    test('formats latency with 1 decimal below 100ms and integer above', () {
      expect(formatLatency(null), 'N/A');
      expect(formatLatency(6.9038089), '6.9 ms');
      expect(formatLatency(7.0), '7 ms');
      expect(formatLatency(250.4), '250 ms');
    });

    test('formats percent and integers in pt-BR', () {
      expect(formatPercent(42.5), '42,5%');
      expect(formatPercent(null), '—');
      expect(formatInteger(2210), '2.210');
      expect(formatCompactCount(850), '850');
      expect(formatCompactCount(1200), '1,2 mil');
      expect(formatCompactCount(3400000), '3,4 mi');
    });

    test('formats context window, hour, token rate, elapsed ms', () {
      expect(formatContextWindow(262144), '256k');
      expect(formatContextWindow(16384, detailed: true), '16.384 tokens (16k)');
      expect(formatHourOfDay(8), '08:00');
      expect(formatTokenRate(42.34), '42,3 tok/s');
      expect(formatElapsedMs(850), '850 ms');
      expect(formatElapsedMs(4200), '4,2 s');
      expect(formatElapsedMs(72000), '1 min 12 s');
      expect(countNoun(1, 'versão', 'versões'), '1 versão');
      expect(countNoun(3, 'versão', 'versões'), '3 versões');
    });
  });

  group('date and time formatters', () {
    test('formats date, dateTime, shortDateTime, clockTime', () {
      final dt = DateTime(2026, 8, 5, 14, 32, 7);
      expect(formatDate(dt), '05/08/2026');
      expect(formatDateTime(dt), '05/08/2026 14:32:07');
      expect(formatShortDateTime(dt), '05/08 14:32:07');
      expect(formatClockTime(dt), '14:32:07');
      expect(formatDate(null), '—');
    });

    test('formats relative time and time span', () {
      final now = DateTime(2026, 8, 5, 14, 0, 0);
      expect(
        formatRelativeTime(now.subtract(const Duration(seconds: 30)), now: now),
        'há 30s',
      );
      expect(
        formatRelativeTime(now.subtract(const Duration(minutes: 5)), now: now),
        'há 5 min',
      );
      expect(
        formatRelativeTime(now.subtract(const Duration(days: 3)), now: now),
        'há 3 dias',
      );
      expect(formatRelativeTime(null), 'nunca');

      expect(
        formatTimeSpan(
          DateTime(2026, 8, 1, 10, 0),
          DateTime(2026, 8, 1, 10, 0, 30),
        ),
        'menos de 1 minuto',
      );
      expect(
        formatTimeSpan(
          DateTime(2026, 8, 1, 10, 0),
          DateTime(2026, 8, 16, 14, 0),
        ),
        '15 dias e 4 horas',
      );
      expect(
        formatTimeSpan(
          DateTime(2026, 8, 1, 10, 0),
          DateTime(2026, 8, 2, 10, 0),
        ),
        '1 dia',
      );
    });
  });
}
