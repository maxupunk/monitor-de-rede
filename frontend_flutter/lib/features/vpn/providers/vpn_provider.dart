import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/features/vpn/data/vpn_repository.dart';
import 'package:netmonitor_app/features/vpn/models/vpn_models.dart';

@immutable
class VpnState {
  const VpnState({
    this.serverState,
    this.peers = const <VpnPeer>[],
    this.preflight,
    this.lastArtifact,
    this.addresses = const <VpnServerAddressHint>[],
    this.existingCidrs = const <String>[],
    this.sites = const <VpnSiteOption>[],
    this.loading = false,
    this.saving = false,
    this.testing = false,
    this.error,
  });

  final VpnServerStateModel? serverState;
  final List<VpnPeer> peers;
  final VpnPreflightResult? preflight;

  /// Cofre efêmero em memória: limpo imediatamente ao fechar o `VpnScriptViewer`.
  final VpnArtifact? lastArtifact;
  final List<VpnServerAddressHint> addresses;
  final List<String> existingCidrs;
  final List<VpnSiteOption> sites;
  final bool loading;
  final bool saving;
  final bool testing;
  final String? error;

  bool get isConfigured => serverState?.configured == true;
  List<VpnProfileCard> get profiles =>
      serverState?.profiles ?? const <VpnProfileCard>[];

  VpnPeer? peerForDevice(int deviceId) {
    for (final peer in peers) {
      if (peer.deviceId == deviceId) return peer;
    }
    return null;
  }

  VpnState copyWith({
    VpnServerStateModel? serverState,
    List<VpnPeer>? peers,
    VpnPreflightResult? preflight,
    bool clearPreflight = false,
    VpnArtifact? lastArtifact,
    bool clearLastArtifact = false,
    List<VpnServerAddressHint>? addresses,
    List<String>? existingCidrs,
    List<VpnSiteOption>? sites,
    bool? loading,
    bool? saving,
    bool? testing,
    String? error,
    bool clearError = false,
  }) {
    return VpnState(
      serverState: serverState ?? this.serverState,
      peers: peers ?? this.peers,
      preflight: clearPreflight ? null : (preflight ?? this.preflight),
      lastArtifact: clearLastArtifact
          ? null
          : (lastArtifact ?? this.lastArtifact),
      addresses: addresses ?? this.addresses,
      existingCidrs: existingCidrs ?? this.existingCidrs,
      sites: sites ?? this.sites,
      loading: loading ?? this.loading,
      saving: saving ?? this.saving,
      testing: testing ?? this.testing,
      error: clearError ? null : (error ?? this.error),
    );
  }
}

/// Estado global da área VPN WireGuard — atualizações ao vivo via SSE (`vpn:peers_updated`).
class VpnNotifier extends Notifier<VpnState> {
  @override
  VpnState build() => const VpnState();

  VpnRepository get _repo => ref.read(vpnRepositoryProvider);

  String _describe(Object err, String fallback) {
    if (err is ApiException && err.message.isNotEmpty) return err.message;
    if (err is NetworkException && err.message.isNotEmpty) return err.message;
    return fallback;
  }

  /// Aplica snapshot SSE `vpn:peers_updated` diretamente em memória (zero polling).
  void applyRealtimePeers(Map<String, dynamic> data) {
    final raw = data['peers'];
    if (raw is! List || raw.isEmpty) return;

    final updatesById = <int, Map<String, dynamic>>{};
    for (final item in raw) {
      if (item is Map && item['id'] is num) {
        updatesById[(item['id'] as num).toInt()] = Map<String, dynamic>.from(
          item,
        );
      }
    }
    if (updatesById.isEmpty) return;

    var changed = false;
    var connectedCount = 0;
    var totalRx = 0;
    var totalTx = 0;

    final nextPeers = state.peers
        .map((peer) {
          final update = updatesById[peer.id];
          if (update == null) {
            if (peer.connectionStatus == VpnConnectionStatus.connected) {
              connectedCount++;
            }
            totalRx += peer.bytesRx;
            totalTx += peer.bytesTx;
            return peer;
          }
          changed = true;
          final status = VpnConnectionStatus.fromRaw(
            update['connectionStatus'],
          );
          final rx = (update['bytesRx'] as num?)?.toInt() ?? peer.bytesRx;
          final tx = (update['bytesTx'] as num?)?.toInt() ?? peer.bytesTx;
          if (status == VpnConnectionStatus.connected) connectedCount++;
          totalRx += rx;
          totalTx += tx;
          return peer.copyWith(
            connectionStatus: status,
            lastHandshakeAt: update['lastHandshakeAt'] as String?,
            clearLastHandshakeAt: update['lastHandshakeAt'] == null,
            lastSeenAt: update['lastSeenAt'] as String?,
            clearLastSeenAt: update['lastSeenAt'] == null,
            bytesRx: rx,
            bytesTx: tx,
            needsFirewallHint: update['needsFirewallHint'] == true,
            pingOutsideTunnel: update['pingOutsideTunnel'] == true,
            pingMonitorId: (update['pingMonitorId'] as num?)?.toInt(),
            clearPingMonitorId: update['pingMonitorId'] == null,
          );
        })
        .toList(growable: false);

    if (!changed) return;

    final currentServer = state.serverState;
    final nextServer = currentServer == null
        ? null
        : VpnServerStateModel(
            configured: currentServer.configured,
            server: currentServer.server,
            cidr: currentServer.cidr,
            serverAddress: currentServer.serverAddress,
            peersTotal: nextPeers.length,
            peersConnected: connectedCount,
            bytesRx: totalRx,
            bytesTx: totalTx,
            persistentKeepalive: currentServer.persistentKeepalive,
            profiles: currentServer.profiles,
          );

    state = state.copyWith(
      peers: List<VpnPeer>.unmodifiable(nextPeers),
      serverState: nextServer,
    );
  }

  /// Limpa imediatamente a chave privada efêmera do cofre ao fechar o visualizador.
  void clearLastArtifact() {
    if (state.lastArtifact != null) {
      state = state.copyWith(clearLastArtifact: true);
    }
  }

  Future<void> fetchServer({bool includeAuxiliary = false}) async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final serverState = await _repo.fetchServer();
      List<VpnServerAddressHint>? addresses;
      List<String>? existingCidrs;
      if (includeAuxiliary) {
        final aux = await Future.wait<dynamic>(<Future<dynamic>>[
          _repo.fetchServerAddresses(),
          _repo.fetchExistingNetworkCidrs(),
        ]);
        addresses = aux[0] as List<VpnServerAddressHint>;
        existingCidrs = aux[1] as List<String>;
      }
      state = state.copyWith(
        serverState: serverState,
        addresses: addresses,
        existingCidrs: existingCidrs,
        loading: false,
      );
    } catch (err) {
      state = state.copyWith(
        loading: false,
        error: _describe(err, 'Erro ao carregar o servidor VPN.'),
      );
    }
  }

  Future<bool> saveServer({
    required String? publicEndpoint,
    required int listenPort,
    required String cidr,
    required int mtu,
    required String? dnsServers,
    required bool allowPeerToPeer,
  }) async {
    state = state.copyWith(saving: true, clearError: true);
    try {
      await _repo.saveServer(
        publicEndpoint: publicEndpoint,
        listenPort: listenPort,
        cidr: cidr,
        mtu: mtu,
        dnsServers: dnsServers,
        allowPeerToPeer: allowPeerToPeer,
      );
      final refreshed = await _repo.fetchServer();
      state = state.copyWith(serverState: refreshed, saving: false);
      return true;
    } catch (err) {
      state = state.copyWith(
        saving: false,
        error: _describe(err, 'Erro ao salvar a configuração do servidor VPN.'),
      );
      return false;
    }
  }

  Future<VpnPreflightResult?> runPreflight({
    String? publicEndpoint,
    int? listenPort,
  }) async {
    state = state.copyWith(testing: true, clearError: true);
    try {
      final result = await _repo.runPreflight(
        publicEndpoint:
            publicEndpoint ?? state.serverState?.server?.publicEndpoint,
        listenPort:
            listenPort ?? state.serverState?.server?.listenPort ?? 51820,
      );
      state = state.copyWith(preflight: result, testing: false);
      return result;
    } catch (err) {
      state = state.copyWith(
        testing: false,
        error: _describe(err, 'Erro ao executar o teste de acessibilidade.'),
      );
      return null;
    }
  }

  Future<String?> detectEndpoint() async {
    state = state.copyWith(testing: true, clearError: true);
    try {
      final detected = await _repo.detectEndpoint();
      state = state.copyWith(testing: false);
      return detected;
    } catch (err) {
      state = state.copyWith(
        testing: false,
        error: _describe(err, 'Erro ao detectar o endereço público.'),
      );
      return null;
    }
  }

  Future<void> fetchPeers() async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final peers = await _repo.fetchPeers();
      state = state.copyWith(peers: peers, loading: false);
    } catch (err) {
      state = state.copyWith(
        loading: false,
        error: _describe(err, 'Erro ao carregar os dispositivos VPN.'),
      );
    }
  }

  Future<void> fetchSites() async {
    final sites = await _repo.fetchSites();
    state = state.copyWith(sites: sites);
  }

  Future<String?> suggestNextIp() async {
    try {
      return await _repo.suggestNextIp();
    } catch (err) {
      state = state.copyWith(
        error: _describe(err, 'Erro ao sugerir o próximo IP livre.'),
      );
      return null;
    }
  }

  Future<VpnArtifact?> createPeer(CreateVpnPeerPayload payload) async {
    state = state.copyWith(saving: true, clearError: true);
    try {
      final artifact = await _repo.createPeer(payload);
      final peers = await _repo.fetchPeers();
      state = state.copyWith(
        lastArtifact: artifact,
        peers: peers,
        saving: false,
      );
      return artifact;
    } catch (err) {
      state = state.copyWith(
        saving: false,
        error: _describe(err, 'Erro ao criar o dispositivo VPN.'),
      );
      return null;
    }
  }

  Future<bool> renamePeer(int peerId, String name) async {
    state = state.copyWith(saving: true, clearError: true);
    try {
      final updated = await _repo.renamePeer(peerId, name);
      final nextPeers = state.peers
          .map((peer) {
            if (peer.id != peerId) return peer;
            return peer.copyWith(device: updated.device ?? peer.device);
          })
          .toList(growable: false);
      state = state.copyWith(peers: nextPeers, saving: false);
      return true;
    } catch (err) {
      state = state.copyWith(
        saving: false,
        error: _describe(err, 'Erro ao renomear o dispositivo VPN.'),
      );
      return false;
    }
  }

  Future<VpnArtifact?> fetchConfig(int peerId) async {
    state = state.copyWith(clearError: true);
    try {
      final artifact = await _repo.fetchConfig(peerId);
      state = state.copyWith(lastArtifact: artifact);
      return artifact;
    } catch (err) {
      state = state.copyWith(
        error: _describe(err, 'Erro ao obter a configuração do dispositivo.'),
      );
      return null;
    }
  }

  Future<VpnArtifact?> rotateKeys(
    int peerId, {
    VpnInstallPolicy? agentPolicy,
  }) async {
    state = state.copyWith(saving: true, clearError: true);
    try {
      final artifact = await _repo.rotateKeys(peerId, agentPolicy: agentPolicy);
      final peers = await _repo.fetchPeers();
      state = state.copyWith(
        lastArtifact: artifact,
        peers: peers,
        saving: false,
      );
      return artifact;
    } catch (err) {
      state = state.copyWith(
        saving: false,
        error: _describe(err, 'Erro ao rotacionar as chaves.'),
      );
      return null;
    }
  }

  Future<bool> updatePeerAgentPolicy({
    required int peerId,
    required String enrollCode,
    required VpnInstallPolicy policy,
    required String confContent,
  }) async {
    state = state.copyWith(clearError: true);
    try {
      final updated = await _repo.updatePeerAgentPolicy(
        peerId: peerId,
        enrollCode: enrollCode,
        policy: policy,
        confContent: confContent,
      );
      final current = state.lastArtifact;
      if (current != null && current.peerId == peerId) {
        state = state.copyWith(
          lastArtifact: current.copyWith(
            agentInstall: updated.agentInstall,
            variants: updated.variants,
          ),
        );
      }
      return true;
    } catch (err) {
      state = state.copyWith(
        error: _describe(err, 'Erro ao atualizar as permissões do agente.'),
      );
      return false;
    }
  }

  Future<String?> fetchFirewallHints(int peerId) async {
    state = state.copyWith(clearError: true);
    try {
      return await _repo.fetchFirewallHints(peerId);
    } catch (err) {
      state = state.copyWith(
        error: _describe(err, 'Erro ao obter as regras de firewall.'),
      );
      return null;
    }
  }

  Future<bool> revokePeer(int peerId) async {
    state = state.copyWith(clearError: true);
    try {
      await _repo.revokePeer(peerId);
      final nextPeers = state.peers
          .where((peer) => peer.id != peerId)
          .toList(growable: false);
      state = state.copyWith(peers: nextPeers);
      return true;
    } catch (err) {
      state = state.copyWith(
        error: _describe(err, 'Erro ao revogar o dispositivo VPN.'),
      );
      return false;
    }
  }
}

final vpnProvider = NotifierProvider<VpnNotifier, VpnState>(VpnNotifier.new);
