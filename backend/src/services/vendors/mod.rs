//! Fabricante de um equipamento a partir do MAC.
//!
//! Duas fontes, numa consulta só ([`registry::lookup`]):
//!
//! * o **registro oficial do IEEE** (MA-L, MA-M e MA-S — ~50 mil blocos),
//!   baixado pelo servidor e guardado em `oui_vendors` ([`service`]);
//! * a **tabela embutida** ([`builtin`]), curta, que vale sem internet e
//!   enquanto o registro não foi baixado.
//!
//! Quem pergunta (descoberta, conflitos de MAC, cadastro de dispositivo) não
//! sabe de onde veio a resposta — só recebe o nome e a origem.

pub mod builtin;
pub mod ieee;
pub mod registry;
pub mod service;

pub use registry::{lookup, vendor_name, MacVendor, VendorSource};
