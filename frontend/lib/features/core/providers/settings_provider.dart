import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/core/models/settings_models.dart';

@immutable
class CoreSettingsState {
  const CoreSettingsState({
    this.preferences = const SystemPreferences(),
    this.preferencesLoaded = false,
    this.preferencesLoading = false,
    this.preferencesSaving = false,
    this.addresses = const <ServerAddressEntry>[],
    this.preferredAddressId,
    this.domainHttps = false,
    this.addressesLoading = false,
    this.addressesSaving = false,
    this.snmpProfiles = const <SnmpDeviceProfile>[],
    this.snmpLoading = false,
    this.operatingSystems = const <OperatingSystemOption>[],
    this.osLoaded = false,
    this.error,
  });

  final SystemPreferences preferences;
  final bool preferencesLoaded;
  final bool preferencesLoading;
  final bool preferencesSaving;
  final List<ServerAddressEntry> addresses;
  final String? preferredAddressId;
  final bool domainHttps;
  final bool addressesLoading;
  final bool addressesSaving;
  final List<SnmpDeviceProfile> snmpProfiles;
  final bool snmpLoading;
  final List<OperatingSystemOption> operatingSystems;
  final bool osLoaded;
  final String? error;

  List<ServerAddressEntry> get usableAddresses => addresses
      .where((e) => e.value != null && e.value!.isNotEmpty)
      .toList(growable: false);

  CoreSettingsState copyWith({
    SystemPreferences? preferences,
    bool? preferencesLoaded,
    bool? preferencesLoading,
    bool? preferencesSaving,
    List<ServerAddressEntry>? addresses,
    String? preferredAddressId,
    bool clearPreferredAddress = false,
    bool? domainHttps,
    bool? addressesLoading,
    bool? addressesSaving,
    List<SnmpDeviceProfile>? snmpProfiles,
    bool? snmpLoading,
    List<OperatingSystemOption>? operatingSystems,
    bool? osLoaded,
    String? error,
    bool clearError = false,
  }) {
    return CoreSettingsState(
      preferences: preferences ?? this.preferences,
      preferencesLoaded: preferencesLoaded ?? this.preferencesLoaded,
      preferencesLoading: preferencesLoading ?? this.preferencesLoading,
      preferencesSaving: preferencesSaving ?? this.preferencesSaving,
      addresses: addresses ?? this.addresses,
      preferredAddressId: clearPreferredAddress
          ? null
          : (preferredAddressId ?? this.preferredAddressId),
      domainHttps: domainHttps ?? this.domainHttps,
      addressesLoading: addressesLoading ?? this.addressesLoading,
      addressesSaving: addressesSaving ?? this.addressesSaving,
      snmpProfiles: snmpProfiles ?? this.snmpProfiles,
      snmpLoading: snmpLoading ?? this.snmpLoading,
      operatingSystems: operatingSystems ?? this.operatingSystems,
      osLoaded: osLoaded ?? this.osLoaded,
      error: clearError ? null : (error ?? this.error),
    );
  }
}

class CoreSettingsNotifier extends Notifier<CoreSettingsState> {
  @override
  CoreSettingsState build() => const CoreSettingsState();

  // --- Global Preferences (/api/settings) ---

  Future<void> fetchPreferences({bool force = false}) async {
    if (state.preferencesLoading || (state.preferencesLoaded && !force)) return;
    state = state.copyWith(preferencesLoading: true, clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<Map<String, dynamic>>('/settings');
      state = state.copyWith(
        preferences: SystemPreferences.fromJson(raw),
        preferencesLoaded: true,
        preferencesLoading: false,
      );
    } catch (err) {
      state = state.copyWith(
        preferencesLoading: false,
        error: describeError(err),
      );
    }
  }

  Future<bool> savePreferences(SystemPreferences next) async {
    state = state.copyWith(preferencesSaving: true, clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .put<Map<String, dynamic>>('/settings', data: next.toJson());
      state = state.copyWith(
        preferences: SystemPreferences.fromJson(raw),
        preferencesLoaded: true,
        preferencesSaving: false,
      );
      return true;
    } catch (err) {
      state = state.copyWith(
        preferencesSaving: false,
        error: describeError(err),
      );
      return false;
    }
  }

  // --- Server Addresses (/api/server-addresses) ---

  Future<void> fetchServerAddresses({bool force = false}) async {
    if (state.addressesLoading || (state.addresses.isNotEmpty && !force)) {
      return;
    }
    state = state.copyWith(addressesLoading: true, clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<Map<String, dynamic>>('/server-addresses');
      state = state.copyWith(
        addresses: asJsonList<ServerAddressEntry>(
          raw['data'],
          ServerAddressEntry.fromJson,
        ),
        preferredAddressId: raw['preferredId'] as String?,
        clearPreferredAddress: raw['preferredId'] == null,
        domainHttps: raw['domainHttps'] == true,
        addressesLoading: false,
      );
    } catch (err) {
      state = state.copyWith(
        addressesLoading: false,
        error: describeError(err),
      );
    }
  }

  Future<bool> saveServerAddresses({
    required Map<String, String> overrides,
    required List<Map<String, dynamic>> custom,
    required String? preferredId,
    required bool domainHttps,
  }) async {
    state = state.copyWith(addressesSaving: true, clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .put<Map<String, dynamic>>(
            '/server-addresses',
            data: <String, dynamic>{
              'overrides': overrides,
              'custom': custom,
              'preferredId': preferredId,
              'domainHttps': domainHttps,
            },
          );
      state = state.copyWith(
        addresses: asJsonList<ServerAddressEntry>(
          raw['data'],
          ServerAddressEntry.fromJson,
        ),
        preferredAddressId: raw['preferredId'] as String?,
        clearPreferredAddress: raw['preferredId'] == null,
        domainHttps: raw['domainHttps'] == true,
        addressesSaving: false,
      );
      return true;
    } catch (err) {
      state = state.copyWith(addressesSaving: false, error: describeError(err));
      return false;
    }
  }

  // --- SNMP Profiles (/api/snmp/profiles) ---

  Future<void> fetchSnmpProfiles() async {
    state = state.copyWith(snmpLoading: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>('/snmp/profiles');
      state = state.copyWith(
        snmpProfiles: asJsonList<SnmpDeviceProfile>(
          raw,
          SnmpDeviceProfile.fromJson,
        ),
        snmpLoading: false,
      );
    } catch (err) {
      state = state.copyWith(snmpLoading: false, error: describeError(err));
    }
  }

  Future<bool> saveSnmpProfile(SnmpDeviceProfile profile) async {
    try {
      await ref
          .read(apiClientProvider)
          .post<dynamic>('/snmp/profiles', data: profile.toJson());
      await fetchSnmpProfiles();
      return true;
    } catch (err) {
      state = state.copyWith(error: describeError(err));
      return false;
    }
  }

  Future<bool> deleteSnmpProfile(String profileId) async {
    try {
      await ref
          .read(apiClientProvider)
          .delete<dynamic>('/snmp/profiles/${Uri.encodeComponent(profileId)}');
      await fetchSnmpProfiles();
      return true;
    } catch (err) {
      state = state.copyWith(error: describeError(err));
      return false;
    }
  }

  // --- Operating Systems (/api/devices/systems) ---

  Future<void> fetchOperatingSystems({bool force = false}) async {
    if (state.osLoaded && !force) return;
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>('/devices/systems');
      state = state.copyWith(
        operatingSystems: asJsonList<OperatingSystemOption>(
          raw,
          OperatingSystemOption.fromJson,
        ),
        osLoaded: true,
      );
    } catch (_) {}
  }
}

final coreSettingsProvider =
    NotifierProvider<CoreSettingsNotifier, CoreSettingsState>(
      CoreSettingsNotifier.new,
    );
