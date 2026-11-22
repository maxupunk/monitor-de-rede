import 'package:flutter/foundation.dart';

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
    final rawPolicy = json['policy'];
    return DockerHostView(
      key: (json['key'] as String?) ?? '',
      name: (json['name'] as String?) ?? '',
      kind: (json['kind'] as String?) ?? 'agent',
      online: json['online'] == true,
      agentId: (json['agentId'] as num?)?.toInt() ?? 0,
      dockerAvailable: json['dockerAvailable'] == true,
      composeAvailable: json['composeAvailable'] == true,
      policy: rawPolicy is List
          ? List<String>.unmodifiable(rawPolicy.whereType<String>())
          : const <String>[],
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
      available: json['available'] == true,
      reason: json['reason'] as String?,
      engineVersion: json['engineVersion'] as String?,
      apiVersion: json['apiVersion'] as String?,
      name: json['name'] as String?,
      operatingSystem: json['operatingSystem'] as String?,
      architecture: json['architecture'] as String?,
      cpus: (json['cpus'] as num?)?.toInt() ?? 0,
      memoryTotalBytes: (json['memoryTotalBytes'] as num?)?.toInt() ?? 0,
      containers: (json['containers'] as num?)?.toInt() ?? 0,
      containersRunning: (json['containersRunning'] as num?)?.toInt() ?? 0,
      containersStopped: (json['containersStopped'] as num?)?.toInt() ?? 0,
      images: (json['images'] as num?)?.toInt() ?? 0,
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
      privatePort: (json['privatePort'] as num?)?.toInt() ?? 0,
      publicPort: (json['publicPort'] as num?)?.toInt(),
      type: (json['type'] as String?) ?? 'tcp',
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
    final rawNames = json['names'];
    final rawLabels = json['labels'];
    final rawPorts = json['ports'];
    final labelsMap = <String, String>{};
    if (rawLabels is Map) {
      rawLabels.forEach((k, v) {
        if (k != null && v != null) labelsMap[k.toString()] = v.toString();
      });
    }
    return DockerContainerSummary(
      id: (json['id'] as String?) ?? '',
      names: rawNames is List
          ? List<String>.unmodifiable(rawNames.whereType<String>())
          : const <String>[],
      image: (json['image'] as String?) ?? '',
      imageId: (json['imageId'] as String?) ?? '',
      state: (json['state'] as String?) ?? 'unknown',
      status: (json['status'] as String?) ?? '',
      labels: Map<String, String>.unmodifiable(labelsMap),
      ports: rawPorts is List
          ? List<DockerPortBinding>.unmodifiable(
              rawPorts.whereType<Map>().map(
                (e) => DockerPortBinding.fromJson(Map<String, dynamic>.from(e)),
              ),
            )
          : const <DockerPortBinding>[],
      created: (json['created'] as num?)?.toInt() ?? 0,
      projectName: json['projectName'] as String?,
      protected: json['protected'] == true,
    );
  }
}

@immutable
class DockerCpuMetrics {
  const DockerCpuMetrics({required this.usagePercent});
  final double usagePercent;

  factory DockerCpuMetrics.fromJson(Map<String, dynamic> json) {
    return DockerCpuMetrics(
      usagePercent: (json['usagePercent'] as num?)?.toDouble() ?? 0,
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
      usageBytes: (json['usageBytes'] as num?)?.toInt() ?? 0,
      limitBytes: (json['limitBytes'] as num?)?.toInt() ?? 0,
      usagePercent: (json['usagePercent'] as num?)?.toDouble() ?? 0,
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
      receivedBytes: (json['receivedBytes'] as num?)?.toInt() ?? 0,
      transmittedBytes: (json['transmittedBytes'] as num?)?.toInt() ?? 0,
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
      readBytes: (json['readBytes'] as num?)?.toInt() ?? 0,
      writeBytes: (json['writeBytes'] as num?)?.toInt() ?? 0,
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
          ? DockerCpuMetrics.fromJson(
              Map<String, dynamic>.from(json['cpu'] as Map),
            )
          : const DockerCpuMetrics(usagePercent: 0),
      memory: json['memory'] is Map
          ? DockerMemoryMetrics.fromJson(
              Map<String, dynamic>.from(json['memory'] as Map),
            )
          : const DockerMemoryMetrics(
              usageBytes: 0,
              limitBytes: 0,
              usagePercent: 0,
            ),
      network: json['network'] is Map
          ? DockerNetworkMetrics.fromJson(
              Map<String, dynamic>.from(json['network'] as Map),
            )
          : const DockerNetworkMetrics(receivedBytes: 0, transmittedBytes: 0),
      blockIo: json['blockIo'] is Map
          ? DockerIoMetrics.fromJson(
              Map<String, dynamic>.from(json['blockIo'] as Map),
            )
          : const DockerIoMetrics(readBytes: 0, writeBytes: 0),
      pids: (json['pids'] as num?)?.toInt(),
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
    final rawList = json['containers'];
    return DockerMetricsResponse(
      dockerAvailable: json['dockerAvailable'] == true,
      unavailableReason: json['unavailableReason'] as String?,
      failedContainerCount:
          (json['failedContainerCount'] as num?)?.toInt() ?? 0,
      collectedAt: (json['collectedAt'] as String?) ?? '',
      containers: rawList is List
          ? List<DockerContainerMetrics>.unmodifiable(
              rawList.whereType<Map>().map(
                (e) => DockerContainerMetrics.fromJson(
                  Map<String, dynamic>.from(e),
                ),
              ),
            )
          : const <DockerContainerMetrics>[],
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
      rw: json['rw'] != false,
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
    final state = json['state'] is Map
        ? Map<String, dynamic>.from(json['state'] as Map)
        : const <String, dynamic>{};
    final config = json['config'] is Map
        ? Map<String, dynamic>.from(json['config'] as Map)
        : const <String, dynamic>{};
    final hostConfig = json['hostConfig'] is Map
        ? Map<String, dynamic>.from(json['hostConfig'] as Map)
        : const <String, dynamic>{};
    final rawCmd = config['cmd'] ?? config['command'];
    final rawEnv = config['env'] ?? config['environment'];
    final rawLabels = config['labels'];
    final labelsMap = <String, String>{};
    if (rawLabels is Map) {
      rawLabels.forEach((k, v) {
        if (k != null && v != null) labelsMap[k.toString()] = v.toString();
      });
    }
    final rawMounts = json['mounts'];
    final rawNetworks = json['networks'];
    return DockerContainerDetail(
      id: (json['id'] as String?) ?? '',
      name: ((json['name'] as String?) ?? '').replaceFirst(RegExp(r'^/'), ''),
      image: (json['image'] as String?) ?? '',
      imageId: (json['imageId'] as String?) ?? '',
      created: (json['created'] as String?) ?? '',
      status: (state['status'] as String?) ?? '',
      running: state['running'] == true,
      pid: (state['pid'] as num?)?.toInt(),
      hostname: config['hostname'] as String?,
      user: config['user'] as String?,
      restartPolicy: hostConfig['restartPolicy'] as String?,
      networkMode: hostConfig['networkMode'] as String?,
      command: rawCmd is List
          ? List<String>.unmodifiable(rawCmd.whereType<String>())
          : const <String>[],
      environment: rawEnv is List
          ? List<String>.unmodifiable(rawEnv.whereType<String>())
          : const <String>[],
      labels: Map<String, String>.unmodifiable(labelsMap),
      mounts: rawMounts is List
          ? List<DockerMount>.unmodifiable(
              rawMounts.whereType<Map>().map(
                (e) => DockerMount.fromJson(Map<String, dynamic>.from(e)),
              ),
            )
          : const <DockerMount>[],
      networks: rawNetworks is List
          ? List<DockerNetworkEndpoint>.unmodifiable(
              rawNetworks.whereType<Map>().map(
                (e) => DockerNetworkEndpoint.fromJson(
                  Map<String, dynamic>.from(e),
                ),
              ),
            )
          : const <DockerNetworkEndpoint>[],
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
    final rawLabels = json['labels'];
    final labelsMap = <String, String>{};
    if (rawLabels is Map) {
      rawLabels.forEach((k, v) {
        if (k != null && v != null) labelsMap[k.toString()] = v.toString();
      });
    }
    return DockerVolumeSummary(
      name: (json['name'] as String?) ?? '',
      driver: (json['driver'] as String?) ?? 'local',
      mountpoint: (json['mountpoint'] as String?) ?? '',
      labels: Map<String, String>.unmodifiable(labelsMap),
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
    Map<String, String> parseMap(Object? raw) {
      final result = <String, String>{};
      if (raw is Map) {
        raw.forEach((k, v) {
          if (k != null && v != null) result[k.toString()] = v.toString();
        });
      }
      return Map<String, String>.unmodifiable(result);
    }

    return DockerVolumeDetail(
      name: (json['name'] as String?) ?? '',
      driver: (json['driver'] as String?) ?? 'local',
      mountpoint: (json['mountpoint'] as String?) ?? '',
      labels: parseMap(json['labels']),
      scope: (json['scope'] as String?) ?? 'local',
      createdAt: json['createdAt'] as String?,
      options: parseMap(json['options']),
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
    final rawIpam = json['ipamConfig'];
    final rawLabels = json['labels'];
    final labelsMap = <String, String>{};
    if (rawLabels is Map) {
      rawLabels.forEach((k, v) {
        if (k != null && v != null) labelsMap[k.toString()] = v.toString();
      });
    }
    return DockerNetworkSummary(
      id: (json['id'] as String?) ?? '',
      name: (json['name'] as String?) ?? '',
      driver: (json['driver'] as String?) ?? '',
      scope: (json['scope'] as String?) ?? '',
      ipamDriver: json['ipamDriver'] as String?,
      ipamConfig: rawIpam is List
          ? List<DockerIpamEntry>.unmodifiable(
              rawIpam.whereType<Map>().map(
                (e) => DockerIpamEntry.fromJson(Map<String, dynamic>.from(e)),
              ),
            )
          : const <DockerIpamEntry>[],
      internal: json['internal'] == true,
      connectedContainers: (json['connectedContainers'] as num?)?.toInt() ?? 0,
      labels: Map<String, String>.unmodifiable(labelsMap),
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
    final rawContainers = json['containers'];
    final rawOptions = json['options'];
    final opts = <String, String>{};
    if (rawOptions is Map) {
      rawOptions.forEach((k, v) {
        if (k != null && v != null) opts[k.toString()] = v.toString();
      });
    }
    return DockerNetworkDetail(
      summary: DockerNetworkSummary.fromJson(json),
      containers: rawContainers is List
          ? List<DockerNetworkContainerRef>.unmodifiable(
              rawContainers.whereType<Map>().map(
                (e) => DockerNetworkContainerRef.fromJson(
                  Map<String, dynamic>.from(e),
                ),
              ),
            )
          : const <DockerNetworkContainerRef>[],
      options: Map<String, String>.unmodifiable(opts),
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
    final rawTags = json['repoTags'];
    final rawDigests = json['repoDigests'];
    final rawLabels = json['labels'];
    final labelsMap = <String, String>{};
    if (rawLabels is Map) {
      rawLabels.forEach((k, v) {
        if (k != null && v != null) labelsMap[k.toString()] = v.toString();
      });
    }
    return DockerImageSummary(
      id: (json['id'] as String?) ?? '',
      parentId: json['parentId'] as String?,
      repoTags: rawTags is List
          ? List<String>.unmodifiable(rawTags.whereType<String>())
          : const <String>[],
      repoDigests: rawDigests is List
          ? List<String>.unmodifiable(rawDigests.whereType<String>())
          : const <String>[],
      created: (json['created'] as num?)?.toInt() ?? 0,
      size: (json['size'] as num?)?.toInt() ?? 0,
      sharedSize: (json['sharedSize'] as num?)?.toInt() ?? 0,
      labels: Map<String, String>.unmodifiable(labelsMap),
      containers: (json['containers'] as num?)?.toInt() ?? 0,
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
    List<String> strList(Object? raw) => raw is List
        ? List<String>.unmodifiable(raw.whereType<String>())
        : const <String>[];
    final labelsMap = <String, String>{};
    if (json['labels'] is Map) {
      (json['labels'] as Map).forEach((k, v) {
        if (k != null && v != null) labelsMap[k.toString()] = v.toString();
      });
    }
    return DockerImageDetail(
      id: (json['id'] as String?) ?? '',
      repoTags: strList(json['repoTags']),
      created: (json['created'] ?? '').toString(),
      size: (json['size'] as num?)?.toInt() ?? 0,
      environment: strList(json['environment']),
      command: strList(json['command']),
      entrypoint: strList(json['entrypoint']),
      labels: Map<String, String>.unmodifiable(labelsMap),
      workingDir: json['workingDir'] as String?,
      user: json['user'] as String?,
      rootFsType: json['rootFsType'] as String?,
      layers: strList(json['layers']),
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
      containers: (json['containers'] as num?)?.toInt() ?? 0,
      running: (json['running'] as num?)?.toInt() ?? 0,
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
    final rawFiles = json['configFiles'];
    final rawServices = json['services'];
    return ComposeProject(
      name: (json['name'] as String?) ?? '',
      workingDir: json['workingDir'] as String?,
      configFiles: rawFiles is List
          ? List<String>.unmodifiable(rawFiles.whereType<String>())
          : const <String>[],
      services: rawServices is List
          ? List<ComposeService>.unmodifiable(
              rawServices.whereType<Map>().map(
                (e) => ComposeService.fromJson(Map<String, dynamic>.from(e)),
              ),
            )
          : const <ComposeService>[],
      containers: (json['containers'] as num?)?.toInt() ?? 0,
      running: (json['running'] as num?)?.toInt() ?? 0,
      protected: json['protected'] == true,
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

@immutable
class DockerAggregateSample {
  const DockerAggregateSample({
    required this.recordedAt,
    required this.cpuPercent,
    required this.memoryPercent,
    required this.memoryUsageBytes,
    required this.networkReceivedBytes,
    required this.networkTransmittedBytes,
    required this.cpuByContainer,
    required this.memoryByContainer,
    required this.containerNames,
  });

  final String recordedAt;
  final double cpuPercent;
  final double memoryPercent;
  final int memoryUsageBytes;
  final int networkReceivedBytes;
  final int networkTransmittedBytes;
  final Map<String, double> cpuByContainer;
  final Map<String, int> memoryByContainer;
  final Map<String, String> containerNames;
}

@immutable
class HostDiskPoint {
  const HostDiskPoint({
    required this.mount,
    required this.usedBytes,
    required this.totalBytes,
  });

  final String mount;
  final int usedBytes;
  final int totalBytes;

  double get usedPercent => totalBytes > 0 ? (usedBytes / totalBytes) * 100 : 0;

  factory HostDiskPoint.fromJson(Map<String, dynamic> json) {
    return HostDiskPoint(
      mount: (json['mount'] as String?) ?? '/',
      usedBytes: (json['usedBytes'] as num?)?.toInt() ?? 0,
      totalBytes: (json['totalBytes'] as num?)?.toInt() ?? 0,
    );
  }
}

@immutable
class HostHistoryPoint {
  const HostHistoryPoint({
    required this.at,
    required this.cpuAvg,
    required this.cpuMax,
    required this.memoryUsed,
    required this.memoryTotal,
    required this.load1,
    required this.netRxBps,
    required this.netTxBps,
    required this.disks,
  });

  final String at;
  final double cpuAvg;
  final double cpuMax;
  final int memoryUsed;
  final int memoryTotal;
  final double load1;
  final double netRxBps;
  final double netTxBps;
  final List<HostDiskPoint> disks;

  factory HostHistoryPoint.fromJson(Map<String, dynamic> json) {
    final rawDisks = json['disks'];
    return HostHistoryPoint(
      at: (json['at'] as String?) ?? '',
      cpuAvg: (json['cpuAvg'] as num?)?.toDouble() ?? 0,
      cpuMax: (json['cpuMax'] as num?)?.toDouble() ?? 0,
      memoryUsed: (json['memoryUsed'] as num?)?.toInt() ?? 0,
      memoryTotal: (json['memoryTotal'] as num?)?.toInt() ?? 0,
      load1: (json['load1'] as num?)?.toDouble() ?? 0,
      netRxBps: (json['netRxBps'] as num?)?.toDouble() ?? 0,
      netTxBps: (json['netTxBps'] as num?)?.toDouble() ?? 0,
      disks: rawDisks is List
          ? List<HostDiskPoint>.unmodifiable(
              rawDisks.whereType<Map>().map(
                (e) => HostDiskPoint.fromJson(Map<String, dynamic>.from(e)),
              ),
            )
          : const <HostDiskPoint>[],
    );
  }
}

@immutable
class ContainerUsageSummary {
  const ContainerUsageSummary({
    required this.name,
    this.project,
    required this.cpuAvg,
    required this.cpuMax,
    required this.memoryMax,
    required this.netRxBytes,
    required this.netTxBytes,
  });

  final String name;
  final String? project;
  final double cpuAvg;
  final double cpuMax;
  final int memoryMax;
  final int netRxBytes;
  final int netTxBytes;

  factory ContainerUsageSummary.fromJson(Map<String, dynamic> json) {
    return ContainerUsageSummary(
      name: (json['name'] as String?) ?? '',
      project: json['project'] as String?,
      cpuAvg: (json['cpuAvg'] as num?)?.toDouble() ?? 0,
      cpuMax: (json['cpuMax'] as num?)?.toDouble() ?? 0,
      memoryMax: (json['memoryMax'] as num?)?.toInt() ?? 0,
      netRxBytes: (json['netRxBytes'] as num?)?.toInt() ?? 0,
      netTxBytes: (json['netTxBytes'] as num?)?.toInt() ?? 0,
    );
  }
}

@immutable
class HostHistoryResponse {
  const HostHistoryResponse({
    required this.hostKey,
    required this.range,
    required this.stepMinutes,
    required this.host,
    required this.containers,
  });

  final String hostKey;
  final String range;
  final int stepMinutes;
  final List<HostHistoryPoint> host;
  final List<ContainerUsageSummary> containers;

  factory HostHistoryResponse.fromJson(Map<String, dynamic> json) {
    final rawHost = json['host'];
    final rawContainers = json['containers'];
    return HostHistoryResponse(
      hostKey: (json['hostKey'] as String?) ?? '',
      range: (json['range'] as String?) ?? '24h',
      stepMinutes: (json['stepMinutes'] as num?)?.toInt() ?? 5,
      host: rawHost is List
          ? List<HostHistoryPoint>.unmodifiable(
              rawHost.whereType<Map>().map(
                (e) => HostHistoryPoint.fromJson(Map<String, dynamic>.from(e)),
              ),
            )
          : const <HostHistoryPoint>[],
      containers: rawContainers is List
          ? List<ContainerUsageSummary>.unmodifiable(
              rawContainers.whereType<Map>().map(
                (e) => ContainerUsageSummary.fromJson(
                  Map<String, dynamic>.from(e),
                ),
              ),
            )
          : const <ContainerUsageSummary>[],
    );
  }
}

@immutable
class ContainerHistoryPoint {
  const ContainerHistoryPoint({
    required this.at,
    required this.cpuAvg,
    required this.cpuMax,
    required this.memoryAvg,
    required this.memoryMax,
    required this.netRxBytes,
    required this.netTxBytes,
    required this.blockReadBytes,
    required this.blockWriteBytes,
  });

  final String at;
  final double cpuAvg;
  final double cpuMax;
  final int memoryAvg;
  final int memoryMax;
  final int netRxBytes;
  final int netTxBytes;
  final int blockReadBytes;
  final int blockWriteBytes;

  factory ContainerHistoryPoint.fromJson(Map<String, dynamic> json) {
    return ContainerHistoryPoint(
      at: (json['at'] as String?) ?? '',
      cpuAvg: (json['cpuAvg'] as num?)?.toDouble() ?? 0,
      cpuMax: (json['cpuMax'] as num?)?.toDouble() ?? 0,
      memoryAvg: (json['memoryAvg'] as num?)?.toInt() ?? 0,
      memoryMax: (json['memoryMax'] as num?)?.toInt() ?? 0,
      netRxBytes: (json['netRxBytes'] as num?)?.toInt() ?? 0,
      netTxBytes: (json['netTxBytes'] as num?)?.toInt() ?? 0,
      blockReadBytes: (json['blockReadBytes'] as num?)?.toInt() ?? 0,
      blockWriteBytes: (json['blockWriteBytes'] as num?)?.toInt() ?? 0,
    );
  }
}

@immutable
class ContainerHistoryResponse {
  const ContainerHistoryResponse({
    required this.hostKey,
    required this.containerName,
    required this.range,
    required this.stepMinutes,
    required this.points,
  });

  final String hostKey;
  final String containerName;
  final String range;
  final int stepMinutes;
  final List<ContainerHistoryPoint> points;

  factory ContainerHistoryResponse.fromJson(Map<String, dynamic> json) {
    final rawPoints = json['points'];
    return ContainerHistoryResponse(
      hostKey: (json['hostKey'] as String?) ?? '',
      containerName: (json['containerName'] as String?) ?? '',
      range: (json['range'] as String?) ?? '24h',
      stepMinutes: (json['stepMinutes'] as num?)?.toInt() ?? 5,
      points: rawPoints is List
          ? List<ContainerHistoryPoint>.unmodifiable(
              rawPoints.whereType<Map>().map(
                (e) => ContainerHistoryPoint.fromJson(
                  Map<String, dynamic>.from(e),
                ),
              ),
            )
          : const <ContainerHistoryPoint>[],
    );
  }
}

@immutable
class DockerPruneResponse {
  const DockerPruneResponse({
    required this.imagesDeleted,
    required this.spaceReclaimed,
  });

  final int imagesDeleted;
  final int spaceReclaimed;

  factory DockerPruneResponse.fromJson(Map<String, dynamic> json) {
    return DockerPruneResponse(
      imagesDeleted: (json['imagesDeleted'] as num?)?.toInt() ?? 0,
      spaceReclaimed: (json['spaceReclaimed'] as num?)?.toInt() ?? 0,
    );
  }
}
