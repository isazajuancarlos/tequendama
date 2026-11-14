// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza
// SPDX-License-Identifier: MIT

//! # tequendama
//!
//! Ingestión de datos externos, sin reinventarla en cada importador.
//!
//! El barrido del universo (2026-09-22) encontró 18 importadores en 7 repos
//! reinventando las mismas cinco cosas, cada uno distinto. Este crate las reúne:
//!
//! - [`http::Cliente`] — `reqwest` (blocking) con timeout, ritmo y reintentos.
//! - [`reintentos::con_reintentos`] — backoff exponencial con techo.
//! - [`ritmo::Ritmo`] — intervalo mínimo entre llamadas.
//! - [`Capturado`] — la envoltura de procedencia (`fuente`, `id_externo`,
//!   `capturado_en`, `dato`).
//! - [`ledger::Ledger`] — cobertura reanudable para imports por lotes.
//! - [`socrata::Socrata`] — cliente `datos.gov.co` (SoQL + paginación).
//!
//! Para CSV se adopta el crate [`csv`](https://docs.rs/csv), en vez del
//! `split(',')` naíf que traía red-hídrica.
//!
//! Lo idiosincrático de cada fuente (nombres de departamentos, relleno RIPS,
//! semántica H1…) NO vive aquí: se queda en su repo (directiva 9, «nunca el alfa»).

pub mod capturado;
pub mod http;
pub mod ledger;
pub mod reintentos;
pub mod ritmo;
pub mod socrata;

pub use capturado::Capturado;
pub use http::{Cliente, ErrorHttp};
pub use ledger::Ledger;
pub use reintentos::{Politica, con_reintentos};
pub use ritmo::Ritmo;
pub use socrata::{Consulta, ErrorSocrata, Socrata};

// El crate `csv` se re-exporta para que los importadores lo usen desde aquí y no
// vuelvan a un parseo a mano.
pub use csv;
