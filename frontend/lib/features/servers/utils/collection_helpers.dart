// Utilitários puros de coleções imutáveis para os Notifiers da área `servers` (DRY).

List<T> appendBounded<T>(List<T> current, Iterable<T> incoming, int maxItems) {
  final merged = <T>[...current, ...incoming];
  if (merged.length <= maxItems) {
    return List<T>.unmodifiable(merged);
  }
  return List<T>.unmodifiable(merged.sublist(merged.length - maxItems));
}

List<T> upsertSortedByName<T>(
  List<T> current,
  T item, {
  required int Function(T) idOf,
  required String Function(T) nameOf,
}) {
  final next = List<T>.from(current);
  final targetId = idOf(item);
  final idx = next.indexWhere((existing) => idOf(existing) == targetId);
  if (idx == -1) {
    next.add(item);
    next.sort(
      (a, b) => nameOf(a).toLowerCase().compareTo(nameOf(b).toLowerCase()),
    );
  } else {
    next[idx] = item;
  }
  return List<T>.unmodifiable(next);
}
