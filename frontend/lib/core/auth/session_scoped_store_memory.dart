final Map<String, String> _memory = <String, String>{};

String? platformSessionRead(String key) => _memory[key];

void platformSessionWrite(String key, String value) => _memory[key] = value;

void platformSessionRemove(String key) => _memory.remove(key);
