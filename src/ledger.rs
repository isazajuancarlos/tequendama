// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza
// SPDX-License-Identifier: MIT

//! Registro de cobertura REANUDABLE.
//!
//! Un import por lotes (cincuenta trimestres, mil ids) no se rehace entero si se
//! cae a la mitad. Este ledger guarda qué CLAVES ya se ingirieron, una por línea
//! en un archivo, y `abrir` las relee: el import pregunta `ya_cubierto` y se salta
//! lo hecho. Patrón de `ideam.rs`, hecho a prueba de caídas: `marcar` **añade** al
//! archivo (append) y hace `flush`, así que una caída no pierde lo ya marcado.

use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// Conjunto persistente de claves cubiertas.
pub struct Ledger {
    ruta: PathBuf,
    cubiertos: BTreeSet<String>,
}

impl Ledger {
    /// Abre el ledger en `ruta`, releyendo lo ya cubierto (o vacío si no existe).
    /// Las líneas en blanco se ignoran; el resto se toma tal cual (recortando el
    /// salto de línea, no los espacios internos: una clave puede llevarlos).
    pub fn abrir(ruta: impl Into<PathBuf>) -> io::Result<Self> {
        let ruta = ruta.into();
        let mut cubiertos = BTreeSet::new();
        match File::open(&ruta) {
            Ok(f) => {
                for linea in BufReader::new(f).lines() {
                    let linea = linea?;
                    let clave = linea.trim_end_matches(['\n', '\r']);
                    if !clave.is_empty() {
                        cubiertos.insert(clave.to_string());
                    }
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        Ok(Self { ruta, cubiertos })
    }

    /// ¿Ya se ingirió esta clave?
    pub fn ya_cubierto(&self, clave: &str) -> bool {
        self.cubiertos.contains(clave)
    }

    /// Marca una clave como cubierta y lo PERSISTE (append + flush). Idempotente:
    /// marcar dos veces la misma clave no la duplica ni la reescribe.
    pub fn marcar(&mut self, clave: impl Into<String>) -> io::Result<()> {
        let clave = clave.into();
        if self.cubiertos.contains(&clave) {
            return Ok(());
        }
        let mut f = OpenOptions::new().create(true).append(true).open(&self.ruta)?;
        writeln!(f, "{clave}")?;
        f.flush()?;
        self.cubiertos.insert(clave);
        Ok(())
    }

    /// Las claves cubiertas, en orden.
    pub fn cubiertos(&self) -> impl Iterator<Item = &String> {
        self.cubiertos.iter()
    }

    /// Cuántas claves hay cubiertas.
    pub fn len(&self) -> usize {
        self.cubiertos.len()
    }

    /// ¿No hay nada cubierto todavía?
    pub fn is_empty(&self) -> bool {
        self.cubiertos.is_empty()
    }

    /// La ruta del archivo del ledger.
    pub fn ruta(&self) -> &Path {
        &self.ruta
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn marca_persiste_y_reanuda() {
        let dir = tempdir().unwrap();
        let ruta = dir.path().join("cobertura.txt");
        {
            let mut l = Ledger::abrir(&ruta).unwrap();
            assert!(l.is_empty());
            l.marcar("2026-Q1").unwrap();
            l.marcar("2026-Q2").unwrap();
        }
        // REANUDA: un ledger nuevo sobre el mismo archivo ve lo marcado.
        let l = Ledger::abrir(&ruta).unwrap();
        assert_eq!(l.len(), 2);
        assert!(l.ya_cubierto("2026-Q1"));
        assert!(l.ya_cubierto("2026-Q2"));
    }

    #[test]
    fn ya_cubierto_discrimina() {
        // ROJO: una clave que NO se marcó devuelve false. Si `ya_cubierto` dijera
        // que sí a todo, un import se saltaría datos sin traer.
        let dir = tempdir().unwrap();
        let mut l = Ledger::abrir(dir.path().join("c.txt")).unwrap();
        l.marcar("A").unwrap();
        assert!(l.ya_cubierto("A"));
        assert!(!l.ya_cubierto("B"));
    }

    #[test]
    fn marcar_es_idempotente_en_disco() {
        let dir = tempdir().unwrap();
        let ruta = dir.path().join("c.txt");
        let mut l = Ledger::abrir(&ruta).unwrap();
        l.marcar("X").unwrap();
        l.marcar("X").unwrap();
        l.marcar("X").unwrap();
        assert_eq!(l.len(), 1);
        // en disco tampoco se duplicó:
        let texto = std::fs::read_to_string(&ruta).unwrap();
        assert_eq!(texto.matches('X').count(), 1);
    }

    #[test]
    fn simulacion_100_reanuda_completo() {
        // Directiva 8: 120 claves marcadas, reabrir, todas presentes y ninguna de
        // más. Simula el import por lotes que se retoma.
        let dir = tempdir().unwrap();
        let ruta = dir.path().join("lote.txt");
        {
            let mut l = Ledger::abrir(&ruta).unwrap();
            for i in 0..120 {
                l.marcar(format!("clave-{i:03}")).unwrap();
            }
        }
        let l = Ledger::abrir(&ruta).unwrap();
        assert_eq!(l.len(), 120);
        for i in 0..120 {
            assert!(l.ya_cubierto(&format!("clave-{i:03}")), "falta {i}");
        }
        assert!(!l.ya_cubierto("clave-120"));
    }
}
