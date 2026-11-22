import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

class _ScannedDartFile {
  const _ScannedDartFile({
    required this.relPath,
    required this.strippedSource,
    required this.strippedLines,
  });

  final String relPath;
  final String strippedSource;
  final List<String> strippedLines;
}

class _InterpFrame {
  _InterpFrame(this.delimiter);

  final String delimiter;
  int braceDepth = 1;
}

Directory _resolveLibDir() {
  final direct = Directory('lib');
  if (direct.existsSync()) return direct;
  final nested = Directory('frontend/lib');
  if (nested.existsSync()) return nested;
  throw StateError(
    'Diretório lib/ não encontrado a partir de ${Directory.current.path}',
  );
}

/// Remove comentários de linha (`//`, `///`) e de bloco (`/* ... */`)
/// preservando literais de string (simples, triplas, raw e interpolações `${}`)
/// e mantendo a numeração exata de linhas 1-based do arquivo original.
String stripDartCommentsPreservingLines(String source) {
  final out = StringBuffer();
  final interpStack = <_InterpFrame>[];
  String? activeDelimiter;
  var activeIsRaw = false;
  var i = 0;
  final len = source.length;

  while (i < len) {
    if (activeDelimiter != null) {
      if (source.startsWith(activeDelimiter, i)) {
        out.write(activeDelimiter);
        i += activeDelimiter.length;
        activeDelimiter = null;
        activeIsRaw = false;
        continue;
      }
      if (!activeIsRaw) {
        if (source[i] == r'\' && i + 1 < len) {
          out.write(source[i]);
          out.write(source[i + 1]);
          i += 2;
          continue;
        }
        if (source.startsWith(r'${', i)) {
          out.write(r'${');
          interpStack.add(_InterpFrame(activeDelimiter));
          activeDelimiter = null;
          i += 2;
          continue;
        }
      }
      out.write(source[i]);
      i++;
      continue;
    }

    // Comentário de linha (`//` ou `///`)
    if (source.startsWith('//', i)) {
      while (i < len && source[i] != '\n') {
        i++;
      }
      continue;
    }

    // Comentário de bloco (`/* ... */`) preservando quebras de linha
    if (source.startsWith('/*', i)) {
      out.write('  ');
      i += 2;
      while (i < len && !source.startsWith('*/', i)) {
        out.write(source[i] == '\n' ? '\n' : ' ');
        i++;
      }
      if (i < len) {
        out.write('  ');
        i += 2;
      }
      continue;
    }

    // Abertura de string raw (`r'...'`, `r"..."`, `r'''...'''`, `r"""..."""`)
    if ((source[i] == 'r' || source[i] == 'R') &&
        i + 1 < len &&
        (source[i + 1] == "'" || source[i + 1] == '"')) {
      final quote = source[i + 1];
      final triple = quote * 3;
      out.write(source[i]);
      i++;
      if (source.startsWith(triple, i)) {
        activeDelimiter = triple;
        activeIsRaw = true;
        out.write(triple);
        i += 3;
      } else {
        activeDelimiter = quote;
        activeIsRaw = true;
        out.write(quote);
        i++;
      }
      continue;
    }

    // Abertura de string normal (`'...'`, `"..."`, `'''...'''`, `"""..."""`)
    if (source[i] == "'" || source[i] == '"') {
      final quote = source[i];
      final triple = quote * 3;
      if (source.startsWith(triple, i)) {
        activeDelimiter = triple;
        activeIsRaw = false;
        out.write(triple);
        i += 3;
      } else {
        activeDelimiter = quote;
        activeIsRaw = false;
        out.write(quote);
        i++;
      }
      continue;
    }

    // Controle de chaves dentro de interpolação `${ ... }`
    if (interpStack.isNotEmpty) {
      if (source[i] == '{') {
        interpStack.last.braceDepth++;
        out.write('{');
        i++;
        continue;
      }
      if (source[i] == '}') {
        interpStack.last.braceDepth--;
        out.write('}');
        i++;
        if (interpStack.last.braceDepth == 0) {
          activeDelimiter = interpStack.removeLast().delimiter;
          activeIsRaw = false;
        }
        continue;
      }
    }

    out.write(source[i]);
    i++;
  }

  return out.toString();
}

/// Resolve um URI de `import`/`export`/`part` para o caminho normalizado em
/// `lib/...` (ou retorna `null` para SDK `dart:*` e pacotes externos).
String? resolveImportedLibPath(String importerRelPath, String uri) {
  final trimmed = uri.trim();
  if (trimmed.isEmpty || trimmed.startsWith('dart:')) {
    return null;
  }
  const pkgPrefix = 'package:netmonitor_app/';
  if (trimmed.startsWith(pkgPrefix)) {
    return _normalizePosixPath('lib/${trimmed.substring(pkgPrefix.length)}');
  }
  if (trimmed.startsWith('package:') || trimmed.contains(':')) {
    return null;
  }
  final slashIdx = importerRelPath.lastIndexOf('/');
  final baseDir = slashIdx >= 0 ? importerRelPath.substring(0, slashIdx) : '';
  final combined = baseDir.isEmpty ? trimmed : '$baseDir/$trimmed';
  return _normalizePosixPath(combined);
}

String _normalizePosixPath(String rawPath) {
  final parts = rawPath.replaceAll(r'\', '/').split('/');
  final stack = <String>[];
  for (final part in parts) {
    if (part.isEmpty || part == '.') continue;
    if (part == '..') {
      if (stack.isNotEmpty) stack.removeLast();
    } else {
      stack.add(part);
    }
  }
  return stack.join('/');
}

int _lineNumberAt(String source, int offset) {
  var line = 1;
  for (var i = 0; i < offset && i < source.length; i++) {
    if (source.codeUnitAt(i) == 0x0A) {
      line++;
    }
  }
  return line;
}

bool _isVisualLayerPath(String relPath) {
  if (relPath.startsWith('lib/features/core/presentation/') ||
      relPath.startsWith('lib/features/auth/') ||
      relPath.startsWith('lib/shared/widgets/')) {
    return true;
  }
  if (relPath.startsWith('lib/features/')) {
    return relPath.contains('/pages/') || relPath.contains('/widgets/');
  }
  return false;
}

List<String>? _forbiddenFeatureTargetsFor(String relPath) {
  if (relPath.startsWith('lib/core/') ||
      relPath.startsWith('lib/shared/') ||
      relPath.startsWith('lib/theme/') ||
      relPath.startsWith('lib/features/auth/') ||
      relPath.startsWith('lib/features/core/')) {
    return const <String>['network', 'servers', 'plugins', 'vpn', 'ai'];
  }
  if (relPath.startsWith('lib/features/network/')) {
    return const <String>['servers', 'plugins', 'vpn', 'ai'];
  }
  if (relPath.startsWith('lib/features/servers/')) {
    return const <String>['network', 'plugins', 'vpn', 'ai'];
  }
  if (relPath.startsWith('lib/features/plugins/')) {
    return const <String>['network', 'servers', 'vpn', 'ai'];
  }
  if (relPath.startsWith('lib/features/vpn/')) {
    return const <String>['network', 'servers', 'plugins', 'ai'];
  }
  if (relPath.startsWith('lib/features/ai/')) {
    return const <String>['network', 'servers', 'plugins', 'vpn'];
  }
  return null;
}

void main() {
  late final List<_ScannedDartFile> dartFiles;

  setUpAll(() {
    final libDir = _resolveLibDir();
    final libRootPath = libDir.absolute.path.replaceAll(r'\', '/');
    final prefixLen = libRootPath.length - 'lib'.length;

    final entities =
        libDir
            .listSync(recursive: true, followLinks: false)
            .whereType<File>()
            .where((f) => f.path.endsWith('.dart'))
            .toList()
          ..sort((a, b) => a.path.compareTo(b.path));

    dartFiles = entities.map((file) {
      final absNormalized = file.absolute.path.replaceAll(r'\', '/');
      final relPath = absNormalized.substring(prefixLen);
      final raw = file.readAsStringSync().replaceAll('\r\n', '\n');
      final stripped = stripDartCommentsPreservingLines(raw);
      return _ScannedDartFile(
        relPath: relPath,
        strippedSource: stripped,
        strippedLines: stripped.split('\n'),
      );
    }).toList();
  });

  group('SOLID, DRY & ADR 017 Architectural Conventions', () {
    test('carrega todos os arquivos .dart de lib/', () {
      expect(dartFiles.length, greaterThanOrEqualTo(250));
    });

    test(
      'Zero Polling (AGENTS.md §9): nenhuma ocorrência de Timer.periodic( em lib/',
      () {
        final violations = <String>[];
        for (final file in dartFiles) {
          for (var i = 0; i < file.strippedLines.length; i++) {
            final line = file.strippedLines[i];
            if (line.contains('Timer.periodic(')) {
              violations.add('${file.relPath}:${i + 1}: ${line.trim()}');
            }
          }
        }
        expect(
          violations,
          isEmpty,
          reason:
              'Polling com Timer.periodic é proibido (AGENTS.md §9):\n'
              '${violations.join('\n')}',
        );
      },
    );

    test(
      'Dropdown Canônico (DRY): DropdownButtonFormField apenas em app_dropdown_form_field.dart',
      () {
        const allowedPath = 'lib/shared/widgets/app_dropdown_form_field.dart';
        final pattern = RegExp(r'\bDropdownButtonFormField\b');
        final violations = <String>[];

        for (final file in dartFiles) {
          if (file.relPath == allowedPath) continue;
          for (var i = 0; i < file.strippedLines.length; i++) {
            final line = file.strippedLines[i];
            if (pattern.hasMatch(line)) {
              violations.add('${file.relPath}:${i + 1}: ${line.trim()}');
            }
          }
        }
        expect(
          violations,
          isEmpty,
          reason:
              'Use AppDropdownFormField em vez de DropdownButtonFormField direto:\n'
              '${violations.join('\n')}',
        );
      },
    );

    test(
      'Sliver Header Canônico (DRY): extends SliverPersistentHeaderDelegate apenas em pinned_sliver_header.dart',
      () {
        const allowedPath = 'lib/shared/widgets/pinned_sliver_header.dart';
        final pattern = RegExp(r'\bextends\s+SliverPersistentHeaderDelegate\b');
        final violations = <String>[];

        for (final file in dartFiles) {
          if (file.relPath == allowedPath) continue;
          for (final match in pattern.allMatches(file.strippedSource)) {
            final lineNo = _lineNumberAt(file.strippedSource, match.start);
            violations.add(
              '${file.relPath}:$lineNo: ${file.strippedLines[lineNo - 1].trim()}',
            );
          }
        }
        expect(
          violations,
          isEmpty,
          reason:
              'Use PinnedSliverHeaderDelegate em vez de criar delegates duplicados:\n'
              '${violations.join('\n')}',
        );
      },
    );

    test(
      'Isolamento Estrito entre Áreas (ADR 017 / DIP): zero imports cruzados proibidos',
      () {
        final directiveStatementPattern = RegExp(
          r'''^\s*(?:import|export|part(?:\s+of)?)\s+['"][^;]*;''',
          multiLine: true,
        );
        final quotedUriPattern = RegExp(r'''['"]([^'"]+)['"]''');
        final violations = <String>[];

        for (final file in dartFiles) {
          final forbiddenAreas = _forbiddenFeatureTargetsFor(file.relPath);
          if (forbiddenAreas == null) continue;

          for (final stmtMatch in directiveStatementPattern.allMatches(
            file.strippedSource,
          )) {
            final stmt = stmtMatch.group(0)!;
            for (final uriMatch in quotedUriPattern.allMatches(stmt)) {
              final uri = uriMatch.group(1);
              if (uri == null) continue;
              final targetLibPath = resolveImportedLibPath(file.relPath, uri);
              if (targetLibPath == null) continue;

              for (final forbidden in forbiddenAreas) {
                if (targetLibPath.startsWith('lib/features/$forbidden/')) {
                  final lineNo = _lineNumberAt(
                    file.strippedSource,
                    stmtMatch.start + uriMatch.start,
                  );
                  violations.add(
                    '${file.relPath}:$lineNo importa $targetLibPath '
                    '(área proibida: features/$forbidden/)',
                  );
                }
              }
            }
          }
        }

        expect(
          violations,
          isEmpty,
          reason:
              'Violação de fronteira modular entre áreas (ADR 017 / DIP):\n'
              '${violations.join('\n')}',
        );
      },
    );

    test(
      'Zero apiClientProvider na camada visual (SRP / DIP — AGENTS.md §10–11)',
      () {
        final pattern = RegExp(r'\bapiClientProvider\b');
        final violations = <String>[];

        for (final file in dartFiles) {
          if (!_isVisualLayerPath(file.relPath)) continue;
          for (var i = 0; i < file.strippedLines.length; i++) {
            final line = file.strippedLines[i];
            if (pattern.hasMatch(line)) {
              violations.add('${file.relPath}:${i + 1}: ${line.trim()}');
            }
          }
        }

        expect(
          violations,
          isEmpty,
          reason:
              'Camada visual não pode acessar apiClientProvider diretamente '
              '(delegue para Notifier/Repository):\n'
              '${violations.join('\n')}',
        );
      },
    );

    test(
      'Formatadores Canônicos (AGENTS.md §11): zero .toStringAsFixed( fora de formatters.dart e topology_export_svg.dart',
      () {
        final violations = <String>[];

        for (final file in dartFiles) {
          if (file.relPath == 'lib/core/utils/formatters.dart' ||
              file.relPath.endsWith('/topology_export_svg.dart')) {
            continue;
          }
          for (var i = 0; i < file.strippedLines.length; i++) {
            final line = file.strippedLines[i];
            if (line.contains('.toStringAsFixed(')) {
              violations.add('${file.relPath}:${i + 1}: ${line.trim()}');
            }
          }
        }

        expect(
          violations,
          isEmpty,
          reason:
              'Use os formatadores canônicos de lib/core/utils/formatters.dart '
              'em vez de .toStringAsFixed(:\n'
              '${violations.join('\n')}',
        );
      },
    );

    test(
      'Erros Canônicos (AGENTS.md §11): zero err.toString() ou error.toString() em lib/features/**',
      () {
        final pattern = RegExp(r'\b(?:err|error)\.toString\(\s*\)');
        final violations = <String>[];

        for (final file in dartFiles) {
          if (!file.relPath.startsWith('lib/features/')) continue;
          for (var i = 0; i < file.strippedLines.length; i++) {
            final line = file.strippedLines[i];
            if (pattern.hasMatch(line)) {
              violations.add('${file.relPath}:${i + 1}: ${line.trim()}');
            }
          }
        }

        expect(
          violations,
          isEmpty,
          reason:
              'Use describeError(err) ou formatDeviceAccessError(err) em vez de '
              'err.toString() / error.toString() em lib/features/**:\n'
              '${violations.join('\n')}',
        );
      },
    );

    test(
      'Auto-verificação do scanner estático (comentários, interpolações e imports relativos)',
      () {
        const sample = '''
/// Doc mencionando Timer.periodic( e DropdownButtonFormField
// import 'package:netmonitor_app/features/vpn/models/vpn_models.dart';
final url = 'https://localhost:8080/api'; // err.toString()
/* Bloco
   Timer.periodic(Duration(seconds: 1), (_) {});
*/
final msg = 'Falha: \${error.toString()}';
''';
        final stripped = stripDartCommentsPreservingLines(sample);
        final lines = stripped.split('\n');
        expect(lines.length, sample.split('\n').length);
        expect(stripped.contains('Timer.periodic('), isFalse);
        expect(stripped.contains('DropdownButtonFormField'), isFalse);
        expect(stripped.contains('vpn_models.dart'), isFalse);
        expect(stripped.contains('https://localhost:8080/api'), isTrue);
        expect(lines[6].contains('error.toString()'), isTrue);

        expect(
          resolveImportedLibPath(
            'lib/features/network/pages/devices_page.dart',
            '../../vpn/models/vpn_models.dart',
          ),
          'lib/features/vpn/models/vpn_models.dart',
        );
        expect(
          resolveImportedLibPath(
            'lib/core/utils/formatters.dart',
            'package:netmonitor_app/features/ai/models/ai_models.dart',
          ),
          'lib/features/ai/models/ai_models.dart',
        );
      },
    );
  });
}
