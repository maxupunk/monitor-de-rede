import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/features/vpn/models/vpn_models.dart';

/// Repositório REST do domínio VPN WireGuard (13 endpoints dedicados + leituras auxiliares).
class VpnRepository {
  const VpnRepository(this._api);

  final ApiClient _api;

  Future<VpnServerStateModel> fetchServer() async {
    final json = await _api.get<Map<String, dynamic>>('/vpn/server');
    return VpnServerStateModel.fromJson(json);
  }

  Future<void> saveServer({
    required String? publicEndpoint,
    required int listenPort,
    required String cidr,
    required int mtu,
    required String? dnsServers,
    required bool allowPeerToPeer,
  }) async {
    await _api.put<dynamic>(
      '/vpn/server',
      data: <String, dynamic>{
        'publicEndpoint':
            publicEndpoint != null && publicEndpoint.trim().isNotEmpty
            ? publicEndpoint.trim()
            : null,
        'listenPort': listenPort,
        'cidr': cidr.trim(),
        'mtu': mtu,
        'dnsServers': dnsServers != null && dnsServers.trim().isNotEmpty
            ? dnsServers.trim()
            : null,
        'allowPeerToPeer': allowPeerToPeer,
      },
    );
  }

  Future<VpnPreflightResult> runPreflight({
    required String? publicEndpoint,
    required int listenPort,
  }) async {
    final json = await _api.post<Map<String, dynamic>>(
      '/vpn/server/preflight',
      data: <String, dynamic>{
        'publicEndpoint': publicEndpoint,
        'listenPort': listenPort,
      },
    );
    return VpnPreflightResult.fromJson(json);
  }

  Future<String?> detectEndpoint() async {
    final json = await _api.post<Map<String, dynamic>>(
      '/vpn/server/detect-endpoint',
    );
    return json['publicEndpoint'] as String?;
  }

  Future<List<VpnPeer>> fetchPeers() async {
    final raw = await _api.get<List<dynamic>>('/vpn/peers');
    return raw
        .whereType<Map>()
        .map((item) => VpnPeer.fromJson(Map<String, dynamic>.from(item)))
        .toList(growable: false);
  }

  Future<String> suggestNextIp() async {
    final json = await _api.get<Map<String, dynamic>>('/vpn/peers/next-ip');
    return (json['ipAddress'] as String?) ?? '';
  }

  Future<VpnArtifact> createPeer(CreateVpnPeerPayload payload) async {
    final json = await _api.post<Map<String, dynamic>>(
      '/vpn/peers',
      data: payload.toJson(),
    );
    final rawArtifact = json['artifact'];
    if (rawArtifact is Map) {
      return VpnArtifact.fromJson(Map<String, dynamic>.from(rawArtifact));
    }
    return VpnArtifact.fromJson(json);
  }

  Future<VpnPeer> renamePeer(int peerId, String name) async {
    final json = await _api.patch<Map<String, dynamic>>(
      '/vpn/peers/$peerId',
      data: <String, dynamic>{'name': name.trim()},
    );
    return VpnPeer.fromJson(json);
  }

  Future<VpnArtifact> fetchConfig(int peerId) async {
    final json = await _api.get<Map<String, dynamic>>(
      '/vpn/peers/$peerId/config',
    );
    return VpnArtifact.fromJson(json);
  }

  Future<VpnArtifact> rotateKeys(
    int peerId, {
    VpnInstallPolicy? agentPolicy,
  }) async {
    final json = await _api.post<Map<String, dynamic>>(
      '/vpn/peers/$peerId/rotate',
      data: agentPolicy != null
          ? <String, dynamic>{'agentPolicy': agentPolicy.toJson()}
          : null,
    );
    final rawArtifact = json['artifact'];
    if (rawArtifact is Map) {
      return VpnArtifact.fromJson(Map<String, dynamic>.from(rawArtifact));
    }
    return VpnArtifact.fromJson(json);
  }

  Future<
    ({VpnArtifactAgentInstall agentInstall, List<VpnArtifactVariant> variants})
  >
  updatePeerAgentPolicy({
    required int peerId,
    required String enrollCode,
    required VpnInstallPolicy policy,
    required String confContent,
  }) async {
    final json = await _api.post<Map<String, dynamic>>(
      '/vpn/peers/$peerId/agent-policy',
      data: <String, dynamic>{
        'enrollCode': enrollCode,
        'policy': policy.toJson(),
        'confContent': confContent,
      },
    );
    final rawInstall = json['agentInstall'];
    final rawVariants = json['variants'];
    return (
      agentInstall: rawInstall is Map
          ? VpnArtifactAgentInstall.fromJson(
              Map<String, dynamic>.from(rawInstall),
            )
          : VpnArtifactAgentInstall(enrollCode: enrollCode, policy: policy),
      variants: rawVariants is List
          ? rawVariants
                .whereType<Map>()
                .map(
                  (e) =>
                      VpnArtifactVariant.fromJson(Map<String, dynamic>.from(e)),
                )
                .toList(growable: false)
          : const <VpnArtifactVariant>[],
    );
  }

  Future<String> fetchFirewallHints(int peerId) async {
    final json = await _api.post<Map<String, dynamic>>(
      '/vpn/peers/$peerId/firewall-hints',
    );
    return (json['content'] as String?) ?? '';
  }

  Future<void> revokePeer(int peerId) async {
    await _api.delete<dynamic>('/vpn/peers/$peerId');
  }

  // --- Leituras auxiliares para o formulário VPN e DeviceVpnTab ---

  Future<List<VpnServerAddressHint>> fetchServerAddresses() async {
    try {
      final raw = await _api.get<dynamic>('/server-addresses');
      final list = raw is List ? raw : (raw is Map ? raw['addresses'] : null);
      if (list is! List) return const <VpnServerAddressHint>[];
      return list
          .whereType<Map>()
          .map(
            (e) => VpnServerAddressHint.fromJson(Map<String, dynamic>.from(e)),
          )
          .where((item) => item.address.trim().isNotEmpty)
          .toList(growable: false);
    } catch (_) {
      return const <VpnServerAddressHint>[];
    }
  }

  Future<List<String>> fetchExistingNetworkCidrs() async {
    try {
      final raw = await _api.get<dynamic>('/networks');
      if (raw is! List) return const <String>[];
      return raw
          .whereType<Map>()
          .map((e) => e['cidr']?.toString() ?? '')
          .where((c) => c.trim().isNotEmpty)
          .toList(growable: false);
    } catch (_) {
      return const <String>[];
    }
  }

  Future<List<VpnSiteOption>> fetchSites() async {
    try {
      final raw = await _api.get<dynamic>('/sites');
      if (raw is! List) return const <VpnSiteOption>[];
      return raw
          .whereType<Map>()
          .map((e) => VpnSiteOption.fromJson(Map<String, dynamic>.from(e)))
          .where((s) => s.id > 0)
          .toList(growable: false);
    } catch (_) {
      return const <VpnSiteOption>[];
    }
  }

  Future<VpnInstallPolicy> fetchSuggestedAgentPolicy() async {
    try {
      final json = await _api.get<Map<String, dynamic>>(
        '/agents/suggested-policy',
      );
      return VpnInstallPolicy.fromJson(json);
    } catch (_) {
      return const VpnInstallPolicy(allow: <String>['metrics', 'docker_read']);
    }
  }

  Future<List<VpnMetricSample>> fetchDeviceVpnMetrics(int deviceId) async {
    try {
      final raw = await _api.get<dynamic>('/devices/$deviceId/metrics');
      if (raw is! List) return const <VpnMetricSample>[];
      final samples = <VpnMetricSample>[];
      for (final item in raw) {
        if (item is! Map) continue;
        final name = (item['metricName'] ?? item['name'])?.toString() ?? '';
        if (name != 'vpn_rx_bps' && name != 'vpn_tx_bps') continue;
        final val = item['value'];
        final ts = DateTime.tryParse(
          (item['recordedAt'] ?? item['createdAt'])?.toString() ?? '',
        );
        if (val is num && ts != null) {
          samples.add(
            VpnMetricSample(
              metricName: name,
              value: val.toDouble(),
              recordedAt: ts.toLocal(),
            ),
          );
        }
      }
      samples.sort((a, b) => a.recordedAt.compareTo(b.recordedAt));
      return samples;
    } catch (_) {
      return const <VpnMetricSample>[];
    }
  }
}

final vpnRepositoryProvider = Provider<VpnRepository>((ref) {
  return VpnRepository(ref.watch(apiClientProvider));
});
