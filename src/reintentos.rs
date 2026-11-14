// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza
// SPDX-License-Identifier: MIT

//! Reintentos con backoff exponencial y techo.
//!
//! Del barrido: 5 reinvenciones distintas (lineal fijo copiado 3×, catch-all con
//! `sleep` fijo, ausente, y el exponencial-con-techo de `polymarket` — el único
//! bien hecho). Aquí, ese último, una vez.

use std::time::Duration;

/// Cuántas veces y con qué esperas se reintenta.
#[derive(Debug, Clone)]
pub struct Politica {
    /// Nº TOTAL de intentos, contando el primero. `1` = no reintenta.
    pub intentos_max: u32,
    /// Espera tras el primer fallo.
    pub base: Duration,
    /// Tope de la espera (el exponencial no crece sin límite).
    pub techo: Duration,
    /// Factor de crecimiento entre esperas (p. ej. `2.0`).
    pub multiplicador: f64,
}

impl Default for Politica {
    /// El de polymarket: 5 intentos, 1s → 60s, ×2.
    fn default() -> Self {
        Self {
            intentos_max: 5,
            base: Duration::from_secs(1),
            techo: Duration::from_secs(60),
            multiplicador: 2.0,
        }
    }
}

impl Politica {
    /// Espera ANTES del reintento número `n` (n = 1 es el primer reintento, tras el
    /// primer fallo). Función pura: se puede probar sin dormir.
    pub fn espera(&self, n: u32) -> Duration {
        if n == 0 {
            return Duration::ZERO;
        }
        let factor = self.multiplicador.powi((n - 1) as i32);
        let segundos = self.base.as_secs_f64() * factor;
        let techo = self.techo.as_secs_f64();
        // `min` sobre f64 y guarda contra NaN/negativos (multiplicador raro).
        let segundos = if segundos.is_finite() && segundos >= 0.0 {
            segundos.min(techo)
        } else {
            techo
        };
        Duration::from_secs_f64(segundos)
    }

    /// Política sin esperas (para pruebas): mismos intentos, `sleep` de cero.
    #[doc(hidden)]
    pub fn instantanea(intentos_max: u32) -> Self {
        Self {
            intentos_max,
            base: Duration::ZERO,
            techo: Duration::ZERO,
            multiplicador: 1.0,
        }
    }
}

/// Ejecuta `f`, reintentando mientras el error sea `reintentable` y queden
/// intentos. Devuelve el primer `Ok`, o el ÚLTIMO error si se agotan.
///
/// `reintentable` decide, por error, si vale la pena reintentar: un timeout o un
/// 5xx sí; un 404 o un fallo de parseo, no (reintentarlos es girar en falso).
pub fn con_reintentos<T, E, F, R>(pol: &Politica, mut reintentable: R, mut f: F) -> Result<T, E>
where
    F: FnMut() -> Result<T, E>,
    R: FnMut(&E) -> bool,
{
    let intentos_max = pol.intentos_max.max(1);
    let mut n = 0u32; // nº de reintentos ya hechos
    loop {
        match f() {
            Ok(v) => return Ok(v),
            Err(e) => {
                let intentos_hechos = n + 1;
                if intentos_hechos >= intentos_max || !reintentable(&e) {
                    return Err(e);
                }
                n += 1;
                std::thread::sleep(pol.espera(n));
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn backoff_crece_y_se_topa() {
        // POSITIVO: crece exponencial… y ROJO del mismo control: NO pasa del techo.
        let p = Politica {
            intentos_max: 10,
            base: Duration::from_secs(1),
            techo: Duration::from_secs(10),
            multiplicador: 2.0,
        };
        assert_eq!(p.espera(1), Duration::from_secs(1));
        assert_eq!(p.espera(2), Duration::from_secs(2));
        assert_eq!(p.espera(3), Duration::from_secs(4));
        assert_eq!(p.espera(4), Duration::from_secs(8));
        assert_eq!(p.espera(5), Duration::from_secs(10)); // 16 topado a 10
        assert_eq!(p.espera(50), Duration::from_secs(10)); // sigue topado
    }

    #[test]
    fn exito_al_primer_intento_no_reintenta() {
        let llamadas = Cell::new(0);
        let r: Result<u8, ()> = con_reintentos(&Politica::instantanea(5), |_| true, || {
            llamadas.set(llamadas.get() + 1);
            Ok(7)
        });
        assert_eq!(r, Ok(7));
        assert_eq!(llamadas.get(), 1);
    }

    #[test]
    fn reintenta_y_luego_acierta() {
        let llamadas = Cell::new(0);
        let r: Result<u8, &str> = con_reintentos(&Politica::instantanea(5), |_| true, || {
            let n = llamadas.get() + 1;
            llamadas.set(n);
            if n < 3 { Err("timeout") } else { Ok(9) }
        });
        assert_eq!(r, Ok(9));
        assert_eq!(llamadas.get(), 3); // 2 fallos + 1 acierto
    }

    #[test]
    fn agota_los_intentos_y_devuelve_el_ultimo_error() {
        let llamadas = Cell::new(0);
        let r: Result<u8, i32> = con_reintentos(&Politica::instantanea(4), |_| true, || {
            let n = llamadas.get() + 1;
            llamadas.set(n);
            Err(n) // error distinto cada vez, para ver que devuelve el ÚLTIMO
        });
        assert_eq!(r, Err(4)); // 4 intentos, el último error
        assert_eq!(llamadas.get(), 4);
    }

    #[test]
    fn error_no_reintentable_no_gasta_intentos() {
        // ROJO por la vía real: un error que `reintentable` marca `false` corta al
        // instante, sin agotar los 5 intentos. Si con_reintentos ignorara el
        // predicado, llamadas sería 5 y esta prueba fallaría.
        let llamadas = Cell::new(0);
        let r: Result<u8, &str> = con_reintentos(
            &Politica::instantanea(5),
            |e| *e != "404",
            || {
                llamadas.set(llamadas.get() + 1);
                Err("404")
            },
        );
        assert_eq!(r, Err("404"));
        assert_eq!(llamadas.get(), 1);
    }

    #[test]
    fn simulacion_100_flaky() {
        // Directiva 8: 100+ operaciones con fallo inyectado. Cada tarea falla las
        // primeras k veces (k = i % 4) y luego acierta; con 5 intentos todas deben
        // terminar en Ok, y ninguna debe llamar más de k+1 veces.
        for i in 0..150u32 {
            let k = i % 4;
            let llamadas = Cell::new(0u32);
            let r: Result<u32, &str> = con_reintentos(&Politica::instantanea(5), |_| true, || {
                let n = llamadas.get() + 1;
                llamadas.set(n);
                if n <= k { Err("flaky") } else { Ok(i) }
            });
            assert_eq!(r, Ok(i), "tarea {i} debió acertar");
            assert_eq!(llamadas.get(), k + 1, "tarea {i} llamó de más");
        }
    }
}
