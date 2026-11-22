import 'package:netmonitor_app/core/network/api_client.dart' show ApiException;
import 'package:netmonitor_app/shared/device_access/device_access.dart'
    as shared_access;

export 'package:netmonitor_app/shared/device_access/device_access.dart'
    hide formatDeviceAccessError;

/// Formata falhas de acesso ao equipamento em PT-BR legível, traduzindo mensagens
/// HTTP genéricas em inglês (`Bad Request` / `Unprocessable Entity`) quando não há
/// detalhes adicionais e delegando os demais casos ao helper canônico compartilhado.
String formatDeviceAccessError(Object err, {String? platformOrOs}) {
  if (err is ApiException) {
    final lower = err.message.trim().toLowerCase();
    final isBareBadRequest =
        lower == 'bad request' || lower == 'unprocessable entity';
    final hasExtraDetails =
        (err.payload?.description?.trim().isNotEmpty ?? false) ||
        err.fieldErrors.isNotEmpty;
    if (isBareBadRequest && !hasExtraDetails) {
      return 'Dados de acesso inválidos. Verifique usuário, senha e porta.';
    }
  }
  return shared_access.formatDeviceAccessError(err, platformOrOs: platformOrOs);
}
