import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';

export 'docker_history_models.dart';

/// Host Docker retornado por `GET /api/docker/hosts`.
/// Todo host é um agente (`HostKey = agent-<id>`, ADR 014).
@immutable
class DockerHostView {
  const DockerHostView({
    required this.key,
    required this.name,
    required this.kind,
    required this.online,
    required this.agentId,
    required this.dockerAvailable,
    required this.composeAvailable,
    required this.policy,
  });

  final String key;
  final String name;
  final String kind; // 'local' | 'agent'
  final bool online;
  final int agentId;
  final bool dockerAvailable;
  final bool composeAvailable;
  final List<String> policy;

  bool get isLocal => kind == 'local';

  DockerHostView copyWith({bool? online}) {
    return DockerHostView(
      key: key,
      name: name,
      kind: kind,
      online: online ?? this.online,
      agentId: agentId,
      dockerAvailable: dockerAvailable,
      composeAvailable: composeAvailable,
      policy: policy,
    );
  }

  factory DockerHostView.fromJson(Map<String, dynamic> json) {
    return DockerHostView(
      key: (json['key'] as String?) ?? '',
      name: (json['name'] as String?) ?? '',
      kind: (json['kind'] as String?) ?? 'agent',
      online: parseBool(json['online']),
      agentId: parseIntOrNull(json['agentId']) ?? 0,
      dockerAvailable: parseBool(json['dockerAvailable']),
      composeAvailable: parseBool(json['composeAvailable']),
      policy: List<String>.unmodifiable(asStringList(json['policy'])),
    );
  }
}

@immutable
class DockerStatusResponse {
  const DockerStatusResponse({
    required this.available,
    this.reason,
    this.engineVersion,
    this.apiVersion,
    this.name,
    this.operatingSystem,
    this.architecture,
    this.cpus = 0,
    this.memoryTotalBytes = 0,
    this.containers = 0,
    this.containersRunning = 0,
    this.containersStopped = 0,
    this.images = 0,
  });

  final bool available;
  final String? reason;
  final String? engineVersion;
  final String? apiVersion;
  final String? name;
  final String? operatingSystem;
  final String? architecture;
  final int cpus;
  final int memoryTotalBytes;
  final int containers;
  final int containersRunning;
  final int containersStopped;
  final int images;

  factory DockerStatusResponse.fromJson(Map<String, dynamic> json) {
    return DockerStatusResponse(
      available: parseBool(json['available']),
      reason: json['reason'] as String?,
      engineVersion: json['engineVersion'] as String?,
      apiVersion: json['apiVersion'] as String?,
      name: json['name'] as String?,
      operatingSystem: json['operatingSystem'] as String?,
      architecture: json['architecture'] as String?,
      cpus: parseIntOrNull(json['cpus']) ?? 0,
      memoryTotalBytes: parseIntOrNull(json['memoryTotalBytes']) ?? 0,
      containers: parseIntOrNull(json['containers']) ?? 0,
      containersRunning: parseIntOrNull(json['containersRunning']) ?? 0,
      containersStopped: parseIntOrNull(json['containersStopped']) ?? 0,
      images: parseIntOrNull(json['images']) ?? 0,
    );
  }
}

@immutable
class DockerPortBinding {
  const DockerPortBinding({
    this.ip,
    required this.privatePort,
    this.publicPort,
    required this.type,
  });

  final String? ip;
  final int privatePort;
  final int? publicPort;
  final String type;

  String get formatted {
    if (publicPort != null && publicPort! > 0) {
      final host = (ip != null && ip!.isNotEmpty) ? '$ip:' : '';
      return '$host$publicPort->$privatePort/$type';
    }
    return '$privatePort/$type';
  }

  factory DockerPortBinding.fromJson(Map<String, dynamic> json) {
    return DockerPortBinding(
      ip: json['ip'] as String?,
      privatePort: parseIntOrNull(json['privatePort']) ?? 0,
      publicPort: parseIntOrNull(json['publicPort']),
      type: (json['protocol'] as String?) ?? (json['type'] as String?) ?? 'tcp',
    );
  }
}

@immutable
class DockerContainerSummary {
  const DockerContainerSummary({
    required this.id,
    required this.names,
    required this.image,
    required this.imageId,
    required this.state,
    required this.status,
    required this.labels,
    required this.ports,
    required this.created,
    this.projectName,
    required this.protected,
  });

  final String id;
  final List<String> names;
  final String image;
  final String imageId;
  final String state;
  final String status;
  final Map<String, String> labels;
  final List<DockerPortBinding> ports;
  final int created;
  final String? projectName;
  final bool protected;

  /// Regra de segurança ADR 014: também confere a label diretamente como defesa em profundidade.
  bool get isProtected =>
      protected || labels['com.netmonitor.protected']?.toLowerCase() == 'true';

  factory DockerContainerSummary.fromJson(Map<String, dynamic> json) {
    return DockerContainerSummary(
      id: (json['id'] as String?) ?? '',
      names: List<String>.unmodifiable(asStringList(json['names'])),
      image: (json['image'] as String?) ?? '',
      imageId: (json['imageId'] as String?) ?? '',
      state: (json['state'] as String?) ?? 'unknown',
      status: (json['status'] as String?) ?? '',
      labels: asStringMap(json['labels']),
      ports: List<DockerPortBinding>.unmodifiable(
        asJsonList(json['ports'], DockerPortBinding.fromJson),
      ),
      created: parseIntOrNull(json['created']) ?? 0,
      projectName: json['projectName'] as String?,
      protected: parseBool(json['protected']),
    );
  }
}

@immutable
class DockerCpuMetrics {
  const DockerCpuMetrics({required this.usagePercent});
  final double usagePercent;

  factory DockerCpuMetrics.fromJson(Map<String, dynamic> json) {
    return DockerCpuMetrics(
      usagePercent: parseDoubleOrNull(json['usagePercent']) ?? 0,
    );
  }
}

@immutable
class DockerMemoryMetrics {
  const DockerMemoryMetrics({
    required this.usageBytes,
    required this.limitBytes,
    required this.usagePercent,
  });

  final int usageBytes;
  final int limitBytes;
  final double usagePercent;

  factory DockerMemoryMetrics.fromJson(Map<String, dynamic> json) {
    return DockerMemoryMetrics(
      usageBytes: parseIntOrNull(json['usageBytes']) ?? 0,
      limitBytes: parseIntOrNull(json['limitBytes']) ?? 0,
      usagePercent: parseDoubleOrNull(json['usagePercent']) ?? 0,
    );
  }
}

@immutable
class DockerNetworkMetrics {
  const DockerNetworkMetrics({
    required this.receivedBytes,
    required this.transmittedBytes,
  });

  final int receivedBytes;
  final int transmittedBytes;

  factory DockerNetworkMetrics.fromJson(Map<String, dynamic> json) {
    return DockerNetworkMetrics(
      receivedBytes: parseIntOrNull(json['receivedBytes']) ?? 0,
      transmittedBytes: parseIntOrNull(json['transmittedBytes']) ?? 0,
    );
  }
}

@immutable
class DockerIoMetrics {
  const DockerIoMetrics({required this.readBytes, required this.writeBytes});

  final int readBytes;
  final int writeBytes;

  factory DockerIoMetrics.fromJson(Map<String, dynamic> json) {
    return DockerIoMetrics(
      readBytes: parseIntOrNull(json['readBytes']) ?? 0,
      writeBytes: parseIntOrNull(json['writeBytes']) ?? 0,
    );
  }
}

@immutable
class DockerContainerMetrics {
  const DockerContainerMetrics({
    required this.containerId,
    required this.containerName,
    this.projectName,
    required this.imageName,
    required this.status,
    required this.cpu,
    required this.memory,
    required this.network,
    required this.blockIo,
    this.pids,
  });

  final String containerId;
  final String containerName;
  final String? projectName;
  final String imageName;
  final String status;
  final DockerCpuMetrics cpu;
  final DockerMemoryMetrics memory;
  final DockerNetworkMetrics network;
  final DockerIoMetrics blockIo;
  final int? pids;

  factory DockerContainerMetrics.fromJson(Map<String, dynamic> json) {
    return DockerContainerMetrics(
      containerId: (json['containerId'] as String?) ?? '',
      containerName: (json['containerName'] as String?) ?? '',
      projectName: json['projectName'] as String?,
      imageName: (json['imageName'] as String?) ?? '',
      status: (json['status'] as String?) ?? '',
      cpu: json['cpu'] is Map
          ? DockerCpuMetrics.fromJson(asJsonMap(json['cpu']))
          : const DockerCpuMetrics(usagePercent: 0),
      memory: json['memory'] is Map
          ? DockerMemoryMetrics.fromJson(asJsonMap(json['memory']))
          : const DockerMemoryMetrics(
              usageBytes: 0,
              limitBytes: 0,
              usagePercent: 0,
            ),
      network: json['network'] is Map
          ? DockerNetworkMetrics.fromJson(asJsonMap(json['network']))
          : const DockerNetworkMetrics(receivedBytes: 0, transmittedBytes: 0),
      blockIo: json['blockIo'] is Map
          ? DockerIoMetrics.fromJson(asJsonMap(json['blockIo']))
          : const DockerIoMetrics(readBytes: 0, writeBytes: 0),
      pids: parseIntOrNull(json['pids']),
    );
  }
}

@immutable
class DockerMetricsResponse {
  const DockerMetricsResponse({
    required this.dockerAvailable,
    this.unavailableReason,
    this.failedContainerCount = 0,
    required this.collectedAt,
    required this.containers,
  });

  final bool dockerAvailable;
  final String? unavailableReason;
  final int failedContainerCount;
  final String collectedAt;
  final List<DockerContainerMetrics> containers;

  factory DockerMetricsResponse.fromJson(Map<String, dynamic> json) {
    return DockerMetricsResponse(
      dockerAvailable: parseBool(json['dockerAvailable']),
      unavailableReason: json['unavailableReason'] as String?,
      failedContainerCount: parseIntOrNull(json['failedContainerCount']) ?? 0,
      collectedAt: (json['collectedAt'] as String?) ?? '',
      containers: List<DockerContainerMetrics>.unmodifiable(
        asJsonList(json['containers'], DockerContainerMetrics.fromJson),
      ),
    );
  }
}

@immutable
class DockerMount {
  const DockerMount({
    required this.mountType,
    this.name,
    required this.source,
    required this.destination,
    required this.rw,
  });

  final String mountType;
  final String? name;
  final String source;
  final String destination;
  final bool rw;

  factory DockerMount.fromJson(Map<String, dynamic> json) {
    return DockerMount(
      mountType:
          (json['mountType'] as String?) ?? (json['type'] as String?) ?? '',
      name: json['name'] as String?,
      source: (json['source'] as String?) ?? '',
      destination: (json['destination'] as String?) ?? '',
      rw: parseBool(json['readWrite'] ?? json['rw'], fallback: true),
    );
  }
}

@immutable
class DockerNetworkEndpoint {
  const DockerNetworkEndpoint({
    required this.networkName,
    this.networkId,
    this.ipAddress,
    this.gateway,
    this.macAddress,
  });

  final String networkName;
  final String? networkId;
  final String? ipAddress;
  final String? gateway;
  final String? macAddress;

  factory DockerNetworkEndpoint.fromJson(Map<String, dynamic> json) {
    return DockerNetworkEndpoint(
      networkName: (json['networkName'] as String?) ?? '',
      networkId: json['networkId'] as String?,
      ipAddress: json['ipAddress'] as String?,
      gateway: json['gateway'] as String?,
      macAddress: json['macAddress'] as String?,
    );
  }
}

@immutable
class DockerContainerDetail {
  const DockerContainerDetail({
    required this.id,
    required this.name,
    required this.image,
    required this.imageId,
    required this.created,
    required this.status,
    required this.running,
    this.pid,
    this.hostname,
    this.user,
    this.restartPolicy,
    this.networkMode,
    required this.command,
    required this.environment,
    required this.labels,
    required this.mounts,
    required this.networks,
  });

  final String id;
  final String name;
  final String image;
  final String imageId;
  final String created;
  final String status;
  final bool running;
  final int? pid;
  final String? hostname;
  final String? user;
  final String? restartPolicy;
  final String? networkMode;
  final List<String> command;
  final List<String> environment;
  final Map<String, String> labels;
  final List<DockerMount> mounts;
  final List<DockerNetworkEndpoint> networks;

  factory DockerContainerDetail.fromJson(Map<String, dynamic> json) {
    final state = asJsonMap(json['state']);
    final config = asJsonMap(json['config']);
    final hostConfig = asJsonMap(json['hostConfig']);
    final rawRestart = hostConfig['restartPolicy'];
    String? restartPolicy;
    if (rawRestart is String) {
      restartPolicy = rawRestart;
    } else if (rawRestart is Map) {
      final map = asJsonMap(rawRestart);
      final name = (map['name'] as String?)?.trim();
      final maxRetry = parseIntOrNull(map['maximumRetryCount']) ?? 0;
      if (name != null && name.isNotEmpty) {
        restartPolicy = maxRetry > 0 ? '$name (máx. $maxRetry)' : name;
      }
    }
    return DockerContainerDetail(
      id: (json['id'] as String?) ?? '',
      name: ((json['name'] as String?) ?? '').replaceFirst(RegExp(r'^/'), ''),
      image: (json['image'] as String?) ?? '',
      imageId: (json['imageId'] as String?) ?? '',
      created: (json['created'] as String?) ?? '',
      status: (state['status'] as String?) ?? '',
      running: parseBool(state['running']),
      pid: parseIntOrNull(state['pid']),
      hostname: config['hostname'] as String?,
      user: config['user'] as String?,
      restartPolicy: restartPolicy,
      networkMode: hostConfig['networkMode'] as String?,
      command: List<String>.unmodifiable(
        asStringList(config['cmd'] ?? config['command']),
      ),
      environment: List<String>.unmodifiable(
        asStringList(config['env'] ?? config['environment']),
      ),
      labels: asStringMap(config['labels']),
      mounts: List<DockerMount>.unmodifiable(
        asJsonList(json['mounts'], DockerMount.fromJson),
      ),
      networks: List<DockerNetworkEndpoint>.unmodifiable(
        asJsonList(json['networks'], DockerNetworkEndpoint.fromJson),
      ),
    );
  }
}

@immutable
class DockerLogEntry {
  const DockerLogEntry({
    this.timestamp,
    required this.stream,
    required this.message,
  });

  final String? timestamp;
  final String stream;
  final String message;

  String get formattedLine => (timestamp != null && timestamp!.isNotEmpty)
      ? '$timestamp $message'
      : message;

  factory DockerLogEntry.fromJson(Map<String, dynamic> json) {
    return DockerLogEntry(
      timestamp: json['timestamp'] as String?,
      stream: (json['stream'] as String?) ?? 'stdout',
      message: (json['message'] as String?) ?? '',
    );
  }
}

@immutable
class DockerVolumeSummary {
  const DockerVolumeSummary({
    required this.name,
    required this.driver,
    required this.mountpoint,
    required this.labels,
    required this.scope,
    this.createdAt,
  });

  final String name;
  final String driver;
  final String mountpoint;
  final Map<String, String> labels;
  final String scope;
  final String? createdAt;

  String? get composeProject {
    final val = labels['com.docker.compose.project']?.trim();
    return (val != null && val.isNotEmpty) ? val : null;
  }

  factory DockerVolumeSummary.fromJson(Map<String, dynamic> json) {
    return DockerVolumeSummary(
      name: (json['name'] as String?) ?? '',
      driver: (json['driver'] as String?) ?? 'local',
      mountpoint: (json['mountpoint'] as String?) ?? '',
      labels: asStringMap(json['labels']),
      scope: (json['scope'] as String?) ?? 'local',
      createdAt: json['createdAt'] as String?,
    );
  }
}

@immutable
class DockerVolumeDetail {
  const DockerVolumeDetail({
    required this.name,
    required this.driver,
    required this.mountpoint,
    required this.labels,
    required this.scope,
    this.createdAt,
    required this.options,
  });

  final String name;
  final String driver;
  final String mountpoint;
  final Map<String, String> labels;
  final String scope;
  final String? createdAt;
  final Map<String, String> options;

  factory DockerVolumeDetail.fromJson(Map<String, dynamic> json) {
    return DockerVolumeDetail(
      name: (json['name'] as String?) ?? '',
      driver: (json['driver'] as String?) ?? 'local',
      mountpoint: (json['mountpoint'] as String?) ?? '',
      labels: asStringMap(json['labels']),
      scope: (json['scope'] as String?) ?? 'local',
      createdAt: json['createdAt'] as String?,
      options: asStringMap(json['options']),
    );
  }
}

@immutable
class DockerIpamEntry {
  const DockerIpamEntry({this.subnet, this.gateway});
  final String? subnet;
  final String? gateway;

  factory DockerIpamEntry.fromJson(Map<String, dynamic> json) {
    return DockerIpamEntry(
      subnet: json['subnet'] as String?,
      gateway: json['gateway'] as String?,
    );
  }
}

@immutable
class DockerNetworkSummary {
  const DockerNetworkSummary({
    required this.id,
    required this.name,
    required this.driver,
    required this.scope,
    this.ipamDriver,
    required this.ipamConfig,
    required this.internal,
    required this.connectedContainers,
    required this.labels,
    this.created,
  });

  final String id;
  final String name;
  final String driver;
  final String scope;
  final String? ipamDriver;
  final List<DockerIpamEntry> ipamConfig;
  final bool internal;
  final int connectedContainers;
  final Map<String, String> labels;
  final String? created;

  bool get isBuiltIn => name == 'bridge' || name == 'host' || name == 'none';

  factory DockerNetworkSummary.fromJson(Map<String, dynamic> json) {
    return DockerNetworkSummary(
      id: (json['id'] as String?) ?? '',
      name: (json['name'] as String?) ?? '',
      driver: (json['driver'] as String?) ?? '',
      scope: (json['scope'] as String?) ?? '',
      ipamDriver: json['ipamDriver'] as String?,
      ipamConfig: List<DockerIpamEntry>.unmodifiable(
        asJsonList(json['ipamConfig'], DockerIpamEntry.fromJson),
      ),
      internal: parseBool(json['internal']),
      connectedContainers: parseIntOrNull(json['connectedContainers']) ?? 0,
      labels: asStringMap(json['labels']),
      created: json['created'] as String?,
    );
  }
}

@immutable
class DockerNetworkContainerRef {
  const DockerNetworkContainerRef({
    required this.containerId,
    required this.name,
    this.macAddress,
    this.ipv4Address,
    this.ipv6Address,
  });

  final String containerId;
  final String name;
  final String? macAddress;
  final String? ipv4Address;
  final String? ipv6Address;

  factory DockerNetworkContainerRef.fromJson(Map<String, dynamic> json) {
    return DockerNetworkContainerRef(
      containerId: (json['containerId'] as String?) ?? '',
      name: (json['name'] as String?) ?? '',
      macAddress: json['macAddress'] as String?,
      ipv4Address: json['ipv4Address'] as String?,
      ipv6Address: json['ipv6Address'] as String?,
    );
  }
}

@immutable
class DockerNetworkDetail {
  const DockerNetworkDetail({
    required this.summary,
    required this.containers,
    required this.options,
  });

  final DockerNetworkSummary summary;
  final List<DockerNetworkContainerRef> containers;
  final Map<String, String> options;

  factory DockerNetworkDetail.fromJson(Map<String, dynamic> json) {
    return DockerNetworkDetail(
      summary: DockerNetworkSummary.fromJson(json),
      containers: List<DockerNetworkContainerRef>.unmodifiable(
        asJsonList(json['containers'], DockerNetworkContainerRef.fromJson),
      ),
      options: asStringMap(json['options']),
    );
  }
}

@immutable
class DockerImageSummary {
  const DockerImageSummary({
    required this.id,
    this.parentId,
    required this.repoTags,
    required this.repoDigests,
    required this.created,
    required this.size,
    this.sharedSize = 0,
    required this.labels,
    this.containers = 0,
  });

  final String id;
  final String? parentId;
  final List<String> repoTags;
  final List<String> repoDigests;
  final int created;
  final int size;
  final int sharedSize;
  final Map<String, String> labels;
  final int containers;

  String get primaryTag => repoTags.isNotEmpty ? repoTags.first : '<sem tag>';

  factory DockerImageSummary.fromJson(Map<String, dynamic> json) {
    return DockerImageSummary(
      id: (json['id'] as String?) ?? '',
      parentId: json['parentId'] as String?,
      repoTags: List<String>.unmodifiable(asStringList(json['repoTags'])),
      repoDigests: List<String>.unmodifiable(asStringList(json['repoDigests'])),
      created: parseIntOrNull(json['created']) ?? 0,
      size: parseIntOrNull(json['size']) ?? 0,
      sharedSize: parseIntOrNull(json['sharedSize']) ?? 0,
      labels: asStringMap(json['labels']),
      containers: parseIntOrNull(json['containers']) ?? 0,
    );
  }
}

@immutable
class DockerImageDetail {
  const DockerImageDetail({
    required this.id,
    required this.repoTags,
    required this.created,
    required this.size,
    required this.environment,
    required this.command,
    required this.entrypoint,
    required this.labels,
    this.workingDir,
    this.user,
    this.rootFsType,
    required this.layers,
  });

  final String id;
  final List<String> repoTags;
  final String created;
  final int size;
  final List<String> environment;
  final List<String> command;
  final List<String> entrypoint;
  final Map<String, String> labels;
  final String? workingDir;
  final String? user;
  final String? rootFsType;
  final List<String> layers;

  factory DockerImageDetail.fromJson(Map<String, dynamic> json) {
    return DockerImageDetail(
      id: (json['id'] as String?) ?? '',
      repoTags: List<String>.unmodifiable(asStringList(json['repoTags'])),
      created: (json['created'] ?? '').toString(),
      size: parseIntOrNull(json['size']) ?? 0,
      environment: List<String>.unmodifiable(asStringList(json['environment'])),
      command: List<String>.unmodifiable(asStringList(json['command'])),
      entrypoint: List<String>.unmodifiable(asStringList(json['entrypoint'])),
      labels: asStringMap(json['labels']),
      workingDir: json['workingDir'] as String?,
      user: json['user'] as String?,
      rootFsType: json['rootFsType'] as String?,
      layers: List<String>.unmodifiable(asStringList(json['layers'])),
    );
  }
}

@immutable
class ComposeService {
  const ComposeService({
    required this.name,
    required this.containers,
    required this.running,
  });

  final String name;
  final int containers;
  final int running;

  factory ComposeService.fromJson(Map<String, dynamic> json) {
    return ComposeService(
      name: (json['name'] as String?) ?? '',
      containers: parseIntOrNull(json['containers']) ?? 0,
      running: parseIntOrNull(json['running']) ?? 0,
    );
  }
}

@immutable
class ComposeProject {
  const ComposeProject({
    required this.name,
    this.workingDir,
    required this.configFiles,
    required this.services,
    required this.containers,
    required this.running,
    required this.protected,
  });

  final String name;
  final String? workingDir;
  final List<String> configFiles;
  final List<ComposeService> services;
  final int containers;
  final int running;
  final bool protected;

  factory ComposeProject.fromJson(Map<String, dynamic> json) {
    return ComposeProject(
      name: (json['name'] as String?) ?? '',
      workingDir: json['workingDir'] as String?,
      configFiles: List<String>.unmodifiable(asStringList(json['configFiles'])),
      services: List<ComposeService>.unmodifiable(
        asJsonList(json['services'], ComposeService.fromJson),
      ),
      containers: parseIntOrNull(json['containers']) ?? 0,
      running: parseIntOrNull(json['running']) ?? 0,
      protected: parseBool(json['protected']),
    );
  }
}

@immutable
class DockerOperationState {
  const DockerOperationState({
    required this.operationId,
    required this.hostKey,
    required this.kind,
    required this.target,
    required this.state,
    required this.lines,
    this.message,
    required this.updatedAt,
  });

  final String operationId;
  final String hostKey;
  final String kind;
  final String target;
  final String state; // 'running' | 'succeeded' | 'failed'
  final List<String> lines;
  final String? message;
  final String updatedAt;

  bool get isRunning => state == 'running';
}

@immutable
class DockerLogStreamState {
  const DockerLogStreamState({
    required this.streamId,
    required this.hostKey,
    required this.containerId,
    required this.entries,
    this.ended,
  });

  final String streamId;
  final String hostKey;
  final String containerId;
  final List<DockerLogEntry> entries;
  final String? ended;

  DockerLogStreamState copyWith({
    List<DockerLogEntry>? entries,
    String? ended,
  }) {
    return DockerLogStreamState(
      streamId: streamId,
      hostKey: hostKey,
      containerId: containerId,
      entries: entries ?? this.entries,
      ended: ended ?? this.ended,
    );
  }
}
