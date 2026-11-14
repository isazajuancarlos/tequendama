// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza
// SPDX-License-Identifier: MIT

//! Limitador de ritmo: intervalo mínimo entre operaciones.
//!
//! Del barrido: dos formas ad hoc —`sleep` fijo en los tres conectores IDEAM, y
//! una clase `RateLimiter` de intervalo mínimo en `polymarket`—. Aquí, una.

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Garantiza que dos llamadas consecutivas no ocurran más juntas que `intervalo`.
/// `esperar()` bloquea lo justo. Seguro entre hilos.
pub struct Ritmo {
    intervalo: Duration,
    ultimo: Mutex<Option<Instant>>,
}

impl Ritmo {
    /// Un ritmo con `intervalo` mínimo entre llamadas.
    pub fn nuevo(intervalo: Duration) -> Self {
        Self {
            intervalo,
            ultimo: Mutex::new(None),
        }
    }

    /// Bloquea hasta que haya pasado `intervalo` desde la última llamada, y marca
    /// esta como la última.
    pub fn esperar(&self) {
        let ahora = Instant::now();
        let espera = {
            let mut ultimo = self.ultimo.lock().unwrap_or_else(|e| e.into_inner());
            let espera = espera_necesaria(*ultimo, ahora, self.intervalo);
            // La próxima llamada se mide desde AHORA+espera, no desde `ahora`, para
            // que el ritmo no se desfase si `esperar` se llama en ráfaga.
            *ultimo = Some(ahora + espera);
            espera
        };
        if !espera.is_zero() {
            std::thread::sleep(espera);
        }
    }
}

/// Cuánto hay que esperar dado el instante de la última llamada, el actual y el
/// intervalo. Pura: es lo que se prueba.
fn espera_necesaria(ultimo: Option<Instant>, ahora: Instant, intervalo: Duration) -> Duration {
    match ultimo {
        None => Duration::ZERO,
        Some(t) => {
            let proxima = t + intervalo;
            proxima.saturating_duration_since(ahora)
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn primera_llamada_no_espera() {
        let base = Instant::now();
        assert_eq!(espera_necesaria(None, base, Duration::from_secs(1)), Duration::ZERO);
    }

    #[test]
    fn espera_si_es_demasiado_pronto_y_no_si_ya_pasó() {
        let base = Instant::now();
        let intervalo = Duration::from_millis(200);
        // POSITIVO: llamada 50 ms después de la última → espera ~150 ms.
        let e = espera_necesaria(Some(base), base + Duration::from_millis(50), intervalo);
        assert_eq!(e, Duration::from_millis(150));
        // ROJO del mismo control: llamada 300 ms después (ya pasó) → NO espera.
        let e = espera_necesaria(Some(base), base + Duration::from_millis(300), intervalo);
        assert_eq!(e, Duration::ZERO);
    }

    #[test]
    fn de_verdad_separa_las_llamadas() {
        // Directiva 8/28: cruzar a `Instant` real. 5 llamadas con 20 ms de intervalo
        // tardan al menos 4×20 ms en total (la primera no espera).
        let ritmo = Ritmo::nuevo(Duration::from_millis(20));
        let t0 = Instant::now();
        for _ in 0..5 {
            ritmo.esperar();
        }
        assert!(
            t0.elapsed() >= Duration::from_millis(80),
            "no respetó el ritmo: {:?}",
            t0.elapsed()
        );
    }
}
