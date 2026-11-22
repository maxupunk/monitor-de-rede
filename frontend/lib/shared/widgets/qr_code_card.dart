import 'package:flutter/material.dart';
import 'package:netmonitor_app/theme/app_theme.dart';
import 'package:qr_flutter/qr_flutter.dart';

/// Container padronizado de alto contraste para exibição de QR Codes (VPN, Wi-Fi,
/// agentes), garantindo leitura óptica confiável tanto no tema claro quanto escuro.
class QrCodeCard extends StatelessWidget {
  const QrCodeCard({
    super.key,
    required this.data,
    this.size = 216,
    this.caption,
    this.padding = const EdgeInsets.all(16),
  });

  final String data;
  final double size;
  final String? caption;
  final EdgeInsetsGeometry padding;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Container(
        padding: padding,
        decoration: BoxDecoration(
          color: Colors.white,
          borderRadius: AppTheme.borderRadius,
          border: Border.all(color: context.colorScheme.outlineVariant),
        ),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            QrImageView(
              data: data,
              version: QrVersions.auto,
              size: size,
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
            if (caption != null && caption!.trim().isNotEmpty) ...<Widget>[
              const SizedBox(height: 8),
              Text(
                caption!,
                textAlign: TextAlign.center,
                style: const TextStyle(
                  color: Color(0xFF0F172A),
                  fontWeight: FontWeight.w700,
                  fontSize: 13,
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}
