import 'package:flutter/material.dart';

/// Validadores compartilhados para formulários (DRY — paridade com `formRules.ts`).
class FormValidators {
  FormValidators._();

  static const int minPasswordLength = 8;
  static const int minNameLength = 2;

  static final RegExp _emailRegex = RegExp(r'^[^\s@]+@[^\s@]+\.[^\s@]+$');
  static final RegExp _upperCaseRegex = RegExp(r'[A-Z]');
  static final RegExp _lowerCaseRegex = RegExp(r'[a-z]');
  static final RegExp _digitRegex = RegExp(r'\d');
  static final RegExp _specialCharRegex = RegExp(r'[^\w\s]');
  static final RegExp _envVarRegex = RegExp(r'^[A-Z_][A-Z0-9_]*$');
  static final RegExp _ipv4Regex = RegExp(
    r'^(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}$',
  );
  static final RegExp _hostnameRegex = RegExp(
    r'^(?=.{1,253}$)(?!-)[A-Za-z0-9-]{1,63}(?<!-)(\.(?!-)[A-Za-z0-9-]{1,63}(?<!-))*$',
  );
  static final RegExp _cronFieldRegex = RegExp(r'^[\d*/,\-]+$');

  static FormFieldValidator<String> compose(
    List<FormFieldValidator<String>> validators,
  ) {
    return (value) {
      for (final validator in validators) {
        final error = validator(value);
        if (error != null) return error;
      }
      return null;
    };
  }

  static FormFieldValidator<String> required([
    String message = 'Campo obrigatório.',
  ]) {
    return (value) {
      if (value == null || value.trim().isEmpty) return message;
      return null;
    };
  }

  static FormFieldValidator<String> minLength(int min, String message) {
    return (value) {
      if (value == null || value.trim().length < min) return message;
      return null;
    };
  }

  static FormFieldValidator<String> email([
    String message = 'Informe um e-mail válido.',
  ]) {
    return (value) {
      if (value == null || value.trim().isEmpty) return null;
      return _emailRegex.hasMatch(value.trim()) ? null : message;
    };
  }

  static FormFieldValidator<String> password() {
    return (value) {
      final text = value ?? '';
      if (text.length < minPasswordLength) {
        return 'A senha precisa ter ao menos $minPasswordLength caracteres.';
      }
      if (!_upperCaseRegex.hasMatch(text)) {
        return 'A senha precisa conter ao menos uma letra maiúscula.';
      }
      return null;
    };
  }

  static FormFieldValidator<String> matches(
    String Function() other, [
    String message = 'Os valores não coincidem.',
  ]) {
    return (value) => value == other() ? null : message;
  }

  static FormFieldValidator<String> ipv4([
    String message = 'Informe um endereço IPv4 válido.',
  ]) {
    return (value) {
      final trimmed = value?.trim() ?? '';
      if (trimmed.isEmpty) return null;
      return _ipv4Regex.hasMatch(trimmed) ? null : message;
    };
  }

  static FormFieldValidator<String> ipOrHost([
    String message = 'Informe um IP ou hostname válido.',
  ]) {
    return (value) {
      final trimmed = value?.trim() ?? '';
      if (trimmed.isEmpty) return null;
      if (_ipv4Regex.hasMatch(trimmed) || _hostnameRegex.hasMatch(trimmed)) {
        return null;
      }
      // Suporte simples a IPv6
      if (trimmed.contains(':') && Uri.tryParse('http://[$trimmed]') != null) {
        return null;
      }
      return message;
    };
  }

  static FormFieldValidator<String> cidr([
    String message = 'Informe um bloco CIDR válido (ex.: 192.168.1.0/24).',
  ]) {
    return (value) {
      final trimmed = value?.trim() ?? '';
      if (trimmed.isEmpty) return null;
      final parts = trimmed.split('/');
      if (parts.length != 2) return message;
      if (!_ipv4Regex.hasMatch(parts[0])) return message;
      final prefix = int.tryParse(parts[1]);
      if (prefix == null || prefix < 0 || prefix > 32) return message;
      return null;
    };
  }

  static FormFieldValidator<String> port([
    String message = 'Informe uma porta válida (1 a 65535).',
  ]) {
    return (value) {
      final trimmed = value?.trim() ?? '';
      if (trimmed.isEmpty) return null;
      final parsed = int.tryParse(trimmed);
      if (parsed == null || parsed < 1 || parsed > 65535) return message;
      return null;
    };
  }

  static FormFieldValidator<String> url([
    String message = 'Informe uma URL HTTP/HTTPS válida.',
  ]) {
    return (value) {
      final trimmed = value?.trim() ?? '';
      if (trimmed.isEmpty) return null;
      final uri = Uri.tryParse(trimmed);
      if (uri == null ||
          (uri.scheme != 'http' && uri.scheme != 'https') ||
          uri.host.isEmpty) {
        return message;
      }
      return null;
    };
  }

  static FormFieldValidator<String> cron([
    String message = 'Expressão cron inválida (esperado 5 campos).',
  ]) {
    return (value) {
      final trimmed = value?.trim() ?? '';
      if (trimmed.isEmpty) return null;
      final parts = trimmed.split(RegExp(r'\s+'));
      if (parts.length != 5) return message;
      for (final part in parts) {
        if (!_cronFieldRegex.hasMatch(part)) return message;
      }
      return null;
    };
  }

  static FormFieldValidator<String> envVariable([
    String message = 'Use MAIÚSCULAS, números e _',
  ]) {
    return (value) {
      final trimmed = value?.trim() ?? '';
      if (trimmed.isEmpty) return null;
      return _envVarRegex.hasMatch(trimmed) ? null : message;
    };
  }

  static FormFieldValidator<String> integerRange(
    int min,
    int max, {
    String? message,
  }) {
    return (value) {
      final trimmed = value?.trim() ?? '';
      if (trimmed.isEmpty) return null;
      final parsed = int.tryParse(trimmed);
      if (parsed == null || parsed < min || parsed > max) {
        return message ?? 'Informe um valor entre $min e $max.';
      }
      return null;
    };
  }

  static PasswordStrength evaluatePasswordStrength(String value) {
    if (value.isEmpty) {
      return const PasswordStrength(
        score: 0,
        label: '',
        tone: PasswordStrengthTone.none,
      );
    }
    var score = 0;
    if (value.length >= minPasswordLength) score++;
    if (value.length >= 12) score++;
    if (_lowerCaseRegex.hasMatch(value) && _upperCaseRegex.hasMatch(value)) {
      score++;
    }
    if (_digitRegex.hasMatch(value)) score++;
    if (_specialCharRegex.hasMatch(value)) score++;

    if (score <= 1) {
      return const PasswordStrength(
        score: 1,
        label: 'Fraca',
        tone: PasswordStrengthTone.weak,
      );
    }
    if (score == 2) {
      return const PasswordStrength(
        score: 2,
        label: 'Razoável',
        tone: PasswordStrengthTone.fair,
      );
    }
    if (score == 3) {
      return const PasswordStrength(
        score: 3,
        label: 'Boa',
        tone: PasswordStrengthTone.good,
      );
    }
    return const PasswordStrength(
      score: 4,
      label: 'Forte',
      tone: PasswordStrengthTone.strong,
    );
  }
}

enum PasswordStrengthTone { none, weak, fair, good, strong }

@immutable
class PasswordStrength {
  const PasswordStrength({
    required this.score,
    required this.label,
    required this.tone,
  });

  final int score;
  final String label;
  final PasswordStrengthTone tone;
}
