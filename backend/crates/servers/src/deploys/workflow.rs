//! O `.github/workflows/deploy.yml` sugerido para uma aplicação — puro.
//!
//! Duas regras de segurança guiam o texto:
//!
//! * nada do evento (mensagem do commit, nome de branch) é interpolado com
//!   `${{ }}` dentro de `run:` — entra por `env:` e o JSON sai do `jq`. Uma
//!   mensagem com aspas não vira comando;
//! * nenhum segredo: a central confere o token OIDC do próprio GitHub
//!   (`id-token: write`), e o endereço dela é uma *variável* do repositório.
//!
//! A tag das imagens é o SHA do commit — imutável. É ela que o agente baixa e
//! é a ela que o rollback volta.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use crate::dtos::deploys::BuildImage;

/// A variável do repositório com o endereço público da central.
pub const URL_VARIABLE: &str = "NETMONITOR_URL";
/// Onde o arquivo mora no repositório.
pub const WORKFLOW_PATH: &str = ".github/workflows/deploy.yml";
/// A rota que recebe o aviso.
pub const HOOK_PATH: &str = "/api/deploy-hooks/github";

/// O que o gerador precisa saber.
pub struct WorkflowSpec<'a> {
    pub application_name: &'a str,
    pub images: &'a [BuildImage],
    /// Os filtros dos alvos: branches exatas e padrões de tag (`v*`).
    pub references: &'a [String],
    pub audience: &'a str,
}

fn quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// O YAML.
#[must_use]
pub fn render(spec: &WorkflowSpec<'_>) -> String {
    let mut branches = BTreeSet::new();
    let mut tags = BTreeSet::new();
    for reference in spec.references {
        let reference = reference.trim();
        if reference.ends_with('*') {
            tags.insert(reference.to_string());
        } else if !reference.is_empty() {
            branches.insert(reference.to_string());
        }
    }
    if branches.is_empty() && tags.is_empty() {
        branches.insert("main".to_string());
    }

    let mut out = String::new();
    let _ = writeln!(
        out,
        "# Gerado pelo NetMonitor para {}.",
        spec.application_name
    );
    out.push_str(
        "# Compila as imagens, publica no ghcr.io com a tag do commit e avisa a central,\n\
         # que decide quando e onde aplicar (aprovação, janela, backup, rollback).\n\
         #\n\
         # Configure no repositório: Settings → Secrets and variables → Actions →\n",
    );
    let _ = writeln!(
        out,
        "# Variables → {URL_VARIABLE} = endereço público da central (https://…)."
    );
    out.push_str(
        "# Nenhum segredo é necessário: a central confere o token OIDC do próprio GitHub.\n\
         #\n\
         # Mantenha os testes do projeto: acrescente o job deles ao `needs` do build.\n\
         name: Build e deploy (NetMonitor)\n\non:\n  push:\n",
    );
    if !branches.is_empty() {
        let list: Vec<String> = branches.iter().map(|branch| quoted(branch)).collect();
        let _ = writeln!(out, "    branches: [{}]", list.join(", "));
    }
    if !tags.is_empty() {
        let list: Vec<String> = tags.iter().map(|tag| quoted(tag)).collect();
        let _ = writeln!(out, "    tags: [{}]", list.join(", "));
    }
    out.push_str(
        "\nconcurrency:\n  group: deploy-${{ github.ref }}\n  cancel-in-progress: false\n\n\
         permissions:\n  contents: read\n  packages: write\n  id-token: write\n\njobs:\n",
    );

    if !spec.images.is_empty() {
        out.push_str(
            "  build:\n    runs-on: ubuntu-latest\n    steps:\n\
             \x20     - uses: actions/checkout@v4\n\n\
             \x20     # O ghcr.io só aceita nomes minúsculos.\n\
             \x20     - name: Prefixo das imagens\n\
             \x20       run: echo \"IMAGE_PREFIX=ghcr.io/${GITHUB_REPOSITORY_OWNER,,}\" >> \"$GITHUB_ENV\"\n\n\
             \x20     - uses: docker/login-action@v3\n\
             \x20       with:\n\
             \x20         registry: ghcr.io\n\
             \x20         username: ${{ github.actor }}\n\
             \x20         password: ${{ secrets.GITHUB_TOKEN }}\n\n\
             \x20     - uses: docker/setup-buildx-action@v3\n",
        );
        for image in spec.images {
            let _ = write!(
                out,
                "\n      - name: Imagem {name}\n\
                 \x20       uses: docker/build-push-action@v6\n\
                 \x20       with:\n\
                 \x20         context: {context}\n\
                 \x20         file: {file}\n\
                 \x20         push: true\n\
                 \x20         tags: ${{{{ env.IMAGE_PREFIX }}}}/{name}:${{{{ github.sha }}}}\n\
                 \x20         cache-from: type=gha,scope={name}\n\
                 \x20         cache-to: type=gha,mode=max,scope={name}\n",
                name = image.name,
                context = quoted(&image.context),
                file = quoted(&image.dockerfile),
            );
        }
        out.push('\n');
    }

    out.push_str("  deploy:\n");
    if !spec.images.is_empty() {
        out.push_str("    needs: build\n");
    }
    let _ = write!(
        out,
        "    runs-on: ubuntu-latest\n    steps:\n\
         \x20     - name: Avisar o NetMonitor\n\
         \x20       env:\n\
         \x20         NETMONITOR_URL: ${{{{ vars.{URL_VARIABLE} }}}}\n\
         \x20         COMMIT_MESSAGE: ${{{{ github.event.head_commit.message }}}}\n\
         \x20       run: |\n\
         \x20         set -euo pipefail\n\
         \x20         TOKEN=$(curl -fsS -H \"Authorization: bearer $ACTIONS_ID_TOKEN_REQUEST_TOKEN\" \\\n\
         \x20           \"$ACTIONS_ID_TOKEN_REQUEST_URL&audience={audience}\" | jq -r .value)\n\
         \x20         BODY=$(jq -n --arg message \"${{COMMIT_MESSAGE:-}}\" --arg tag \"$GITHUB_SHA\" \\\n\
         \x20           '{{commitMessage: $message, imageTag: $tag}}')\n\
         \x20         curl -fsS --retry 3 -X POST \"$NETMONITOR_URL{HOOK_PATH}\" \\\n\
         \x20           -H \"Authorization: Bearer $TOKEN\" \\\n\
         \x20           -H 'Content-Type: application/json' \\\n\
         \x20           -d \"$BODY\"\n",
        audience = spec.audience,
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn images() -> Vec<BuildImage> {
        vec![
            BuildImage {
                name: "sgc-api".into(),
                dockerfile: "apps/api/Dockerfile".into(),
                context: ".".into(),
            },
            BuildImage {
                name: "sgc-front-tenants".into(),
                dockerfile: "docker/frontend/Dockerfile".into(),
                context: ".".into(),
            },
        ]
    }

    #[test]
    fn workflow_completo() {
        let images = images();
        let references = vec!["main".to_string(), "develop".to_string(), "v*".to_string()];
        let yaml = render(&WorkflowSpec {
            application_name: "SGCv4",
            images: &images,
            references: &references,
            audience: "netmonitor-deploy",
        });
        insta::assert_snapshot!(yaml);
    }

    #[test]
    fn nada_do_evento_vai_direto_ao_shell() {
        let images = images();
        let yaml = render(&WorkflowSpec {
            application_name: "SGCv4",
            images: &images,
            references: &["main".to_string()],
            audience: "netmonitor-deploy",
        });
        let run_blocks: Vec<&str> = yaml
            .split("run: |")
            .skip(1)
            .map(|block| block.split("\n      - ").next().unwrap_or(block))
            .collect();
        assert!(!run_blocks.is_empty());
        for block in run_blocks {
            assert!(
                !block.contains("${{"),
                "interpolação dentro de run: {block}"
            );
        }
        assert!(yaml.contains("id-token: write"));
        assert!(!yaml.contains("secrets.NETMONITOR"));
    }

    #[test]
    fn sem_imagens_so_avisa() {
        let yaml = render(&WorkflowSpec {
            application_name: "Stack",
            images: &[],
            references: &[],
            audience: "x",
        });
        assert!(!yaml.contains("build-push-action"));
        assert!(!yaml.contains("needs: build"));
        assert!(yaml.contains("branches: ['main']"));
    }
}
