import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/device_defaults.dart';
import 'package:netmonitor_app/core/utils/form_validators.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/core/models/log_models.dart';
import 'package:netmonitor_app/features/core/models/settings_models.dart'
    show ServerAddressEntry;
import 'package:netmonitor_app/features/core/providers/logs_provider.dart';
import 'package:netmonitor_app/features/core/providers/settings_provider.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/features/network/models/diagnostics_models.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/features/network/providers/sites_networks_provider.dart';
import 'package:netmonitor_app/features/network/widgets/device_access_host.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

const List<(String, String)> deviceTypeOptions = <(String, String)>[
  ('router', 'Roteador'),
  ('switch', 'Switch'),
  ('access_point', 'Access Point'),
  ('firewall', 'Firewall'),
  ('server', 'Servidor'),
  ('workstation', 'Estação de trabalho'),
  ('nas', 'NAS / Storage'),
  ('printer', 'Impressora'),
  ('camera', 'Câmera'),
  ('iot', 'IoT'),
  ('vm', 'Máquina Virtual'),
  ('other', 'Outro'),
];

String deviceTypeLabel(String type) {
  for (final item in deviceTypeOptions) {
    if (item.$1 == type) return item.$2;
  }
  return type;
}

IconData deviceTypeIcon(String type) {
  return switch (type.toLowerCase()) {
    'router' => Icons.router_outlined,
    'switch' => Icons.hub_outlined,
    'access_point' => Icons.wifi_tethering,
    'firewall' => Icons.security_outlined,
    'server' => Icons.dns_outlined,
    'workstation' => Icons.computer_outlined,
    'nas' => Icons.storage_outlined,
    'printer' => Icons.print_outlined,
    'camera' => Icons.videocam_outlined,
    'iot' => Icons.sensors_outlined,
    'vm' => Icons.cloud_queue_outlined,
    _ => Icons.devices_other_outlined,
  };
}

/// Abre o formulário adaptativo (bottom sheet em mobile / dialog em desktop)
/// para criar, editar ou adotar um dispositivo descoberto.
Future<DeviceModel?> showDeviceFormSheet({
  required BuildContext context,
  DeviceModel? existing,
  Map<String, dynamic>? prefill,
}) {
  return showAdaptiveFormSheet<DeviceModel>(
    context: context,
    title: existing != null ? 'Editar dispositivo' : 'Novo dispositivo',
    subtitle: existing != null
        ? existing.name
        : 'Cadastre identidade, rede, sistema e telemetria SNMP.',
    maxWidth: 680,
    builder: (ctx) => _DeviceFormBody(
      existing: existing,
      prefill: prefill,
      onDone: (saved) => Navigator.of(ctx).pop(saved),
    ),
  );
}

class _DeviceFormBody extends ConsumerStatefulWidget {
  const _DeviceFormBody({this.existing, this.prefill, required this.onDone});

  final DeviceModel? existing;
  final Map<String, dynamic>? prefill;
  final ValueChanged<DeviceModel> onDone;

  @override
  ConsumerState<_DeviceFormBody> createState() => _DeviceFormBodyState();
}

class _DeviceFormBodyState extends ConsumerState<_DeviceFormBody> {
  final _formKey = GlobalKey<FormState>();

  late final TextEditingController _nameCtrl;
  late final TextEditingController _ipCtrl;
  late final TextEditingController _macCtrl;
  late final TextEditingController _vendorCtrl;
  late final TextEditingController _modelCtrl;
  late final TextEditingController _locationCtrl;
  late final TextEditingController _descCtrl;
  late final TextEditingController _communityCtrl;
  late final TextEditingController _pollIntervalCtrl;
  late final TextEditingController _linkIfaceCtrl;
  final TextEditingController _sshAdminUserCtrl = TextEditingController();
  final TextEditingController _sshAdminPassCtrl = TextEditingController();
  final TextEditingController _sshDedicatedPortCtrl = TextEditingController(
    text: '22',
  );
  final TextEditingController _oneTimeUserCtrl = TextEditingController();
  final TextEditingController _oneTimePassCtrl = TextEditingController();
  final TextEditingController _oneTimePortCtrl = TextEditingController(
    text: '22',
  );
  final TextEditingController _syslogServerAddrCtrl = TextEditingController();

  String _type = 'router';
  int? _siteId;
  int? _networkId;
  int? _parentId;
  String? _operatingSystem;
  String? _accessMode;
  bool _isMonitored = true;
  bool _snmpEnabled = false;
  bool _syslogEnabled = false;
  SyslogSetupMode _syslogMode = SyslogSetupMode.automatic;
  SyslogAccessMethod _syslogAccessMethod = SyslogAccessMethod.dedicatedSshKey;
  bool _reuseExistingSyslogAccess = false;
  bool _reuseChoiceTouched = false;
  bool _sshAdminUserEdited = false;
  bool _oneTimeUserEdited = false;
  String _oneTimeProtocol = 'ssh';
  String? _selectedSyslogAddress;
  String _snmpVersion = 'v2c';

  bool _saving = false;
  bool _identifying = false;
  bool _autoIdentifying = false;
  bool _userPickedOs = false;
  bool _testingSnmp = false;
  bool _loadingIfaces = false;
  bool _loadingSyslogGuide = false;
  bool _provisioningSyslog = false;
  bool _ifacesQueried = false;
  bool _manualLinkIface = false;
  int? _manualSyslogSnippetIndex;
  String? _error;
  String? _syslogProvisionStep;
  String? _syslogProvisionError;
  DeviceModel? _createdDevice;
  ProvisionLoggingResponse? _syslogProvisionResult;
  ProvisionHintsResponse? _syslogHints;
  DeviceAccessViewModel? _existingAccessView;
  IdentifyResultModel? _identifyResult;
  SnmpTestResult? _snmpResult;
  MacVendorLookup? _macLookup;
  List<DeviceInterfaceModel> _discoveredIfaces = const <DeviceInterfaceModel>[];
  Timer? _autoIdentifyDebounce;
  String _lastAutoIdentifyKey = '';
  int _identifyRequestSeq = 0;

  int? get _effectiveDeviceId => widget.existing?.id ?? _createdDevice?.id;

  DeviceCredentialView? get _reusableSyslogCredential {
    final creds =
        _existingAccessView?.credentials ?? const <DeviceCredentialView>[];
    for (final c in creds) {
      if (c.kind == 'ssh' && (c.dedicated || c.storage == 'key')) return c;
    }
    for (final c in creds) {
      if (c.kind == 'ssh' || c.kind == 'telnet') return c;
    }
    return null;
  }

  String? _resolvedEffectiveOsId() =>
      _operatingSystem ??
      _identifyResult?.operatingSystem ??
      _syslogHints?.operatingSystem ??
      widget.existing?.effectiveOperatingSystem;

  String? _resolveAuthPlatformHint() {
    final os = _resolvedEffectiveOsId()?.trim();
    if (os != null && os.isNotEmpty) return os;
    final accessPlat = _existingAccessView?.dedicatedPlatform?.trim();
    if (accessPlat != null && accessPlat.isNotEmpty) return accessPlat;
    final vendor = _vendorCtrl.text.trim();
    if (vendor.isNotEmpty) return vendor;
    final model = _modelCtrl.text.trim();
    if (model.isNotEmpty) return model;
    return null;
  }

  void _syncDefaultDeviceUsernames() {
    final suggested = defaultDeviceAdminUsername(_resolveAuthPlatformHint());
    if (!_sshAdminUserEdited && _sshAdminUserCtrl.text != suggested) {
      _sshAdminUserCtrl.text = suggested;
    }
    if (!_oneTimeUserEdited && _oneTimeUserCtrl.text != suggested) {
      _oneTimeUserCtrl.text = suggested;
    }
  }

  @override
  void initState() {
    super.initState();
    final d = widget.existing;
    final p = widget.prefill ?? const <String, dynamic>{};

    _nameCtrl = TextEditingController(
      text: d?.name ?? (p['name'] as String?) ?? '',
    );
    _ipCtrl = TextEditingController(
      text: d?.ipAddress ?? (p['ipAddress'] as String?) ?? '',
    );
    _macCtrl = TextEditingController(
      text: d?.macAddress ?? (p['macAddress'] as String?) ?? '',
    );
    _vendorCtrl = TextEditingController(
      text: d?.vendor ?? (p['vendor'] as String?) ?? '',
    );
    _modelCtrl = TextEditingController(
      text: d?.model ?? (p['model'] as String?) ?? '',
    );
    _locationCtrl = TextEditingController(text: d?.location ?? '');
    _descCtrl = TextEditingController(text: d?.description ?? '');
    _communityCtrl = TextEditingController(text: d?.snmpCommunity ?? 'public');
    _pollIntervalCtrl = TextEditingController(
      text: (d?.snmpPollIntervalSeconds ?? 60).toString(),
    );
    _linkIfaceCtrl = TextEditingController(text: d?.linkInterfaceName ?? '');

    _type = d?.type ?? (p['type'] as String?) ?? 'router';
    if (!deviceTypeOptions.any((e) => e.$1 == _type)) _type = 'other';
    _siteId = d?.siteId ?? (p['siteId'] as int?);
    _networkId = d?.networkId ?? (p['networkId'] as int?);
    _parentId = d?.parentId;
    _operatingSystem = d?.operatingSystem ?? (p['operatingSystem'] as String?);
    _userPickedOs =
        _operatingSystem != null && _operatingSystem!.trim().isNotEmpty;
    _accessMode = d?.accessMode;
    _isMonitored = d?.isMonitored ?? true;
    _snmpEnabled = d?.snmpEnabled ?? false;
    _snmpVersion = d?.snmpVersion ?? 'v2c';
    _syncDefaultDeviceUsernames();

    Future<void>.microtask(() {
      ref.read(sitesProvider.notifier).fetchSites();
      ref.read(networksProvider.notifier).fetchNetworks();
      ref.read(operatingSystemsProvider.notifier).fetchAll();
      ref.read(snmpProfilesProvider.notifier).fetchProfiles();
      if (_macCtrl.text.trim().length >= 6) _lookupMac(_macCtrl.text);
      if (widget.existing == null && !_userPickedOs && _hasIdentifySignals()) {
        _identifyDevice(automatic: true);
      }
    });
  }

  @override
  void dispose() {
    _autoIdentifyDebounce?.cancel();
    _sshAdminPassCtrl.clear();
    _oneTimePassCtrl.clear();
    _nameCtrl.dispose();
    _ipCtrl.dispose();
    _macCtrl.dispose();
    _vendorCtrl.dispose();
    _modelCtrl.dispose();
    _locationCtrl.dispose();
    _descCtrl.dispose();
    _communityCtrl.dispose();
    _pollIntervalCtrl.dispose();
    _linkIfaceCtrl.dispose();
    _sshAdminUserCtrl.dispose();
    _sshAdminPassCtrl.dispose();
    _sshDedicatedPortCtrl.dispose();
    _oneTimeUserCtrl.dispose();
    _oneTimePassCtrl.dispose();
    _oneTimePortCtrl.dispose();
    _syslogServerAddrCtrl.dispose();
    super.dispose();
  }

  bool _hasIdentifySignals() {
    final ip = _ipCtrl.text.trim();
    final vendor = _vendorCtrl.text.trim();
    final model = _modelCtrl.text.trim();
    final macHex = _macCtrl.text.replaceAll(RegExp(r'[^0-9a-fA-F]'), '');
    return ip.isNotEmpty ||
        vendor.isNotEmpty ||
        model.isNotEmpty ||
        macHex.length >= 6;
  }

  String _identifySignature() {
    return <String>[
      _nameCtrl.text.trim().toLowerCase(),
      _ipCtrl.text.trim().toLowerCase(),
      _vendorCtrl.text.trim().toLowerCase(),
      _modelCtrl.text.trim().toLowerCase(),
      _snmpEnabled ? _snmpVersion : '',
      _snmpEnabled ? _communityCtrl.text.trim() : '',
    ].join('|');
  }

  void _scheduleAutoIdentify() {
    if (_userPickedOs) return;
    _autoIdentifyDebounce?.cancel();
    if (!_hasIdentifySignals()) return;
    _autoIdentifyDebounce = Timer(const Duration(milliseconds: 600), () {
      if (!mounted || _userPickedOs || !_hasIdentifySignals()) return;
      _identifyDevice(automatic: true);
    });
  }

  Future<void> _lookupMac(String mac) async {
    final result = await ref.read(vendorsProvider.notifier).lookup(mac);
    if (!mounted || result == null) return;
    setState(() {
      _macLookup = result;
      if (_vendorCtrl.text.trim().isEmpty && result.vendor != null) {
        _vendorCtrl.text = result.vendor!;
      }
      if (widget.existing == null &&
          result.hintDeviceType != null &&
          deviceTypeOptions.any((o) => o.$1 == result.hintDeviceType)) {
        _type = result.hintDeviceType!;
      }
      _syncDefaultDeviceUsernames();
    });
    _scheduleAutoIdentify();
  }

  Future<void> _identifyDevice({bool automatic = false}) async {
    if (automatic && _userPickedOs) return;
    final signature = _identifySignature();
    if (automatic && signature == _lastAutoIdentifyKey) return;
    if (automatic) _lastAutoIdentifyKey = signature;

    final requestSeq = ++_identifyRequestSeq;
    setState(() {
      if (automatic) {
        _autoIdentifying = true;
      } else {
        _identifying = true;
        _error = null;
      }
    });
    try {
      final res = await ref
          .read(operatingSystemsProvider.notifier)
          .identify(
            name: _nameCtrl.text.trim().isEmpty ? null : _nameCtrl.text.trim(),
            ipAddress: _ipCtrl.text.trim().isEmpty ? null : _ipCtrl.text.trim(),
            snmpVersion: _snmpEnabled ? _snmpVersion : null,
            snmpCommunity: _snmpEnabled ? _communityCtrl.text.trim() : null,
            vendor: _vendorCtrl.text.trim().isEmpty
                ? null
                : _vendorCtrl.text.trim(),
            model: _modelCtrl.text.trim().isEmpty
                ? null
                : _modelCtrl.text.trim(),
          );
      if (!mounted || requestSeq != _identifyRequestSeq) return;
      if (automatic && _userPickedOs) return;
      setState(() {
        _identifyResult = res;
        if (res?.operatingSystem != null) {
          _operatingSystem = res!.operatingSystem;
          _manualSyslogSnippetIndex = null;
        }
        if (_vendorCtrl.text.trim().isEmpty && res?.vendor != null) {
          _vendorCtrl.text = res!.vendor!;
        }
        if (_modelCtrl.text.trim().isEmpty && res?.model != null) {
          _modelCtrl.text = res!.model!;
        }
        _syncDefaultDeviceUsernames();
      });
    } catch (err) {
      if (!automatic && mounted && requestSeq == _identifyRequestSeq) {
        setState(() => _error = err.toString());
      }
    } finally {
      if (mounted && requestSeq == _identifyRequestSeq) {
        setState(() {
          if (automatic) {
            _autoIdentifying = false;
          } else {
            _identifying = false;
          }
        });
      }
    }
  }

  Future<void> _loadSyslogGuide({
    bool force = false,
    String? overrideAddress,
  }) async {
    setState(() => _loadingSyslogGuide = true);
    try {
      final deviceId = _effectiveDeviceId;
      ProvisionHintsResponse? hints = _syslogHints;
      DeviceAccessViewModel? accessView = _existingAccessView;

      await Future.wait<void>(<Future<void>>[
        () async {
          try {
            await ref
                .read(coreSettingsProvider.notifier)
                .fetchServerAddresses(force: force);
          } catch (_) {}
        }(),
        if (deviceId != null && (force || hints == null))
          () async {
            try {
              hints = await ref
                  .read(logsProvider.notifier)
                  .fetchProvisionHints(deviceId);
            } catch (_) {}
          }(),
        if (deviceId != null && (force || accessView == null))
          () async {
            try {
              accessView = await ref
                  .read(deviceAccessProvider.notifier)
                  .fetchAccess(deviceId);
            } catch (_) {}
          }(),
      ]);

      if (!mounted) return;
      final settings = ref.read(coreSettingsProvider);
      final currentGuide = ref.read(logsProvider).setupGuide;
      final resolvedAddress = DeviceSyslogSetupPanel.resolveBestServerAddress(
        selectedAddress: overrideAddress ?? _selectedSyslogAddress,
        hintedAddress: hints?.serverAddress,
        usableAddresses: settings.usableAddresses,
        preferredAddressId: settings.preferredAddressId,
        guideAddress: currentGuide?.serverAddress,
      );

      setState(() {
        _syslogHints = hints;
        _existingAccessView = accessView;
        if (resolvedAddress != null) {
          _selectedSyslogAddress ??= resolvedAddress;
          if (_syslogServerAddrCtrl.text.trim().isEmpty) {
            _syslogServerAddrCtrl.text = resolvedAddress;
          }
        }
        if (hints != null) {
          if (!hints!.sshOpen && hints!.telnetOpen) {
            _oneTimeProtocol = 'telnet';
            if (_oneTimePortCtrl.text.trim() == '22') {
              _oneTimePortCtrl.text = '23';
            }
          }
        }
        if (!_reuseChoiceTouched) {
          _reuseExistingSyslogAccess = _reusableSyslogCredential != null;
        }
        _syncDefaultDeviceUsernames();
      });

      await ref
          .read(logsProvider.notifier)
          .fetchSetupGuide(address: resolvedAddress);
    } finally {
      if (mounted) setState(() => _loadingSyslogGuide = false);
    }
  }

  Future<void> _onSelectSyslogServerAddress(String? address) async {
    final trimmed = address?.trim();
    setState(() {
      _selectedSyslogAddress = (trimmed == null || trimmed.isEmpty)
          ? null
          : trimmed;
      if (_selectedSyslogAddress != null) {
        _syslogServerAddrCtrl.text = _selectedSyslogAddress!;
      }
    });
    await _loadSyslogGuide(
      force: true,
      overrideAddress: _selectedSyslogAddress,
    );
  }

  String? _resolveSyslogTargetAddress() {
    final settings = ref.read(coreSettingsProvider);
    final guide = ref.read(logsProvider).setupGuide;
    final typed = _syslogServerAddrCtrl.text.trim();
    return DeviceSyslogSetupPanel.resolveBestServerAddress(
      selectedAddress: typed.isNotEmpty ? typed : _selectedSyslogAddress,
      hintedAddress: _syslogHints?.serverAddress,
      usableAddresses: settings.usableAddresses,
      preferredAddressId: settings.preferredAddressId,
      guideAddress: guide?.serverAddress,
    );
  }

  String? _validateSyslogAutoInputs() {
    final ip = _ipCtrl.text.trim();
    final mac = _macCtrl.text.trim();
    final reusable = _reuseExistingSyslogAccess
        ? _reusableSyslogCredential
        : null;
    if (reusable != null) {
      if (ip.isEmpty) {
        return 'Informe o endereço IP do dispositivo antes de configurar o Syslog automaticamente.';
      }
      return null;
    }

    if (_syslogAccessMethod == SyslogAccessMethod.dedicatedSshKey) {
      if (ip.isEmpty) {
        return 'Informe o endereço IP do dispositivo para provisionar a chave SSH dedicada.';
      }
      if (_sshAdminUserCtrl.text.trim().isEmpty ||
          _sshAdminPassCtrl.text.isEmpty) {
        return 'Informe o usuário e a senha administrativos para instalar a chave SSH dedicada.';
      }
      return null;
    }

    if (_oneTimeProtocol != 'mactelnet' && ip.isEmpty) {
      return 'Informe o endereço IP do dispositivo para conectar via ${_oneTimeProtocol.toUpperCase()}.';
    }
    if (_oneTimeProtocol == 'mactelnet' &&
        mac.isEmpty &&
        (_syslogHints?.macAddress?.trim().isEmpty ?? true)) {
      return 'Informe o endereço MAC do dispositivo para conectar via MAC-Telnet.';
    }
    if (_oneTimeUserCtrl.text.trim().isEmpty || _oneTimePassCtrl.text.isEmpty) {
      return 'Informe usuário e senha de uso único para configurar o Syslog.';
    }
    return null;
  }

  Future<bool> _runAutoSyslogProvision(int deviceId) async {
    final validationError = _validateSyslogAutoInputs();
    if (validationError != null) {
      setState(() {
        _syslogProvisionError = validationError;
        _syslogProvisionStep = null;
      });
      return false;
    }

    final effectiveOsId = _resolvedEffectiveOsId();
    final serverAddress = _resolveSyslogTargetAddress();
    final reusable = _reuseExistingSyslogAccess
        ? _reusableSyslogCredential
        : null;

    setState(() {
      _provisioningSyslog = true;
      _syslogProvisionError = null;
      _syslogProvisionStep = reusable != null
          ? 'Aplicando configuração de Syslog com acesso já configurado...'
          : (_syslogAccessMethod == SyslogAccessMethod.dedicatedSshKey
                ? '1/2 · Instalando chave SSH dedicada no equipamento...'
                : 'Conectando ao equipamento e aplicando Syslog...');
    });

    try {
      late final ProvisionLoggingResponse result;

      if (reusable != null) {
        final protocol = reusable.kind == 'telnet' ? 'telnet' : 'ssh';
        result = await ref
            .read(logsProvider.notifier)
            .provisionDevice(deviceId, <String, dynamic>{
              'protocol': protocol,
              'port': reusable.port ?? (protocol == 'telnet' ? 23 : 22),
              'savedCredential': protocol,
              'saveCredential': false,
              'operatingSystem': ?effectiveOsId,
              'serverAddress': ?serverAddress,
            });
      } else if (_syslogAccessMethod == SyslogAccessMethod.dedicatedSshKey) {
        final sshPort = int.tryParse(_sshDedicatedPortCtrl.text.trim()) ?? 22;
        final platform = DeviceSyslogSetupPanel.inferDedicatedPlatform(
          osId: effectiveOsId,
          vendorHint: _vendorCtrl.text.trim(),
          accessPlatform: _existingAccessView?.dedicatedPlatform,
        );
        await ref
            .read(deviceAccessProvider.notifier)
            .provisionDedicatedKey(deviceId, <String, dynamic>{
              'platform': ?platform,
              'username': _sshAdminUserCtrl.text.trim(),
              'password': _sshAdminPassCtrl.text,
              'port': sshPort,
            });
        if (!mounted) return false;
        setState(
          () => _syslogProvisionStep =
              '2/2 · Aplicando configuração de Syslog via chave SSH...',
        );
        result = await ref
            .read(logsProvider.notifier)
            .provisionDevice(deviceId, <String, dynamic>{
              'protocol': 'ssh',
              'port': sshPort,
              'savedCredential': 'ssh',
              'saveCredential': false,
              'operatingSystem': ?effectiveOsId,
              'serverAddress': ?serverAddress,
            });
      } else {
        final defaultPort = switch (_oneTimeProtocol) {
          'telnet' => 23,
          'mactelnet' => 20561,
          _ => 22,
        };
        final port = int.tryParse(_oneTimePortCtrl.text.trim()) ?? defaultPort;
        final mac = _macCtrl.text.trim().isNotEmpty
            ? _macCtrl.text.trim()
            : _syslogHints?.macAddress?.trim();
        result = await ref
            .read(logsProvider.notifier)
            .provisionDevice(deviceId, <String, dynamic>{
              'protocol': _oneTimeProtocol,
              'port': port,
              'username': _oneTimeUserCtrl.text.trim(),
              'password': _oneTimePassCtrl.text,
              'savedCredential': null,
              'saveCredential': false,
              'operatingSystem': ?effectiveOsId,
              'serverAddress': ?serverAddress,
              if (_oneTimeProtocol == 'mactelnet' &&
                  mac != null &&
                  mac.isNotEmpty)
                'macAddress': mac,
            });
      }

      if (!mounted) return true;
      _sshAdminPassCtrl.clear();
      _oneTimePassCtrl.clear();
      DeviceAccessViewModel? refreshedAccess = _existingAccessView;
      if (_syslogAccessMethod == SyslogAccessMethod.dedicatedSshKey &&
          reusable == null) {
        try {
          refreshedAccess = await ref
              .read(deviceAccessProvider.notifier)
              .fetchAccess(deviceId);
        } catch (_) {}
      }
      if (!mounted) return true;
      setState(() {
        _syslogProvisionResult = result;
        _syslogProvisionError = null;
        _syslogProvisionStep = null;
        _existingAccessView = refreshedAccess;
        if (_reusableSyslogCredential != null) {
          _reuseExistingSyslogAccess = true;
        }
      });
      return true;
    } catch (err) {
      if (mounted) {
        final platformHint = _resolveAuthPlatformHint();
        setState(() {
          _provisioningSyslog = false;
          _syslogProvisionError = formatDeviceAccessError(
            err,
            platformOrOs: platformHint,
          );
          _syslogProvisionStep = null;
        });
        if (isDeviceAuthFailure(err)) {
          await showDeviceAuthFailureDialog(
            context,
            err,
            platformOrOs: platformHint,
          );
        }
      }
      return false;
    } finally {
      if (mounted && (_provisioningSyslog || _syslogProvisionStep != null)) {
        setState(() {
          _provisioningSyslog = false;
          _syslogProvisionStep = null;
        });
      }
    }
  }

  Future<void> _testSnmp({required bool autoDetect}) async {
    final host = _ipCtrl.text.trim();
    if (host.isEmpty) {
      setState(() => _error = 'Informe o endereço IP antes de testar SNMP.');
      return;
    }
    setState(() {
      _testingSnmp = true;
      _error = null;
    });
    var shouldQueryInterfaces = false;
    try {
      final res = await ref
          .read(snmpProfilesProvider.notifier)
          .testSnmp(
            host: host,
            version: _snmpVersion,
            community: _communityCtrl.text.trim().isEmpty
                ? 'public'
                : _communityCtrl.text.trim(),
            autoDetect: autoDetect,
          );
      if (!mounted) return;
      setState(() {
        _snmpResult = res;
        if (res.responded) {
          _snmpEnabled = true;
          if (res.version != null) _snmpVersion = res.version!;
          if (res.community != null) _communityCtrl.text = res.community!;
          if (_nameCtrl.text.trim().isEmpty && res.sysName != null) {
            _nameCtrl.text = res.sysName!;
          }
        }
      });
      shouldQueryInterfaces = res.responded;
    } catch (err) {
      if (mounted) setState(() => _error = err.toString());
    } finally {
      if (mounted) setState(() => _testingSnmp = false);
    }
    if (shouldQueryInterfaces && mounted) {
      await _queryLiveInterfaces();
      if (mounted && !_userPickedOs) {
        _lastAutoIdentifyKey = '';
        await _identifyDevice(automatic: true);
      }
    }
  }

  Future<void> _queryLiveInterfaces() async {
    final host = _ipCtrl.text.trim();
    if (host.isEmpty) {
      setState(
        () => _error = 'Informe o endereço IP antes de listar interfaces.',
      );
      return;
    }
    setState(() {
      _loadingIfaces = true;
      _error = null;
    });
    try {
      final list = await ref
          .read(snmpProfilesProvider.notifier)
          .queryInterfaces(
            host: host,
            version: _snmpVersion,
            community: _communityCtrl.text.trim().isEmpty
                ? 'public'
                : _communityCtrl.text.trim(),
          );
      if (mounted) {
        setState(() {
          _discoveredIfaces = list;
          _ifacesQueried = true;
          if (list.isNotEmpty) _manualLinkIface = false;
        });
      }
    } catch (err) {
      if (mounted) setState(() => _error = err.toString());
    } finally {
      if (mounted) setState(() => _loadingIfaces = false);
    }
  }

  Future<void> _submit() async {
    if (!_formKey.currentState!.validate()) return;
    final shouldAutoProvision =
        _syslogEnabled &&
        _syslogMode == SyslogSetupMode.automatic &&
        _syslogProvisionResult == null;
    if (shouldAutoProvision) {
      final syslogValidation = _validateSyslogAutoInputs();
      if (syslogValidation != null) {
        setState(() => _syslogProvisionError = syslogValidation);
        return;
      }
    }

    setState(() {
      _saving = true;
      _error = null;
      _syslogProvisionError = null;
    });

    final payload = <String, dynamic>{
      'name': _nameCtrl.text.trim(),
      'type': _type,
      if (_ipCtrl.text.trim().isNotEmpty) 'ipAddress': _ipCtrl.text.trim(),
      if (_macCtrl.text.trim().isNotEmpty) 'macAddress': _macCtrl.text.trim(),
      if (_vendorCtrl.text.trim().isNotEmpty) 'vendor': _vendorCtrl.text.trim(),
      if (_modelCtrl.text.trim().isNotEmpty) 'model': _modelCtrl.text.trim(),
      if (_locationCtrl.text.trim().isNotEmpty)
        'location': _locationCtrl.text.trim(),
      if (_descCtrl.text.trim().isNotEmpty)
        'description': _descCtrl.text.trim(),
      'siteId': _siteId,
      'networkId': _networkId,
      'parentId': _parentId,
      'operatingSystem': _operatingSystem,
      'accessMode': _accessMode,
      'isMonitored': _isMonitored,
      'snmpEnabled': _snmpEnabled,
      if (_snmpEnabled) ...<String, dynamic>{
        'snmpVersion': _snmpVersion,
        'snmpCommunity': _communityCtrl.text.trim().isEmpty
            ? 'public'
            : _communityCtrl.text.trim(),
        'snmpPollIntervalSeconds':
            int.tryParse(_pollIntervalCtrl.text.trim()) ?? 60,
      },
      if (_linkIfaceCtrl.text.trim().isNotEmpty)
        'linkInterfaceName': _linkIfaceCtrl.text.trim(),
    };

    final notifier = ref.read(devicesProvider.notifier);
    final existingId = _effectiveDeviceId;
    final saved = existingId != null
        ? await notifier.updateDevice(existingId, payload)
        : await notifier.createDevice(payload);

    if (!mounted) return;
    if (saved == null) {
      setState(() {
        _saving = false;
        _error =
            ref.read(devicesProvider).error ?? 'Falha ao salvar dispositivo.';
      });
      return;
    }

    if (widget.existing == null) {
      setState(() => _createdDevice = saved);
    }

    if (shouldAutoProvision) {
      final provisioned = await _runAutoSyslogProvision(saved.id);
      if (!mounted) return;
      setState(() => _saving = false);
      if (provisioned) {
        final confirmed = _syslogProvisionResult?.confirmed == true;
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(
              confirmed
                  ? 'Dispositivo salvo e Syslog configurado e confirmado!'
                  : 'Dispositivo salvo e Syslog configurado automaticamente.',
            ),
          ),
        );
        widget.onDone(saved);
      } else {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text(
              'Dispositivo salvo, mas a configuração automática de Syslog falhou. '
              'Você pode tentar novamente ou copiar os comandos no modo Manual.',
            ),
          ),
        );
      }
      return;
    }

    setState(() => _saving = false);
    widget.onDone(saved);
  }

  @override
  Widget build(BuildContext context) {
    final sites = ref.watch(sitesProvider).items;
    final networks = ref.watch(networksProvider).items;
    final devices = ref
        .watch(devicesProvider)
        .items
        .where((d) => d.id != widget.existing?.id)
        .toList(growable: false);
    final systems = ref.watch(operatingSystemsProvider);
    final syslogGuide = ref.watch(logsProvider).setupGuide;
    final coreSettings = ref.watch(coreSettingsProvider);
    final effectiveOsId = _resolvedEffectiveOsId();
    final resolvedSyslogAddress =
        DeviceSyslogSetupPanel.resolveBestServerAddress(
          selectedAddress: _syslogServerAddrCtrl.text.trim().isNotEmpty
              ? _syslogServerAddrCtrl.text.trim()
              : _selectedSyslogAddress,
          hintedAddress: _syslogHints?.serverAddress,
          usableAddresses: coreSettings.usableAddresses,
          preferredAddressId: coreSettings.preferredAddressId,
          guideAddress: syslogGuide?.serverAddress,
        );
    final syslogPort = syslogGuide?.port ?? _syslogHints?.serverPort ?? 514;
    final reusableCred = _reusableSyslogCredential;
    final willAutoProvisionOnSave =
        _syslogEnabled &&
        _syslogMode == SyslogSetupMode.automatic &&
        _syslogProvisionResult == null;
    String? effectiveOsLabel;
    if (effectiveOsId != null) {
      for (final s in systems) {
        if (s.id == effectiveOsId) {
          effectiveOsLabel = s.label;
          break;
        }
      }
    }

    return Form(
      key: _formKey,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          if (_error != null) ...<Widget>[
            NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
            const SizedBox(height: 12),
          ],
          TextFormField(
            controller: _nameCtrl,
            decoration: const InputDecoration(
              labelText: 'Nome do dispositivo *',
            ),
            validator: FormValidators.required(
              'Informe o nome do dispositivo.',
            ),
          ),
          const SizedBox(height: 12),
          ResponsiveFieldRow(
            children: <Widget>[
              AppDropdownFormField<String>(
                initialValue: _type,
                decoration: const InputDecoration(labelText: 'Tipo'),
                items: deviceTypeOptions
                    .map(
                      (opt) => DropdownMenuItem(
                        value: opt.$1,
                        child: Text(opt.$2, overflow: TextOverflow.ellipsis),
                      ),
                    )
                    .toList(growable: false),
                onChanged: (v) {
                  if (v != null) setState(() => _type = v);
                },
              ),
              TextFormField(
                controller: _ipCtrl,
                decoration: const InputDecoration(labelText: 'Endereço IP'),
                validator: FormValidators.ipOrHost(),
                onChanged: (_) => _scheduleAutoIdentify(),
              ),
            ],
          ),
          const SizedBox(height: 12),
          TextFormField(
            controller: _macCtrl,
            decoration: InputDecoration(
              labelText: 'Endereço MAC',
              hintText: 'AA:BB:CC:DD:EE:FF',
              suffixIcon: IconButton(
                tooltip: 'Consultar fabricante (OUI)',
                icon: const Icon(Icons.search),
                onPressed: () => _lookupMac(_macCtrl.text),
              ),
            ),
            onChanged: (val) {
              final hex = val.replaceAll(RegExp(r'[^0-9a-fA-F]'), '');
              if (hex.length >= 6) _lookupMac(val);
            },
          ),
          if (_macLookup != null && _macLookup!.vendor != null) ...<Widget>[
            const SizedBox(height: 6),
            SemanticChip(
              label:
                  'OUI: ${_macLookup!.vendor} (${_macLookup!.source.toUpperCase()})',
              tone: SemanticTone.info,
              compact: true,
              icon: Icons.verified_outlined,
            ),
          ],
          const SizedBox(height: 12),
          ResponsiveFieldRow(
            children: <Widget>[
              AppDropdownFormField<int?>(
                initialValue: sites.any((s) => s.id == _siteId)
                    ? _siteId
                    : null,
                decoration: const InputDecoration(labelText: 'Site'),
                items: <DropdownMenuItem<int?>>[
                  const DropdownMenuItem<int?>(
                    value: null,
                    child: Text('Nenhum'),
                  ),
                  for (final s in sites)
                    DropdownMenuItem<int?>(
                      value: s.id,
                      child: Text(s.name, overflow: TextOverflow.ellipsis),
                    ),
                ],
                onChanged: (v) => setState(() => _siteId = v),
              ),
              AppDropdownFormField<int?>(
                initialValue: networks.any((n) => n.id == _networkId)
                    ? _networkId
                    : null,
                decoration: const InputDecoration(labelText: 'Rede IP'),
                items: <DropdownMenuItem<int?>>[
                  const DropdownMenuItem<int?>(
                    value: null,
                    child: Text('Nenhuma'),
                  ),
                  for (final n in networks)
                    DropdownMenuItem<int?>(
                      value: n.id,
                      child: Text(
                        '${n.name} (${n.cidr})',
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                ],
                onChanged: (v) => setState(() => _networkId = v),
              ),
            ],
          ),
          const SizedBox(height: 12),
          ResponsiveFieldRow(
            children: <Widget>[
              AppDropdownFormField<int?>(
                initialValue: devices.any((d) => d.id == _parentId)
                    ? _parentId
                    : null,
                decoration: const InputDecoration(
                  labelText: 'Dispositivo pai (hierarquia)',
                ),
                items: <DropdownMenuItem<int?>>[
                  const DropdownMenuItem<int?>(
                    value: null,
                    child: Text('Raiz / Nenhum'),
                  ),
                  for (final d in devices)
                    DropdownMenuItem<int?>(
                      value: d.id,
                      child: Text(d.name, overflow: TextOverflow.ellipsis),
                    ),
                ],
                onChanged: (v) => setState(() => _parentId = v),
              ),
              AppDropdownFormField<String?>(
                initialValue: _accessMode,
                decoration: const InputDecoration(labelText: 'Modo de acesso'),
                items: const <DropdownMenuItem<String?>>[
                  DropdownMenuItem<String?>(
                    value: null,
                    child: Text('Automático'),
                  ),
                  DropdownMenuItem<String?>(
                    value: 'local',
                    child: Text('Rede local'),
                  ),
                  DropdownMenuItem<String?>(value: 'vpn', child: Text('VPN')),
                  DropdownMenuItem<String?>(
                    value: 'remote',
                    child: Text('Sonda remota'),
                  ),
                ],
                onChanged: (v) => setState(() => _accessMode = v),
              ),
            ],
          ),
          const SizedBox(height: 12),
          ResponsiveFieldRow(
            children: <Widget>[
              TextFormField(
                controller: _vendorCtrl,
                decoration: const InputDecoration(labelText: 'Fabricante'),
                onChanged: (_) {
                  _syncDefaultDeviceUsernames();
                  _scheduleAutoIdentify();
                },
              ),
              TextFormField(
                controller: _modelCtrl,
                decoration: const InputDecoration(labelText: 'Modelo'),
                onChanged: (_) {
                  _syncDefaultDeviceUsernames();
                  _scheduleAutoIdentify();
                },
              ),
            ],
          ),
          const SizedBox(height: 12),
          Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: <Widget>[
              Expanded(
                child: AppDropdownFormField<String?>(
                  key: ValueKey<String>(
                    'device-os-${_operatingSystem ?? 'auto'}-${systems.length}',
                  ),
                  initialValue: systems.any((s) => s.id == _operatingSystem)
                      ? _operatingSystem
                      : null,
                  decoration: const InputDecoration(
                    labelText: 'Sistema operacional',
                  ),
                  items: <DropdownMenuItem<String?>>[
                    const DropdownMenuItem<String?>(
                      value: null,
                      child: Text('Automático'),
                    ),
                    for (final s in systems)
                      DropdownMenuItem<String?>(
                        value: s.id,
                        child: Text(s.label, overflow: TextOverflow.ellipsis),
                      ),
                  ],
                  onChanged: (v) => setState(() {
                    _operatingSystem = v;
                    _userPickedOs = v != null;
                    _manualSyslogSnippetIndex = null;
                    _syncDefaultDeviceUsernames();
                  }),
                ),
              ),
              const SizedBox(width: 8),
              OutlinedButton.icon(
                onPressed: _identifying ? null : () => _identifyDevice(),
                icon: _identifying
                    ? const SizedBox(
                        width: 14,
                        height: 14,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(Icons.auto_fix_high, size: 18),
                label: const Text('Identificar'),
              ),
            ],
          ),
          if (_autoIdentifying) ...<Widget>[
            const SizedBox(height: 6),
            Row(
              children: <Widget>[
                const SizedBox(
                  width: 12,
                  height: 12,
                  child: CircularProgressIndicator(strokeWidth: 2),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    'Identificando SO automaticamente...',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: context.textTheme.bodySmall?.copyWith(
                      color: context.colorScheme.onSurfaceVariant,
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                ),
              ],
            ),
          ],
          if (_identifyResult != null) ...<Widget>[
            const SizedBox(height: 8),
            NestedPanel(
              tone: SemanticTone.info,
              padding: const EdgeInsets.all(10),
              child: Text(
                'Detectado: ${_identifyResult!.operatingSystem ?? 'desconhecido'}'
                '${_identifyResult!.source != null ? ' (fonte: ${_identifyResult!.source})' : ''}'
                '${_identifyResult!.evidence != null ? '\n${_identifyResult!.evidence}' : ''}',
                style: context.textTheme.bodySmall,
              ),
            ),
          ],
          const SizedBox(height: 12),
          NestedPanel(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Material(
                  type: MaterialType.transparency,
                  child: SwitchListTile(
                    contentPadding: EdgeInsets.zero,
                    title: const Text('Habilitar SNMP'),
                    subtitle: const Text(
                      'Coleta de interfaces, CPU, memória e sensores',
                    ),
                    value: _snmpEnabled,
                    onChanged: (v) {
                      setState(() => _snmpEnabled = v);
                      if (v) _scheduleAutoIdentify();
                    },
                  ),
                ),
                if (_snmpEnabled) ...<Widget>[
                  const SizedBox(height: 6),
                  ResponsiveFieldRow(
                    breakpoint: 480,
                    spacing: 12,
                    runSpacing: 12,
                    children: <Widget>[
                      AppDropdownFormField<String>(
                        initialValue: _snmpVersion,
                        decoration: const InputDecoration(
                          labelText: 'Versão SNMP',
                        ),
                        items: const <DropdownMenuItem<String>>[
                          DropdownMenuItem(value: 'v1', child: Text('SNMP v1')),
                          DropdownMenuItem(
                            value: 'v2c',
                            child: Text('SNMP v2c'),
                          ),
                          DropdownMenuItem(value: 'v3', child: Text('SNMP v3')),
                        ],
                        onChanged: (v) {
                          if (v != null) {
                            setState(() => _snmpVersion = v);
                            _scheduleAutoIdentify();
                          }
                        },
                      ),
                      TextFormField(
                        controller: _pollIntervalCtrl,
                        keyboardType: TextInputType.number,
                        decoration: const InputDecoration(
                          labelText: 'Intervalo (s)',
                        ),
                      ),
                    ],
                  ),
                  const SizedBox(height: 12),
                  TextFormField(
                    controller: _communityCtrl,
                    decoration: const InputDecoration(
                      labelText: 'Community SNMP',
                    ),
                    onChanged: (_) => _scheduleAutoIdentify(),
                  ),
                  const SizedBox(height: 12),
                  SymmetricalActionBar(
                    children: <Widget>[
                      OutlinedButton.icon(
                        onPressed: _testingSnmp
                            ? null
                            : () => _testSnmp(autoDetect: false),
                        icon: _testingSnmp
                            ? const SizedBox(
                                width: 14,
                                height: 14,
                                child: CircularProgressIndicator(
                                  strokeWidth: 2,
                                ),
                              )
                            : const Icon(Icons.network_check, size: 16),
                        label: const Text(
                          'Testar SNMP',
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                      OutlinedButton.icon(
                        onPressed: _testingSnmp
                            ? null
                            : () => _testSnmp(autoDetect: true),
                        icon: const Icon(Icons.auto_awesome, size: 16),
                        label: const Text(
                          'Auto-detectar',
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                      OutlinedButton.icon(
                        onPressed: _loadingIfaces ? null : _queryLiveInterfaces,
                        icon: _loadingIfaces
                            ? const SizedBox(
                                width: 14,
                                height: 14,
                                child: CircularProgressIndicator(
                                  strokeWidth: 2,
                                ),
                              )
                            : const Icon(Icons.settings_ethernet, size: 16),
                        label: const Text(
                          'Listar interfaces',
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                    ],
                  ),
                  if (_snmpResult != null) ...<Widget>[
                    const SizedBox(height: 8),
                    NestedPanel(
                      tone: _snmpResult!.responded
                          ? SemanticTone.success
                          : SemanticTone.error,
                      padding: const EdgeInsets.all(10),
                      child: Text(
                        _snmpResult!.responded
                            ? 'SNMP OK${_snmpResult!.sysName != null ? ' · ${_snmpResult!.sysName}' : ''}'
                                  '${_snmpResult!.sysDescr != null ? '\n${_snmpResult!.sysDescr}' : ''}'
                            : (_snmpResult!.message ?? 'Sem resposta SNMP.'),
                        style: context.textTheme.bodySmall,
                      ),
                    ),
                  ],
                  const SizedBox(height: 12),
                  SnmpUplinkInterfaceSelector(
                    controller: _linkIfaceCtrl,
                    interfaces: _discoveredIfaces,
                    loading: _loadingIfaces,
                    hasQueried: _ifacesQueried,
                    manualMode: _manualLinkIface,
                    onManualModeChanged: (manual) =>
                        setState(() => _manualLinkIface = manual),
                    onRequestInterfaces: _queryLiveInterfaces,
                    onSelected: (_) => setState(() {}),
                  ),
                ],
              ],
            ),
          ),
          const SizedBox(height: 12),
          DeviceSyslogSetupPanel(
            enabled: _syslogEnabled,
            loading: _loadingSyslogGuide,
            guide: syslogGuide,
            mode: _syslogMode,
            accessMethod: _syslogAccessMethod,
            isExistingDevice: _effectiveDeviceId != null,
            resolvedServerAddress: resolvedSyslogAddress,
            serverPort: syslogPort,
            usableAddresses: coreSettings.usableAddresses,
            serverAddressController: _syslogServerAddrCtrl,
            existingCredential: reusableCred,
            reuseExistingAccess: _reuseExistingSyslogAccess,
            sshAdminUserController: _sshAdminUserCtrl,
            sshAdminPasswordController: _sshAdminPassCtrl,
            sshDedicatedPortController: _sshDedicatedPortCtrl,
            oneTimeProtocol: _oneTimeProtocol,
            oneTimeUserController: _oneTimeUserCtrl,
            oneTimePasswordController: _oneTimePassCtrl,
            oneTimePortController: _oneTimePortCtrl,
            provisioning: _provisioningSyslog,
            provisionStep: _syslogProvisionStep,
            provisionError: _syslogProvisionError,
            provisionResult: _syslogProvisionResult,
            operatingSystemId: effectiveOsId,
            operatingSystemLabel: effectiveOsLabel,
            vendorHint: _vendorCtrl.text.trim().isEmpty
                ? null
                : _vendorCtrl.text.trim(),
            manualSnippetIndex: _manualSyslogSnippetIndex,
            onToggle: (v) {
              setState(() {
                _syslogEnabled = v;
                if (!v) _syslogProvisionError = null;
              });
              if (v) _loadSyslogGuide();
            },
            onModeChanged: (mode) => setState(() {
              _syslogMode = mode;
              _syslogProvisionError = null;
            }),
            onAccessMethodChanged: (method) => setState(() {
              _syslogAccessMethod = method;
              _syslogProvisionError = null;
            }),
            onReuseExistingAccessChanged: (reuse) => setState(() {
              _reuseChoiceTouched = true;
              _reuseExistingSyslogAccess = reuse;
              _syslogProvisionError = null;
            }),
            onOneTimeProtocolChanged: (protocol) => setState(() {
              _oneTimeProtocol = protocol;
              _oneTimePortCtrl.text = switch (protocol) {
                'telnet' => '23',
                'mactelnet' => '20561',
                _ => '22',
              };
              _syslogProvisionError = null;
            }),
            onSelectServerAddress: _onSelectSyslogServerAddress,
            onSelectSnippetIndex: (idx) =>
                setState(() => _manualSyslogSnippetIndex = idx),
            onRetryLoad: () => _loadSyslogGuide(force: true),
            onSshAdminUserChanged: (_) => _sshAdminUserEdited = true,
            onOneTimeUserChanged: (_) => _oneTimeUserEdited = true,
            onProvisionNow: _effectiveDeviceId != null
                ? () => _runAutoSyslogProvision(_effectiveDeviceId!)
                : null,
          ),
          const SizedBox(height: 10),
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            title: const Text('Monitorar dispositivo'),
            value: _isMonitored,
            onChanged: (v) => setState(() => _isMonitored = v),
          ),
          TextFormField(
            controller: _locationCtrl,
            decoration: const InputDecoration(labelText: 'Localização física'),
          ),
          const SizedBox(height: 10),
          TextFormField(
            controller: _descCtrl,
            maxLines: 2,
            decoration: const InputDecoration(labelText: 'Observações'),
          ),
          const SizedBox(height: 16),
          SymmetricalActionBar(
            children: <Widget>[
              OutlinedButton(
                onPressed: (_saving || _provisioningSyslog)
                    ? null
                    : () => Navigator.of(context).pop(_createdDevice),
                child: Text(
                  _createdDevice != null ? 'Concluir sem Syslog' : 'Cancelar',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              FilledButton.icon(
                onPressed: (_saving || _provisioningSyslog) ? null : _submit,
                icon: (_saving || _provisioningSyslog)
                    ? const SizedBox(
                        width: 16,
                        height: 16,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(Icons.check),
                label: Text(
                  _effectiveDeviceId != null
                      ? 'Salvar alterações'
                      : (willAutoProvisionOnSave
                            ? 'Cadastrar e configurar Syslog'
                            : 'Cadastrar'),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

/// Seletor coeso para a interface de uplink / entrada de link no cadastro SNMP.
///
/// Exibe um select Material 3 com status semântico (`UP`/`DOWN`) e velocidade
/// negociada (`formatLinkSpeed`) assim que as interfaces são descobertas,
/// preservando valores customizados existentes e permitindo alternância limpa
/// para digitação manual.
class SnmpUplinkInterfaceSelector extends StatelessWidget {
  const SnmpUplinkInterfaceSelector({
    super.key,
    required this.controller,
    required this.interfaces,
    required this.loading,
    required this.manualMode,
    required this.onManualModeChanged,
    this.hasQueried = false,
    this.onRequestInterfaces,
    this.onSelected,
  });

  static const String manualEntryValue = '__snmp_uplink_manual__';

  final TextEditingController controller;
  final List<DeviceInterfaceModel> interfaces;
  final bool loading;
  final bool hasQueried;
  final bool manualMode;
  final ValueChanged<bool> onManualModeChanged;
  final VoidCallback? onRequestInterfaces;
  final ValueChanged<String>? onSelected;

  /// Formata a velocidade negociada usando o formatador canônico `formatLinkSpeed`.
  static String formatNegotiatedSpeed(DeviceInterfaceModel iface) {
    final speed = iface.speed;
    if (speed == null || !speed.isFinite || speed <= 0) {
      return iface.isUp ? '—' : 'Sem link';
    }
    return formatLinkSpeed(speed);
  }

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final sorted = DeviceInterfaceModel.sortForUplinkSelection(interfaces);
    final showSelect = sorted.isNotEmpty && !manualMode;
    final currentValue = controller.text.trim();
    final hasCustomValue =
        currentValue.isNotEmpty &&
        !sorted.any((iface) => iface.displayName == currentValue);
    final upCount = sorted.where((i) => i.isUp).length;
    final downCount = sorted.length - upCount;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (loading) ...<Widget>[
          Row(
            children: <Widget>[
              const SizedBox(
                width: 14,
                height: 14,
                child: CircularProgressIndicator(strokeWidth: 2),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  'Consultando interfaces SNMP...',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: context.textTheme.bodySmall?.copyWith(
                    color: scheme.onSurfaceVariant,
                    fontWeight: FontWeight.w500,
                  ),
                ),
              ),
            ],
          ),
          const SizedBox(height: 8),
        ],
        if (showSelect) ...<Widget>[
          AppDropdownFormField<String>(
            key: ValueKey<String>(
              'snmp-uplink-select-$currentValue-${sorted.length}',
            ),
            initialValue: currentValue,
            decoration: const InputDecoration(
              labelText: 'Interface de uplink / link',
            ),
            items: <DropdownMenuItem<String>>[
              DropdownMenuItem<String>(
                value: '',
                child: Text(
                  'Nenhuma (automático)',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(color: scheme.onSurfaceVariant),
                ),
              ),
              if (hasCustomValue)
                DropdownMenuItem<String>(
                  value: currentValue,
                  child: _CustomInterfaceOptionRow(name: currentValue),
                ),
              for (final iface in sorted)
                DropdownMenuItem<String>(
                  value: iface.displayName,
                  child: _SnmpInterfaceOptionRow(iface: iface),
                ),
              DropdownMenuItem<String>(
                value: manualEntryValue,
                child: Row(
                  children: <Widget>[
                    Icon(Icons.edit_outlined, size: 16, color: scheme.primary),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        'Informar manualmente...',
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          color: scheme.primary,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ],
            onChanged: (value) {
              if (value == null) return;
              if (value == manualEntryValue) {
                onManualModeChanged(true);
                return;
              }
              controller.text = value;
              onSelected?.call(value);
            },
          ),
          const SizedBox(height: 6),
          Wrap(
            spacing: 10,
            runSpacing: 4,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: <Widget>[
              _InterfaceCountLegend(
                label: '$upCount UP',
                tone: SemanticTone.success,
              ),
              _InterfaceCountLegend(
                label: '$downCount DOWN',
                tone: SemanticTone.error,
              ),
            ],
          ),
        ] else ...<Widget>[
          TextFormField(
            controller: controller,
            decoration: InputDecoration(
              labelText: 'Interface de uplink / link',
              hintText: 'Ex.: ether1, wan0',
              suffixIcon:
                  sorted.isEmpty && onRequestInterfaces != null && !loading
                  ? IconButton(
                      tooltip: 'Listar interfaces SNMP',
                      icon: const Icon(Icons.settings_ethernet),
                      onPressed: onRequestInterfaces,
                    )
                  : null,
            ),
          ),
          const SizedBox(height: 6),
          if (manualMode && sorted.isNotEmpty)
            Align(
              alignment: AlignmentDirectional.centerStart,
              child: TextButton.icon(
                onPressed: () => onManualModeChanged(false),
                style: TextButton.styleFrom(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 8,
                    vertical: 4,
                  ),
                  minimumSize: const Size(0, 32),
                  tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                ),
                icon: const Icon(Icons.list_alt, size: 16),
                label: Text('Selecionar da lista (${sorted.length})'),
              ),
            )
          else if (!loading)
            Text(
              hasQueried
                  ? 'Nenhuma interface SNMP retornada — informe manualmente.'
                  : 'Teste o SNMP ou clique em "Listar interfaces" para selecionar com status e velocidade.',
              style: context.textTheme.bodySmall?.copyWith(
                color: scheme.onSurfaceVariant,
              ),
            ),
        ],
      ],
    );
  }
}

class _SnmpInterfaceOptionRow extends StatelessWidget {
  const _SnmpInterfaceOptionRow({required this.iface});

  final DeviceInterfaceModel iface;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final subtitle = iface.subtitleText;
    final speedLabel = SnmpUplinkInterfaceSelector.formatNegotiatedSpeed(iface);

    return Row(
      children: <Widget>[
        _InterfaceStatusBadge(isUp: iface.isUp),
        const SizedBox(width: 8),
        Expanded(
          child: Text.rich(
            TextSpan(
              text: iface.displayName,
              style: TextStyle(
                fontWeight: FontWeight.w600,
                color: scheme.onSurface,
              ),
              children: subtitle != null
                  ? <InlineSpan>[
                      TextSpan(
                        text: ' · $subtitle',
                        style: TextStyle(
                          fontWeight: FontWeight.w400,
                          color: scheme.onSurfaceVariant,
                        ),
                      ),
                    ]
                  : null,
            ),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            softWrap: false,
          ),
        ),
        const SizedBox(width: 8),
        _InterfaceSpeedTag(
          label: speedLabel,
          isUp: iface.isUp,
          hasSpeed: iface.negotiatedSpeedBps > 0,
        ),
      ],
    );
  }
}

class _CustomInterfaceOptionRow extends StatelessWidget {
  const _CustomInterfaceOptionRow({required this.name});

  final String name;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final infoColors = context.semanticColors.resolve(
      SemanticTone.info,
      scheme,
    );

    return Row(
      children: <Widget>[
        Container(
          padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
          decoration: BoxDecoration(
            color: infoColors.container,
            borderRadius: BorderRadius.circular(6),
            border: Border.all(color: infoColors.border),
          ),
          child: Text(
            'ATUAL',
            style: TextStyle(
              fontSize: 10,
              fontWeight: FontWeight.w700,
              color: infoColors.onContainer,
              height: 1.1,
            ),
          ),
        ),
        const SizedBox(width: 8),
        Expanded(
          child: Text(
            name,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            softWrap: false,
            style: TextStyle(
              fontWeight: FontWeight.w600,
              color: scheme.onSurface,
            ),
          ),
        ),
      ],
    );
  }
}

class _InterfaceStatusBadge extends StatelessWidget {
  const _InterfaceStatusBadge({required this.isUp});

  final bool isUp;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final colors = context.semanticColors.resolve(
      isUp ? SemanticTone.success : SemanticTone.error,
      scheme,
    );

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
      decoration: BoxDecoration(
        color: colors.container,
        borderRadius: BorderRadius.circular(6),
        border: Border.all(color: colors.border),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Container(
            width: 6,
            height: 6,
            decoration: BoxDecoration(
              color: colors.foreground,
              shape: BoxShape.circle,
            ),
          ),
          const SizedBox(width: 4),
          Text(
            isUp ? 'UP' : 'DOWN',
            style: TextStyle(
              fontSize: 10.5,
              fontWeight: FontWeight.w800,
              color: colors.onContainer,
              height: 1.1,
            ),
          ),
        ],
      ),
    );
  }
}

class _InterfaceSpeedTag extends StatelessWidget {
  const _InterfaceSpeedTag({
    required this.label,
    required this.isUp,
    required this.hasSpeed,
  });

  final String label;
  final bool isUp;
  final bool hasSpeed;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final activeColors = context.semanticColors.resolve(
      SemanticTone.info,
      scheme,
    );
    final highlight = isUp && hasSpeed;

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
      decoration: BoxDecoration(
        color: highlight ? activeColors.container : scheme.surfaceContainerHigh,
        borderRadius: BorderRadius.circular(6),
        border: Border.all(
          color: highlight ? activeColors.border : scheme.outlineVariant,
        ),
      ),
      child: Text(
        label,
        maxLines: 1,
        softWrap: false,
        style: TextStyle(
          fontSize: 11,
          fontWeight: FontWeight.w700,
          color: highlight ? activeColors.onContainer : scheme.onSurfaceVariant,
          height: 1.1,
        ),
      ),
    );
  }
}

class _InterfaceCountLegend extends StatelessWidget {
  const _InterfaceCountLegend({required this.label, required this.tone});

  final String label;
  final SemanticTone tone;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final colors = context.semanticColors.resolve(tone, scheme);
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        Container(
          width: 7,
          height: 7,
          decoration: BoxDecoration(
            color: colors.foreground,
            shape: BoxShape.circle,
          ),
        ),
        const SizedBox(width: 5),
        Text(
          label,
          style: context.textTheme.labelSmall?.copyWith(
            color: scheme.onSurfaceVariant,
            fontWeight: FontWeight.w600,
          ),
        ),
      ],
    );
  }
}

/// Modo de configuração do painel "Receber logs (Syslog)".
enum SyslogSetupMode { automatic, manual }

/// Método de autorização escolhido para a configuração automática de Syslog.
enum SyslogAccessMethod { dedicatedSshKey, oneTimePassword }

/// Painel coeso "Receber logs (Syslog)" no cadastro/edição de dispositivo.
///
/// Quando habilitado, inicia em modo **Automático** perguntando como o
/// NetMonitor deve se autenticar no equipamento (**Chave SSH dedicada** via
/// `POST /api/devices/{id}/access/dedicated` ou **Login e senha de uso único**
/// via `POST /api/logs/devices/{id}/provision`), com modo **Manual** como
/// alternativa contendo o destino UDP e a receita de comandos por SO.
class DeviceSyslogSetupPanel extends StatelessWidget {
  const DeviceSyslogSetupPanel({
    super.key,
    required this.enabled,
    required this.loading,
    required this.onToggle,
    this.guide,
    this.mode = SyslogSetupMode.automatic,
    this.accessMethod = SyslogAccessMethod.dedicatedSshKey,
    this.isExistingDevice = false,
    this.resolvedServerAddress,
    this.serverPort = 514,
    this.usableAddresses = const <ServerAddressEntry>[],
    this.serverAddressController,
    this.existingCredential,
    this.reuseExistingAccess = false,
    this.sshAdminUserController,
    this.sshAdminPasswordController,
    this.sshDedicatedPortController,
    this.oneTimeProtocol = 'ssh',
    this.oneTimeUserController,
    this.oneTimePasswordController,
    this.oneTimePortController,
    this.provisioning = false,
    this.provisionStep,
    this.provisionError,
    this.provisionResult,
    this.operatingSystemId,
    this.operatingSystemLabel,
    this.vendorHint,
    this.manualSnippetIndex,
    this.onModeChanged,
    this.onAccessMethodChanged,
    this.onReuseExistingAccessChanged,
    this.onOneTimeProtocolChanged,
    this.onSelectServerAddress,
    this.onSelectSnippetIndex,
    this.onRetryLoad,
    this.onSshAdminUserChanged,
    this.onOneTimeUserChanged,
    this.onProvisionNow,
  });

  static const String unknownServerPlaceholder = '<IP-DO-SERVIDOR>';

  final bool enabled;
  final bool loading;
  final SetupGuide? guide;
  final SyslogSetupMode mode;
  final SyslogAccessMethod accessMethod;
  final bool isExistingDevice;
  final String? resolvedServerAddress;
  final int serverPort;
  final List<ServerAddressEntry> usableAddresses;
  final TextEditingController? serverAddressController;
  final DeviceCredentialView? existingCredential;
  final bool reuseExistingAccess;
  final TextEditingController? sshAdminUserController;
  final TextEditingController? sshAdminPasswordController;
  final TextEditingController? sshDedicatedPortController;
  final String oneTimeProtocol;
  final TextEditingController? oneTimeUserController;
  final TextEditingController? oneTimePasswordController;
  final TextEditingController? oneTimePortController;
  final bool provisioning;
  final String? provisionStep;
  final String? provisionError;
  final ProvisionLoggingResponse? provisionResult;
  final String? operatingSystemId;
  final String? operatingSystemLabel;
  final String? vendorHint;
  final int? manualSnippetIndex;
  final ValueChanged<bool> onToggle;
  final ValueChanged<SyslogSetupMode>? onModeChanged;
  final ValueChanged<SyslogAccessMethod>? onAccessMethodChanged;
  final ValueChanged<bool>? onReuseExistingAccessChanged;
  final ValueChanged<String>? onOneTimeProtocolChanged;
  final ValueChanged<String?>? onSelectServerAddress;
  final ValueChanged<int?>? onSelectSnippetIndex;
  final VoidCallback? onRetryLoad;
  final ValueChanged<String>? onSshAdminUserChanged;
  final ValueChanged<String>? onOneTimeUserChanged;
  final VoidCallback? onProvisionNow;

  /// Resolve o melhor endereço real do servidor Syslog disponível, evitando
  /// `<IP-DO-SERVIDOR>` sempre que houver endereço configurado ou sugerido.
  static String? resolveBestServerAddress({
    String? selectedAddress,
    String? hintedAddress,
    List<ServerAddressEntry> usableAddresses = const <ServerAddressEntry>[],
    String? preferredAddressId,
    String? guideAddress,
  }) {
    bool isReal(String? value) {
      final clean = value?.trim() ?? '';
      return clean.isNotEmpty && clean != unknownServerPlaceholder;
    }

    if (isReal(selectedAddress)) return selectedAddress!.trim();
    if (isReal(hintedAddress)) return hintedAddress!.trim();
    if (preferredAddressId != null && preferredAddressId.isNotEmpty) {
      for (final entry in usableAddresses) {
        if (entry.id == preferredAddressId && isReal(entry.value)) {
          return entry.value!.trim();
        }
      }
    }
    for (final entry in usableAddresses) {
      if (isReal(entry.value)) return entry.value!.trim();
    }
    if (isReal(guideAddress)) return guideAddress!.trim();
    return null;
  }

  /// Infere a plataforma de acesso dedicado (`mikrotik`/`openwrt`) quando já
  /// conhecida; retorna `null` para deixar o backend detectar automaticamente.
  static String? inferDedicatedPlatform({
    String? osId,
    String? vendorHint,
    String? accessPlatform,
  }) {
    final explicit = accessPlatform?.trim().toLowerCase() ?? '';
    if (explicit == 'mikrotik' || explicit == 'openwrt') return explicit;
    final combined = '${osId ?? ''} ${vendorHint ?? ''}'.toLowerCase();
    if (combined.contains('openwrt') || combined.contains('lede')) {
      return 'openwrt';
    }
    if (combined.contains('routeros') || combined.contains('mikrotik')) {
      return 'mikrotik';
    }
    return null;
  }

  /// Resolve o índice da receita Syslog mais adequada ao SO / fabricante atual.
  static int? resolveSnippetIndex(
    List<SetupSnippet> snippets, {
    String? osId,
    String? osLabel,
    String? vendorHint,
  }) {
    if (snippets.isEmpty) return null;

    final idNeedle = osId?.trim().toLowerCase() ?? '';
    final labelNeedle = osLabel?.trim().toLowerCase() ?? '';
    final vendorNeedle = vendorHint?.trim().toLowerCase() ?? '';

    if (idNeedle.isNotEmpty) {
      for (var i = 0; i < snippets.length; i++) {
        if (snippets[i].id.trim().toLowerCase() == idNeedle) return i;
      }
      for (var i = 0; i < snippets.length; i++) {
        final combined = '${snippets[i].id} ${snippets[i].label}'.toLowerCase();
        if (combined.contains(idNeedle) ||
            _matchesOsAlias(idNeedle, combined)) {
          return i;
        }
      }
    }

    if (labelNeedle.isNotEmpty) {
      for (var i = 0; i < snippets.length; i++) {
        final label = snippets[i].label.toLowerCase();
        if (label.contains(labelNeedle) || labelNeedle.contains(label)) {
          return i;
        }
      }
    }

    if (vendorNeedle.isNotEmpty) {
      for (var i = 0; i < snippets.length; i++) {
        final combined = '${snippets[i].id} ${snippets[i].label}'.toLowerCase();
        if (combined.contains(vendorNeedle) ||
            _matchesOsAlias(vendorNeedle, combined)) {
          return i;
        }
      }
    }

    return 0;
  }

  static bool _matchesOsAlias(String needle, String haystack) {
    const aliasGroups = <List<String>>[
      <String>['mikrotik', 'routeros'],
      <String>['ubiquiti', 'unifi', 'edgeos', 'edgerouter', 'airos'],
      <String>['cisco', 'ios', 'nxos'],
      <String>['openwrt', 'lede'],
      <String>['linux', 'rsyslog', 'debian', 'ubuntu', 'systemd'],
      <String>['pfsense', 'opnsense', 'freebsd'],
      <String>['fortinet', 'fortigate', 'fortios'],
      <String>['huawei', 'vrp'],
      <String>['juniper', 'junos'],
    ];
    for (final group in aliasGroups) {
      final needleInGroup = group.any(needle.contains);
      if (needleInGroup && group.any(haystack.contains)) {
        return true;
      }
    }
    return false;
  }

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final snippets = guide?.snippets ?? const <SetupSnippet>[];
    final autoIdx = resolveSnippetIndex(
      snippets,
      osId: operatingSystemId,
      osLabel: operatingSystemLabel,
      vendorHint: vendorHint,
    );
    final selectedIdx =
        (manualSnippetIndex != null &&
            manualSnippetIndex! >= 0 &&
            manualSnippetIndex! < snippets.length)
        ? manualSnippetIndex!
        : autoIdx;
    final activeSnippet = (selectedIdx != null && selectedIdx < snippets.length)
        ? snippets[selectedIdx]
        : null;

    return NestedPanel(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Material(
            type: MaterialType.transparency,
            child: SwitchListTile(
              contentPadding: EdgeInsets.zero,
              title: const Text('Receber logs (Syslog)'),
              subtitle: const Text(
                'Configuração automática pelo servidor ou comandos manuais por SO',
              ),
              value: enabled,
              onChanged: provisioning ? null : onToggle,
            ),
          ),
          if (enabled) ...<Widget>[
            const SizedBox(height: 8),
            SegmentedButton<SyslogSetupMode>(
              showSelectedIcon: false,
              segments: const <ButtonSegment<SyslogSetupMode>>[
                ButtonSegment<SyslogSetupMode>(
                  value: SyslogSetupMode.automatic,
                  icon: Icon(Icons.auto_fix_high, size: 16),
                  label: Text(
                    'Automático',
                    maxLines: 1,
                    softWrap: false,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                ButtonSegment<SyslogSetupMode>(
                  value: SyslogSetupMode.manual,
                  icon: Icon(Icons.terminal, size: 16),
                  label: Text(
                    'Manual',
                    maxLines: 1,
                    softWrap: false,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              ],
              selected: <SyslogSetupMode>{mode},
              onSelectionChanged: provisioning
                  ? null
                  : (sel) => onModeChanged?.call(sel.first),
            ),
            const SizedBox(height: 12),
            _SyslogTargetAddressSection(
              resolvedServerAddress: resolvedServerAddress,
              serverPort: serverPort,
              usableAddresses: usableAddresses,
              serverAddressController: serverAddressController,
              loading: loading,
              onSelectServerAddress: onSelectServerAddress,
            ),
            const SizedBox(height: 12),
            if (mode == SyslogSetupMode.automatic)
              _SyslogAutomaticSetupSection(
                isExistingDevice: isExistingDevice,
                accessMethod: accessMethod,
                existingCredential: existingCredential,
                reuseExistingAccess: reuseExistingAccess,
                sshAdminUserController: sshAdminUserController,
                sshAdminPasswordController: sshAdminPasswordController,
                sshDedicatedPortController: sshDedicatedPortController,
                oneTimeProtocol: oneTimeProtocol,
                oneTimeUserController: oneTimeUserController,
                oneTimePasswordController: oneTimePasswordController,
                oneTimePortController: oneTimePortController,
                provisioning: provisioning,
                provisionStep: provisionStep,
                provisionError: provisionError,
                provisionResult: provisionResult,
                onAccessMethodChanged: onAccessMethodChanged,
                onReuseExistingAccessChanged: onReuseExistingAccessChanged,
                onOneTimeProtocolChanged: onOneTimeProtocolChanged,
                onSshAdminUserChanged: onSshAdminUserChanged,
                onOneTimeUserChanged: onOneTimeUserChanged,
                onProvisionNow: onProvisionNow,
                onSwitchToManual: () =>
                    onModeChanged?.call(SyslogSetupMode.manual),
              )
            else
              _SyslogManualSetupSection(
                loading: loading,
                guide: guide,
                resolvedServerAddress: resolvedServerAddress,
                serverPort: serverPort,
                snippets: snippets,
                selectedIdx: selectedIdx,
                activeSnippet: activeSnippet,
                onSelectSnippetIndex: onSelectSnippetIndex,
                onRetryLoad: onRetryLoad,
                onSurfaceVariant: scheme.onSurfaceVariant,
              ),
          ],
        ],
      ),
    );
  }
}

class _SyslogTargetAddressSection extends StatelessWidget {
  const _SyslogTargetAddressSection({
    required this.resolvedServerAddress,
    required this.serverPort,
    required this.usableAddresses,
    required this.serverAddressController,
    required this.loading,
    required this.onSelectServerAddress,
  });

  final String? resolvedServerAddress;
  final int serverPort;
  final List<ServerAddressEntry> usableAddresses;
  final TextEditingController? serverAddressController;
  final bool loading;
  final ValueChanged<String?>? onSelectServerAddress;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final hasRealAddress =
        resolvedServerAddress != null && resolvedServerAddress!.isNotEmpty;
    final displayTarget = hasRealAddress
        ? '$resolvedServerAddress:$serverPort'
        : '${DeviceSyslogSetupPanel.unknownServerPlaceholder}:$serverPort';
    final uniqueAddresses = <ServerAddressEntry>[];
    final seenValues = <String>{};
    for (final entry in usableAddresses) {
      final val = entry.value?.trim() ?? '';
      if (val.isNotEmpty && seenValues.add(val)) {
        uniqueAddresses.add(entry);
      }
    }
    final dropdownMatches =
        hasRealAddress &&
        uniqueAddresses.any((e) => e.value?.trim() == resolvedServerAddress);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        KeyValueRow(
          label: 'Destino Syslog (UDP)',
          value: displayTarget,
          monospace: true,
          copyable: hasRealAddress,
        ),
        if (uniqueAddresses.length > 1) ...<Widget>[
          const SizedBox(height: 8),
          AppDropdownFormField<String>(
            key: ValueKey<String>(
              'syslog-server-addr-${dropdownMatches ? resolvedServerAddress : 'first'}-${uniqueAddresses.length}',
            ),
            initialValue: dropdownMatches
                ? resolvedServerAddress!
                : uniqueAddresses.first.value!.trim(),
            decoration: const InputDecoration(
              labelText: 'Endereço deste servidor (alvo Syslog)',
              isDense: true,
            ),
            items: <DropdownMenuItem<String>>[
              for (final entry in uniqueAddresses)
                DropdownMenuItem<String>(
                  value: entry.value!.trim(),
                  child: Text(
                    '${entry.label} (${entry.value!.trim()})',
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
            ],
            onChanged: (v) => onSelectServerAddress?.call(v),
          ),
        ] else if (!hasRealAddress &&
            !loading &&
            serverAddressController != null) ...<Widget>[
          const SizedBox(height: 8),
          TextFormField(
            controller: serverAddressController,
            decoration: const InputDecoration(
              labelText: 'Endereço deste servidor (opcional)',
              hintText: 'Ex.: 192.168.1.10',
              helperText:
                  'IP pelo qual o equipamento alcança o NetMonitor na rede.',
            ),
            onFieldSubmitted: (v) => onSelectServerAddress?.call(v),
          ),
        ],
        if (loading && !hasRealAddress) ...<Widget>[
          const SizedBox(height: 6),
          Row(
            children: <Widget>[
              const SizedBox(
                width: 12,
                height: 12,
                child: CircularProgressIndicator(strokeWidth: 2),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  'Detectando endereço do servidor Syslog...',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: context.textTheme.bodySmall?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
              ),
            ],
          ),
        ],
      ],
    );
  }
}

class _SyslogAutomaticSetupSection extends StatelessWidget {
  const _SyslogAutomaticSetupSection({
    required this.isExistingDevice,
    required this.accessMethod,
    required this.existingCredential,
    required this.reuseExistingAccess,
    required this.sshAdminUserController,
    required this.sshAdminPasswordController,
    required this.sshDedicatedPortController,
    required this.oneTimeProtocol,
    required this.oneTimeUserController,
    required this.oneTimePasswordController,
    required this.oneTimePortController,
    required this.provisioning,
    required this.provisionStep,
    required this.provisionError,
    required this.provisionResult,
    required this.onAccessMethodChanged,
    required this.onReuseExistingAccessChanged,
    required this.onOneTimeProtocolChanged,
    this.onSshAdminUserChanged,
    this.onOneTimeUserChanged,
    required this.onProvisionNow,
    required this.onSwitchToManual,
  });

  final bool isExistingDevice;
  final SyslogAccessMethod accessMethod;
  final DeviceCredentialView? existingCredential;
  final bool reuseExistingAccess;
  final TextEditingController? sshAdminUserController;
  final TextEditingController? sshAdminPasswordController;
  final TextEditingController? sshDedicatedPortController;
  final String oneTimeProtocol;
  final TextEditingController? oneTimeUserController;
  final TextEditingController? oneTimePasswordController;
  final TextEditingController? oneTimePortController;
  final bool provisioning;
  final String? provisionStep;
  final String? provisionError;
  final ProvisionLoggingResponse? provisionResult;
  final ValueChanged<SyslogAccessMethod>? onAccessMethodChanged;
  final ValueChanged<bool>? onReuseExistingAccessChanged;
  final ValueChanged<String>? onOneTimeProtocolChanged;
  final ValueChanged<String>? onSshAdminUserChanged;
  final ValueChanged<String>? onOneTimeUserChanged;
  final VoidCallback? onProvisionNow;
  final VoidCallback onSwitchToManual;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final hasReusable = existingCredential != null;
    final usingExisting = hasReusable && reuseExistingAccess;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (hasReusable) ...<Widget>[
          _SyslogExistingAccessBanner(
            credential: existingCredential!,
            reusing: usingExisting,
            busy: provisioning,
            onToggleReuse: (v) => onReuseExistingAccessChanged?.call(v),
          ),
          const SizedBox(height: 12),
        ],
        if (!usingExisting) ...<Widget>[
          Text(
            'Como liberar o acesso do NetMonitor ao equipamento?',
            style: context.textTheme.titleSmall?.copyWith(
              fontWeight: FontWeight.w700,
              color: scheme.onSurface,
            ),
          ),
          const SizedBox(height: 8),
          ChoiceCard(
            title: 'Chave SSH dedicada',
            subtitle:
                'Cria conta dedicada por chave SSH sem salvar a senha administrativa.',
            icon: Icons.vpn_key_outlined,
            tone: SemanticTone.plugin,
            badge: 'Recomendado',
            badgeTone: SemanticTone.plugin,
            selected: accessMethod == SyslogAccessMethod.dedicatedSshKey,
            enabled: !provisioning,
            onTap: () =>
                onAccessMethodChanged?.call(SyslogAccessMethod.dedicatedSshKey),
          ),
          const SizedBox(height: 8),
          ChoiceCard(
            title: 'Login e senha (uso único)',
            subtitle: 'Usado apenas nesta configuração — não fica salvo.',
            icon: Icons.password_outlined,
            tone: SemanticTone.primary,
            selected: accessMethod == SyslogAccessMethod.oneTimePassword,
            enabled: !provisioning,
            onTap: () =>
                onAccessMethodChanged?.call(SyslogAccessMethod.oneTimePassword),
          ),
          const SizedBox(height: 12),
          if (accessMethod == SyslogAccessMethod.dedicatedSshKey)
            _SyslogDedicatedSshFields(
              userController: sshAdminUserController,
              passwordController: sshAdminPasswordController,
              portController: sshDedicatedPortController,
              enabled: !provisioning,
              onUserChanged: onSshAdminUserChanged,
            )
          else
            _SyslogOneTimeLoginFields(
              protocol: oneTimeProtocol,
              userController: oneTimeUserController,
              passwordController: oneTimePasswordController,
              portController: oneTimePortController,
              enabled: !provisioning,
              onProtocolChanged: onOneTimeProtocolChanged,
              onUserChanged: onOneTimeUserChanged,
            ),
        ],
        if (provisioning && provisionStep != null) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(
            tone: SemanticTone.info,
            padding: const EdgeInsets.all(10),
            child: Row(
              children: <Widget>[
                const SizedBox(
                  width: 16,
                  height: 16,
                  child: CircularProgressIndicator(strokeWidth: 2),
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: Text(
                    provisionStep!,
                    style: context.textTheme.bodySmall?.copyWith(
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                ),
              ],
            ),
          ),
        ],
        if (provisionError != null) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(
            tone: SemanticTone.error,
            padding: const EdgeInsets.all(12),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Text(provisionError!, style: context.textTheme.bodySmall),
                const SizedBox(height: 6),
                Align(
                  alignment: AlignmentDirectional.centerStart,
                  child: TextButton.icon(
                    onPressed: provisioning ? null : onSwitchToManual,
                    style: TextButton.styleFrom(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 8,
                        vertical: 4,
                      ),
                      minimumSize: const Size(0, 32),
                      tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                    ),
                    icon: const Icon(Icons.terminal, size: 16),
                    label: const Text('Ver comandos manuais'),
                  ),
                ),
              ],
            ),
          ),
        ],
        if (provisionResult != null) ...<Widget>[
          const SizedBox(height: 12),
          _SyslogProvisionResultBanner(result: provisionResult!),
        ],
        const SizedBox(height: 12),
        if (isExistingDevice && onProvisionNow != null)
          Align(
            alignment: AlignmentDirectional.centerEnd,
            child: FilledButton.tonalIcon(
              onPressed: provisioning ? null : onProvisionNow,
              icon: provisioning
                  ? const SizedBox(
                      width: 16,
                      height: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(Icons.play_arrow_rounded, size: 18),
              label: Text(
                provisionResult != null
                    ? 'Reaplicar Syslog agora'
                    : 'Configurar Syslog agora',
              ),
            ),
          )
        else
          NestedPanel(
            tone: SemanticTone.info,
            padding: const EdgeInsets.all(10),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Icon(
                  Icons.info_outline,
                  size: 18,
                  color: context.semanticColors
                      .resolve(SemanticTone.info, scheme)
                      .onContainer,
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    'Ao cadastrar, o NetMonitor autorizará o acesso e configurará o Syslog automaticamente.',
                    style: context.textTheme.bodySmall,
                  ),
                ),
              ],
            ),
          ),
      ],
    );
  }
}

class _SyslogExistingAccessBanner extends StatelessWidget {
  const _SyslogExistingAccessBanner({
    required this.credential,
    required this.reusing,
    required this.busy,
    required this.onToggleReuse,
  });

  final DeviceCredentialView credential;
  final bool reusing;
  final bool busy;
  final ValueChanged<bool> onToggleReuse;

  @override
  Widget build(BuildContext context) {
    final isKey = credential.dedicated || credential.storage == 'key';
    final summary =
        '${isKey ? 'Chave SSH dedicada' : credential.kind.toUpperCase()} · '
        '${credential.username ?? '—'}'
        '${credential.port != null ? ' (porta ${credential.port})' : ''}';

    return NestedPanel(
      tone: SemanticTone.success,
      padding: const EdgeInsets.all(12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Wrap(
            spacing: 8,
            runSpacing: 6,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: <Widget>[
              const SemanticChip(
                label: 'Acesso já configurado',
                tone: SemanticTone.success,
                compact: true,
                icon: Icons.verified_user_outlined,
              ),
              Text(
                summary,
                style: context.textTheme.bodySmall?.copyWith(
                  fontWeight: FontWeight.w600,
                ),
              ),
            ],
          ),
          const SizedBox(height: 8),
          Wrap(
            spacing: 8,
            runSpacing: 6,
            children: <Widget>[
              if (reusing)
                OutlinedButton.icon(
                  onPressed: busy ? null : () => onToggleReuse(false),
                  icon: const Icon(Icons.tune, size: 16),
                  label: const Text('Reconfigurar acesso'),
                )
              else
                FilledButton.tonalIcon(
                  onPressed: busy ? null : () => onToggleReuse(true),
                  icon: const Icon(Icons.check, size: 16),
                  label: const Text('Reusar acesso configurado'),
                ),
            ],
          ),
        ],
      ),
    );
  }
}

class _SyslogDedicatedSshFields extends StatelessWidget {
  const _SyslogDedicatedSshFields({
    required this.userController,
    required this.passwordController,
    required this.portController,
    required this.enabled,
    this.onUserChanged,
  });

  final TextEditingController? userController;
  final TextEditingController? passwordController;
  final TextEditingController? portController;
  final bool enabled;
  final ValueChanged<String>? onUserChanged;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        ResponsiveFieldRow(
          breakpoint: 480,
          children: <Widget>[
            TextFormField(
              controller: userController,
              enabled: enabled,
              onChanged: onUserChanged,
              decoration: const InputDecoration(
                labelText: 'Usuário admin atual',
                hintText: 'Ex.: root ou admin',
              ),
            ),
            TextFormField(
              controller: portController,
              enabled: enabled,
              keyboardType: TextInputType.number,
              decoration: const InputDecoration(labelText: 'Porta SSH'),
            ),
          ],
        ),
        const SizedBox(height: 10),
        TextFormField(
          controller: passwordController,
          enabled: enabled,
          obscureText: true,
          decoration: const InputDecoration(
            labelText: 'Senha admin (uso único)',
            helperText:
                'Usada uma única vez para instalar a chave SSH dedicada e descartada em seguida.',
          ),
        ),
      ],
    );
  }
}

class _SyslogOneTimeLoginFields extends StatelessWidget {
  const _SyslogOneTimeLoginFields({
    required this.protocol,
    required this.userController,
    required this.passwordController,
    required this.portController,
    required this.enabled,
    required this.onProtocolChanged,
    this.onUserChanged,
  });

  final String protocol;
  final TextEditingController? userController;
  final TextEditingController? passwordController;
  final TextEditingController? portController;
  final bool enabled;
  final ValueChanged<String>? onProtocolChanged;
  final ValueChanged<String>? onUserChanged;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        ResponsiveFieldRow(
          breakpoint: 480,
          children: <Widget>[
            AppDropdownFormField<String>(
              key: ValueKey<String>('syslog-onetime-proto-$protocol'),
              initialValue: protocol,
              decoration: const InputDecoration(labelText: 'Protocolo'),
              items: const <DropdownMenuItem<String>>[
                DropdownMenuItem<String>(
                  value: 'ssh',
                  child: Text('SSH', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem<String>(
                  value: 'telnet',
                  child: Text('Telnet', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem<String>(
                  value: 'mactelnet',
                  child: Text('MAC-Telnet', overflow: TextOverflow.ellipsis),
                ),
              ],
              onChanged: enabled
                  ? (v) {
                      if (v != null) onProtocolChanged?.call(v);
                    }
                  : null,
            ),
            TextFormField(
              controller: portController,
              enabled: enabled,
              keyboardType: TextInputType.number,
              decoration: const InputDecoration(labelText: 'Porta'),
            ),
          ],
        ),
        const SizedBox(height: 10),
        ResponsiveFieldRow(
          breakpoint: 480,
          children: <Widget>[
            TextFormField(
              controller: userController,
              enabled: enabled,
              onChanged: onUserChanged,
              decoration: const InputDecoration(
                labelText: 'Usuário',
                hintText: 'Ex.: root ou admin',
              ),
            ),
            TextFormField(
              controller: passwordController,
              enabled: enabled,
              obscureText: true,
              decoration: const InputDecoration(
                labelText: 'Senha (uso único)',
                helperText: 'Usada apenas agora e descartada.',
              ),
            ),
          ],
        ),
      ],
    );
  }
}

class _SyslogProvisionResultBanner extends StatelessWidget {
  const _SyslogProvisionResultBanner({required this.result});

  final ProvisionLoggingResponse result;

  @override
  Widget build(BuildContext context) {
    final confirmed = result.confirmed == true;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        NestedPanel(
          tone: confirmed ? SemanticTone.success : SemanticTone.info,
          padding: const EdgeInsets.all(12),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Text(
                confirmed
                    ? 'Syslog configurado e confirmado!'
                    : 'Comandos Syslog aplicados no equipamento.',
                style: context.textTheme.titleSmall?.copyWith(
                  fontWeight: FontWeight.w700,
                ),
              ),
              if (result.identifiedHostname != null &&
                  result.identifiedHostname!.isNotEmpty) ...<Widget>[
                const SizedBox(height: 4),
                Text(
                  'Identidade detectada: ${result.identifiedHostname}',
                  style: context.textTheme.bodySmall,
                ),
              ],
              if (result.persistenceWarning != null &&
                  result.persistenceWarning!.isNotEmpty) ...<Widget>[
                const SizedBox(height: 4),
                Text(
                  result.persistenceWarning!,
                  style: context.textTheme.bodySmall,
                ),
              ],
            ],
          ),
        ),
        if (result.transcript.isNotEmpty) ...<Widget>[
          const SizedBox(height: 8),
          CodeViewer(
            title: 'Transcrição do equipamento',
            code: result.transcript,
            maxHeight: 160,
          ),
        ],
      ],
    );
  }
}

class _SyslogManualSetupSection extends StatelessWidget {
  const _SyslogManualSetupSection({
    required this.loading,
    required this.guide,
    required this.resolvedServerAddress,
    required this.serverPort,
    required this.snippets,
    required this.selectedIdx,
    required this.activeSnippet,
    required this.onSelectSnippetIndex,
    required this.onRetryLoad,
    required this.onSurfaceVariant,
  });

  final bool loading;
  final SetupGuide? guide;
  final String? resolvedServerAddress;
  final int serverPort;
  final List<SetupSnippet> snippets;
  final int? selectedIdx;
  final SetupSnippet? activeSnippet;
  final ValueChanged<int?>? onSelectSnippetIndex;
  final VoidCallback? onRetryLoad;
  final Color onSurfaceVariant;

  @override
  Widget build(BuildContext context) {
    if (loading && guide == null) {
      return Row(
        children: <Widget>[
          const SizedBox(
            width: 14,
            height: 14,
            child: CircularProgressIndicator(strokeWidth: 2),
          ),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              'Carregando comandos Syslog...',
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: context.textTheme.bodySmall?.copyWith(
                color: onSurfaceVariant,
              ),
            ),
          ),
        ],
      );
    }

    if (guide == null) {
      return Wrap(
        spacing: 8,
        runSpacing: 8,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: <Widget>[
          Text(
            'Não foi possível carregar o guia Syslog do servidor.',
            style: context.textTheme.bodySmall?.copyWith(
              color: onSurfaceVariant,
            ),
          ),
          if (onRetryLoad != null)
            OutlinedButton.icon(
              onPressed: onRetryLoad,
              icon: const Icon(Icons.refresh, size: 16),
              label: const Text('Tentar novamente'),
            ),
        ],
      );
    }

    final targetDisplay =
        resolvedServerAddress ??
        guide?.serverAddress ??
        DeviceSyslogSetupPanel.unknownServerPlaceholder;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (snippets.length > 1) ...<Widget>[
          AppDropdownFormField<int>(
            key: ValueKey<String>(
              'syslog-recipe-$selectedIdx-${snippets.length}',
            ),
            initialValue: selectedIdx ?? 0,
            decoration: const InputDecoration(
              labelText: 'Receita de configuração (SO)',
            ),
            items: <DropdownMenuItem<int>>[
              for (var i = 0; i < snippets.length; i++)
                DropdownMenuItem<int>(
                  value: i,
                  child: Text(
                    snippets[i].label,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
            ],
            onChanged: (idx) => onSelectSnippetIndex?.call(idx),
          ),
          const SizedBox(height: 10),
        ],
        if (activeSnippet != null) ...<Widget>[
          CodeViewer(
            title: activeSnippet!.label,
            code: activeSnippet!.commands,
            maxHeight: 200,
          ),
          if (activeSnippet!.notes != null &&
              activeSnippet!.notes!.isNotEmpty) ...<Widget>[
            const SizedBox(height: 6),
            Text(
              activeSnippet!.notes!,
              style: context.textTheme.bodySmall?.copyWith(
                color: onSurfaceVariant,
              ),
            ),
          ],
        ] else
          Text(
            'Configure o equipamento para enviar Syslog UDP para '
            '$targetDisplay:$serverPort.',
            style: context.textTheme.bodySmall?.copyWith(
              color: onSurfaceVariant,
            ),
          ),
      ],
    );
  }
}
