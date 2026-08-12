//! `backend_rust-cli task vpn_probe_register` — registra o probe dedicado da VPN.
//!
//! ⚠️ **Não remover.** É o comando que deixa o `vpn-probe` pronto para o
//! heartbeat antes de o container do WireGuard subir. Sem ele, os monitores dos
//! peers ficam sem agente que enxergue a `wg0`.

use loco_rs::prelude::*;

use crate::services::vpn::probe_registrar;

pub struct VpnProbeRegister;

#[async_trait]
impl Task for VpnProbeRegister {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "vpn_probe_register".into(),
            detail: "Gera ou reutiliza o token do probe dedicado da VPN".into(),
        }
    }

    async fn run(&self, ctx: &AppContext, _vars: &task::Vars) -> Result<()> {
        let registration = probe_registrar::register_with_generated_token(&ctx.db).await?;
        let probe = &registration.probe;

        println!(
            "Probe \"{}\" (ID #{}) {}.",
            probe.name,
            probe.id,
            if registration.created {
                "registrado"
            } else {
                "atualizado"
            }
        );

        // O token só é impresso quando foi gerado aqui: o que veio do ambiente
        // o operador já tem, e repeti-lo no log seria vazamento sem ganho.
        if let Some(token) = registration.token {
            println!("----------------------------------------------------");
            println!("VPN_PROBE_TOKEN: {token}");
            println!("Guarde este token e configure-o no container vpn-probe.");
            println!("----------------------------------------------------");
        } else {
            println!("Token mantido a partir de VPN_PROBE_TOKEN (ou do padrão compartilhado).");
        }
        Ok(())
    }
}
