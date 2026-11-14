// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza
// SPDX-License-Identifier: MIT

//! La envoltura de PROCEDENCIA.
//!
//! Del barrido del 2026-09-22: de 18 importadores, **solo `polymarket`** guardaba
//! `id_externo` + `timestamp` de forma limpia; el resto perdía de dónde salió cada
//! dato. Aquí eso es el tipo base: todo lo que entra de una fuente externa se
//! envuelve, y así queda registrado QUÉ fuente, con QUÉ id y CUÁNDO se capturó.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Un dato traído de una fuente externa, con su procedencia atada.
///
/// `id_externo` es la clave que la FUENTE le da al registro (no la nuestra): es lo
/// que permite deduplicar y reanudar sin inventar identidades (directiva 20).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capturado<T> {
    /// Qué sistema externo lo produjo (p. ej. `"datos.gov.co/s54a-sgyg"`).
    pub fuente: String,
    /// El identificador que la fuente da al registro.
    pub id_externo: String,
    /// Instante de la captura (UTC), serializado RFC3339.
    #[serde(with = "time::serde::rfc3339")]
    pub capturado_en: OffsetDateTime,
    /// El dato en sí.
    pub dato: T,
}

impl<T> Capturado<T> {
    /// Envuelve un dato capturado en ESTE instante.
    pub fn ahora(fuente: impl Into<String>, id_externo: impl Into<String>, dato: T) -> Self {
        Self::en(fuente, id_externo, OffsetDateTime::now_utc(), dato)
    }

    /// Envuelve con un instante explícito (re-captura fechada, o pruebas).
    pub fn en(
        fuente: impl Into<String>,
        id_externo: impl Into<String>,
        capturado_en: OffsetDateTime,
        dato: T,
    ) -> Self {
        Self {
            fuente: fuente.into(),
            id_externo: id_externo.into(),
            capturado_en,
            dato,
        }
    }

    /// Transforma el dato CONSERVANDO la procedencia (parseo, normalización…).
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Capturado<U> {
        Capturado {
            fuente: self.fuente,
            id_externo: self.id_externo,
            capturado_en: self.capturado_en,
            dato: f(self.dato),
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn conserva_la_procedencia_en_json() {
        // POSITIVO: al serializar y deserializar, fuente/id/fecha/dato sobreviven.
        let c = Capturado::en("ideam", "2026-Q1", datetime!(2026-03-14 09:12:00 UTC), 42u32);
        let json = serde_json::to_string(&c).unwrap();
        let vuelta: Capturado<u32> = serde_json::from_str(&json).unwrap();
        assert_eq!(c, vuelta);
        // y la fecha va en RFC3339, no en un entero opaco:
        assert!(json.contains("2026-03-14T09:12:00Z"), "fecha no es RFC3339: {json}");
    }

    #[test]
    fn distingue_capturas_con_distinto_id() {
        // ROJO: dos capturas del MISMO dato pero con id_externo distinto NO son
        // iguales — si lo fueran, el dedup borraría una de dos filas legítimas.
        let a = Capturado::en("secop", "RAD-1", datetime!(2026-01-01 00:00:00 UTC), "x");
        let b = Capturado::en("secop", "RAD-2", datetime!(2026-01-01 00:00:00 UTC), "x");
        assert_ne!(a, b);
    }

    #[test]
    fn map_conserva_procedencia_y_cambia_dato() {
        let c = Capturado::en("dane", "05001", datetime!(2026-01-01 00:00:00 UTC), "  42 ");
        let n = c.clone().map(|s| s.trim().parse::<u32>().unwrap());
        assert_eq!(n.dato, 42);
        // la procedencia no se toca:
        assert_eq!((n.fuente, n.id_externo, n.capturado_en), (c.fuente, c.id_externo, c.capturado_en));
    }
}
