// Barrel de compatibilidade para os utilitários da área `servers`.
// Cada responsabilidade vive em seu próprio módulo coeso (SRP — AGENTS.md §10).
export 'package:netmonitor_app/core/utils/network_address_utils.dart'
    show isPrivateHost, looksPrivateUrl;

export 'agent_presentation.dart';
export 'collection_helpers.dart';
export 'deploy_presentation.dart';
export 'docker_presentation.dart';
export 'env_file_parser.dart';
export 'registry_presentation.dart';
export 'status_visual.dart';
export 'storage_presentation.dart';
