//! O caminho, **no host**, da raiz das stacks.
//!
//! O CLI compose roda dentro do agente, mas quem resolve os bind mounts
//! relativos de uma stack (`./data`) é a Engine do host: o caminho que o
//! compose manda precisa existir lá. Antes isso obrigava a montar uma pasta
//! do host no mesmo caminho dos dois lados — e a pasta tinha de existir antes
//! do `docker compose up`.
//!
//! Agora a raiz pode ser qualquer montagem (um volume nomeado, criado pelo
//! próprio Docker, ou uma pasta): o agente pergunta à Engine de onde vem o que
//! está montado na raiz e cria, **dentro do container**, um link com o caminho
//! do host apontando para ela. O compose passa a trabalhar por esse caminho, e
//! a Engine recebe um caminho que existe de verdade. Fora de container (agente
//! nativo) o caminho já é o do host e nada muda.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// O id do container em que este processo roda, lido do `mountinfo`: o
/// `/etc/hostname` de todo container Docker é montado de
/// `<raiz>/containers/<id>/hostname` — também em `network_mode: host`, em que
/// o `HOSTNAME` é o do host e não serve.
#[must_use]
pub fn own_container_id(mountinfo: &str) -> Option<String> {
    mountinfo.lines().find_map(|line| {
        let start = line.find("/containers/")? + "/containers/".len();
        let rest = &line[start..];
        let id = rest.split('/').next()?;
        let is_id = id.len() == 64 && id.bytes().all(|byte| byte.is_ascii_hexdigit());
        (is_id && rest[id.len()..].starts_with("/hostname")).then(|| id.to_string())
    })
}

/// A origem, no host, do que está montado em `destination` (o `inspect` do
/// próprio container). Volume ou bind, tanto faz: `Source` é o caminho real.
#[must_use]
pub fn mount_source(inspected: &Value, destination: &Path) -> Option<PathBuf> {
    inspected
        .get("Mounts")?
        .as_array()?
        .iter()
        .find(|mount| {
            mount
                .get("Destination")
                .and_then(Value::as_str)
                .map(Path::new)
                == Some(destination)
        })
        .and_then(|mount| mount.get("Source")?.as_str())
        .filter(|source| !source.is_empty())
        .map(PathBuf::from)
}

/// Deixa `host` (caminho do host) apontando para `mounted` (onde a raiz está
/// montada neste container). Idempotente: um link certo já existente fica.
///
/// # Errors
///
/// O motivo, em português, quando o caminho do host já existe aqui dentro e
/// não é o link esperado, ou quando o link não pode ser criado.
pub fn ensure_alias(host: &Path, mounted: &Path) -> Result<(), String> {
    if host == mounted {
        return Ok(());
    }
    if let Ok(target) = std::fs::read_link(host) {
        if target == mounted {
            return Ok(());
        }
        std::fs::remove_file(host).map_err(|error| {
            format!("não foi possível trocar o link {}: {error}", host.display())
        })?;
    } else if host.exists() {
        return Err(format!(
            "{} já existe no agente e não é o link para {}",
            host.display(),
            mounted.display()
        ));
    }
    if let Some(parent) = host.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("não foi possível criar {}: {error}", parent.display()))?;
    }
    symlink(mounted, host)
        .map_err(|error| format!("não foi possível criar o link {}: {error}", host.display()))
}

#[cfg(unix)]
fn symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(not(unix))]
fn symlink(_target: &Path, _link: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "o agente em container só roda em Linux",
    ))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const ID: &str = "51e5370669315d99d5c941e866996fe7e283a02147c9f67fab076c2cd5c693f8";

    #[test]
    fn id_do_container_vem_do_hostname_montado() {
        let mountinfo = format!(
            "1 0 0:1 / / rw - overlay overlay rw\n\
             2 1 254:1 /docker/containers/{ID}/resolv.conf /etc/resolv.conf rw - ext4 /dev/vda1 rw\n\
             3 1 254:1 /docker/containers/{ID}/hostname /etc/hostname rw - ext4 /dev/vda1 rw\n"
        );
        assert_eq!(own_container_id(&mountinfo).as_deref(), Some(ID));
    }

    #[test]
    fn fora_de_container_nao_ha_id() {
        assert_eq!(
            own_container_id("1 0 0:1 / / rw - ext4 /dev/sda1 rw\n"),
            None
        );
        assert_eq!(
            own_container_id("3 1 254:1 /containers/abc/hostname /etc/hostname rw\n"),
            None
        );
    }

    #[test]
    fn origem_da_montagem_da_raiz() {
        let inspected = json!({"Mounts": [
            {"Destination": "/var/run/docker.sock", "Source": "/var/run/docker.sock"},
            {"Destination": "/stacks", "Source": "/var/lib/docker/volumes/x_stacks/_data"}
        ]});
        assert_eq!(
            mount_source(&inspected, Path::new("/stacks")),
            Some(PathBuf::from("/var/lib/docker/volumes/x_stacks/_data"))
        );
        assert_eq!(mount_source(&inspected, Path::new("/outra")), None);
    }

    #[test]
    fn mesmo_caminho_dos_dois_lados_nao_cria_link() {
        assert!(ensure_alias(Path::new("/srv/stacks"), Path::new("/srv/stacks")).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn cria_o_link_e_e_idempotente() {
        let dir = tempfile::tempdir().expect("tmp");
        let mounted = dir.path().join("stacks");
        std::fs::create_dir(&mounted).expect("raiz");
        let host = dir.path().join("var/lib/docker/volumes/x/_data");

        ensure_alias(&host, &mounted).expect("cria");
        ensure_alias(&host, &mounted).expect("de novo");
        std::fs::write(host.join("a"), b"ok").expect("escreve pelo link");
        assert!(mounted.join("a").exists());
    }

    #[test]
    fn caminho_ocupado_por_outra_coisa_e_recusado() {
        let dir = tempfile::tempdir().expect("tmp");
        let mounted = dir.path().join("stacks");
        let host = dir.path().join("ocupado");
        std::fs::create_dir_all(&host).expect("dir");
        assert!(ensure_alias(&host, &mounted).is_err());
    }
}
