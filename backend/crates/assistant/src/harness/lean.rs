//! O resultado de uma ferramenta como a IA o lê: o mesmo conteúdo, com menos
//! tokens.
//!
//! Cada rodada reenvia os resultados anteriores, então o que se economiza aqui
//! se multiplica pelo número de rodadas. Quatro cortes, nenhum perde
//! informação que a IA use:
//!
//! 1. **Vazio sai**: `null`, `""`, `[]` e `{}` não dizem nada. `false` e `0`
//!    ficam — são resposta.
//! 2. **Lista de objetos vira tabela**: `[{"id":1,"nome":"a"},{"id":2,…}]`
//!    repete as chaves em cada item; `{"columns":["id","nome"],"rows":[[1,"a"],
//!    [2,…]]}` as diz uma vez. É o corte que mais rende (listas de
//!    dispositivos, alertas, linhas do grep).
//! 3. **Número com casas demais** vira duas casas (quatro, se for menor que
//!    0,01): `12.3456789 ms` não diagnostica melhor que `12.35 ms`.
//! 4. **Instante sem fração de segundo e em UTC curto**:
//!    `2026-09-30T11:52:33.617579965+00:00` → `2026-09-30T11:52:33Z`.
//!
//! Só o texto que vai ao modelo passa por aqui; a tela recebe o resultado
//! original pelo evento `ToolResult` (gráficos, cartões).

use serde_json::{Map, Value};

/// A partir de quantos itens uma lista de objetos vale virar tabela. Com
/// menos, o cabeçalho custa o que economiza.
const MIN_TABLE_ROWS: usize = 3;

/// O resultado enxuto. Idempotente: passar duas vezes dá o mesmo.
#[must_use]
pub fn lean(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .filter_map(|(key, value)| {
                    let value = lean(value);
                    (!is_empty(&value)).then(|| (key.clone(), value))
                })
                .collect(),
        ),
        Value::Array(items) => {
            let items: Vec<Value> = items.iter().map(lean).collect();
            tabulate(&items).unwrap_or(Value::Array(items))
        }
        Value::Number(number) => round(number),
        Value::String(text) => Value::String(short_instant(text).unwrap_or_else(|| text.clone())),
        other => other.clone(),
    }
}

fn is_empty(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(text) => text.is_empty(),
        Value::Array(items) => items.is_empty(),
        Value::Object(map) => map.is_empty(),
        _ => false,
    }
}

/// Lista de objetos → `{columns, rows}`. As colunas são a união das chaves na
/// ordem em que aparecem; item sem a coluna leva `null` na posição.
fn tabulate(items: &[Value]) -> Option<Value> {
    if items.len() < MIN_TABLE_ROWS || !items.iter().all(Value::is_object) {
        return None;
    }
    let mut columns: Vec<String> = Vec::new();
    for item in items {
        for key in item.as_object()?.keys() {
            if !columns.contains(key) {
                columns.push(key.clone());
            }
        }
    }
    let rows: Vec<Value> = items
        .iter()
        .filter_map(Value::as_object)
        .map(|item| {
            Value::Array(
                columns
                    .iter()
                    .map(|column| item.get(column).cloned().unwrap_or(Value::Null))
                    .collect(),
            )
        })
        .collect();
    let mut table = Map::new();
    table.insert(
        "columns".into(),
        Value::Array(columns.into_iter().map(Value::String).collect()),
    );
    table.insert("rows".into(), Value::Array(rows));
    Some(Value::Object(table))
}

fn round(number: &serde_json::Number) -> Value {
    if number.is_i64() || number.is_u64() {
        return Value::Number(number.clone());
    }
    let Some(float) = number.as_f64() else {
        return Value::Number(number.clone());
    };
    let places = if float != 0.0 && float.abs() < 0.01 {
        4
    } else {
        2
    };
    let factor = 10_f64.powi(places);
    let rounded = (float * factor).round() / factor;
    if rounded.fract() == 0.0 && rounded.abs() < 9.0e15 {
        #[allow(clippy::cast_possible_truncation)]
        return Value::from(rounded as i64);
    }
    serde_json::Number::from_f64(rounded).map_or(Value::Null, Value::Number)
}

/// `AAAA-MM-DDTHH:MM:SS[.fração][Z|+00:00]` → `AAAA-MM-DDTHH:MM:SSZ`.
/// Outro fuso fica como está (só perde a fração): trocar de fuso aqui mudaria
/// o que a IA mostra ao usuário.
fn short_instant(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let looks_like = text.len() >= 20
        && bytes.get(4) == Some(&b'-')
        && bytes.get(10) == Some(&b'T')
        && bytes.get(13) == Some(&b':')
        && bytes.get(16) == Some(&b':')
        && text[..19]
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '-' | 'T' | ':'));
    if !looks_like {
        return None;
    }
    let (base, rest) = text.split_at(19);
    let zone = rest.trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    if zone.len() == rest.len() && !rest.is_empty() && zone != "Z" && zone != "+00:00" {
        return None;
    }
    let zone = match zone {
        "Z" | "+00:00" | "-00:00" => "Z",
        other => other,
    };
    Some(format!("{base}{zone}"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn vazio_sai_e_resposta_fica() {
        assert_eq!(
            lean(
                &json!({ "a": null, "b": "", "c": [], "d": {}, "e": false, "f": 0, "g": { "h": null } })
            ),
            json!({ "e": false, "f": 0 })
        );
    }

    #[test]
    fn lista_de_objetos_vira_tabela() {
        let devices = json!([
            { "id": 1, "name": "Borda", "status": "up" },
            { "id": 2, "name": "Core", "status": "down", "note": "sem energia" },
            { "id": 3, "name": "AP-1", "status": "up" }
        ]);
        assert_eq!(
            lean(&devices),
            json!({
                "columns": ["id", "name", "status", "note"],
                "rows": [
                    [1, "Borda", "up", null],
                    [2, "Core", "down", "sem energia"],
                    [3, "AP-1", "up", null]
                ]
            })
        );
        // Poucos itens ou itens de tipos misturados continuam lista.
        assert_eq!(
            lean(&json!([{ "a": 1 }, { "a": 2 }])),
            json!([{ "a": 1 }, { "a": 2 }])
        );
        assert_eq!(lean(&json!([1, 2, 3])), json!([1, 2, 3]));
    }

    #[test]
    fn numeros_e_instantes_encurtam() {
        assert_eq!(
            lean(&json!([12.345_678, 0.001_234_5, 3.0, 7, -0.004])),
            json!([12.35, 0.0012, 3, 7, -0.004])
        );
        assert_eq!(
            lean(&json!("2026-09-30T11:52:33.617579965+00:00")),
            json!("2026-09-30T11:52:33Z")
        );
        assert_eq!(
            lean(&json!("2026-09-30T08:52:33-03:00")),
            json!("2026-09-30T08:52:33-03:00")
        );
        assert_eq!(
            lean(&json!("2026-09-30T08:52:33.5-03:00")),
            json!("2026-09-30T08:52:33-03:00")
        );
        assert_eq!(lean(&json!("não é data")), json!("não é data"));
    }

    #[test]
    fn e_idempotente_e_economiza_numa_lista_real() {
        let rows: Vec<Value> = (1..=40)
            .map(|n| {
                json!({
                    "id": n,
                    "name": format!("SW-ANDAR-{n}"),
                    "ipAddress": format!("10.0.{n}.1"),
                    "type": "switch",
                    "vendor": null,
                    "status": if n % 7 == 0 { "down" } else { "up" },
                    "lastSeenAt": "2026-09-30T11:52:33.617579965+00:00",
                    "latencyMs": 1.234_567 * f64::from(n),
                })
            })
            .collect();
        let original = json!({ "devices": rows, "total": 40 });
        let once = lean(&original);
        assert_eq!(lean(&once), once, "idempotente");
        let before = original.to_string().len();
        let after = once.to_string().len();
        assert!(
            after * 100 / before <= 55,
            "economia abaixo do esperado: {before} → {after}"
        );
    }
}
