// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza
// SPDX-License-Identifier: MIT

//! Cliente HTTP: `reqwest` (blocking) con timeout, ritmo y reintentos integrados.
//!
//! Del barrido: dos clientes compitiendo (`reqwest` en 4 repos, `ureq` en
//! red-hídrica). Se estandariza en `reqwest`. Este envoltorio ata las tres cosas
//! que cada importador cableaba a mano: timeout, un intervalo mínimo entre
//! llamadas, y reintentos que distinguen lo transitorio de lo definitivo.

use std::fmt;
use std::time::Duration;

use serde::de::DeserializeOwned;

use crate::reintentos::{Politica, con_reintentos};
use crate::ritmo::Ritmo;

/// Lo que puede salir mal al traer algo por HTTP.
#[derive(Debug)]
pub enum ErrorHttp {
    /// Fallo de transporte (timeout, conexión, DNS…).
    Transporte(reqwest::Error),
    /// El servidor respondió con un código que no es 2xx.
    Estado { codigo: u16, url: String },
    /// El cuerpo no se pudo leer o deserializar.
    Cuerpo(reqwest::Error),
}

impl ErrorHttp {
    /// ¿Vale la pena reintentar? Timeout/conexión: sí. `429` y `5xx`: sí (el
    /// servidor está saturado o caído, es transitorio). Los demás `4xx`: **no** —
    /// el problema es la petición, reintentarla es girar en falso. Cuerpo ilegible:
    /// no — el mismo cuerpo volverá igual.
    pub fn reintentable(&self) -> bool {
        match self {
            ErrorHttp::Transporte(e) => e.is_timeout() || e.is_connect(),
            ErrorHttp::Estado { codigo, .. } => *codigo == 429 || (500..600).contains(codigo),
            ErrorHttp::Cuerpo(_) => false,
        }
    }
}

impl fmt::Display for ErrorHttp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorHttp::Transporte(e) => write!(f, "transporte: {e}"),
            ErrorHttp::Estado { codigo, url } => write!(f, "estado {codigo} en {url}"),
            ErrorHttp::Cuerpo(e) => write!(f, "cuerpo: {e}"),
        }
    }
}

impl std::error::Error for ErrorHttp {}

/// Cliente HTTP con ritmo y reintentos.
pub struct Cliente {
    http: reqwest::blocking::Client,
    ritmo: Ritmo,
    politica: Politica,
}

impl Cliente {
    /// Un cliente con `timeout` por petición, `intervalo_min` entre llamadas y la
    /// `politica` de reintentos dada.
    pub fn nuevo(
        timeout: Duration,
        intervalo_min: Duration,
        politica: Politica,
    ) -> reqwest::Result<Self> {
        let http = reqwest::blocking::Client::builder().timeout(timeout).build()?;
        Ok(Self {
            http,
            ritmo: Ritmo::nuevo(intervalo_min),
            politica,
        })
    }

    /// Un GET a `url`, con ritmo y reintentos. Devuelve la respuesta 2xx o el error.
    fn traer(&self, url: &str) -> Result<reqwest::blocking::Response, ErrorHttp> {
        con_reintentos(
            &self.politica,
            |e: &ErrorHttp| e.reintentable(),
            || {
                self.ritmo.esperar();
                let resp = self.http.get(url).send().map_err(ErrorHttp::Transporte)?;
                let codigo = resp.status().as_u16();
                if resp.status().is_success() {
                    Ok(resp)
                } else {
                    Err(ErrorHttp::Estado {
                        codigo,
                        url: url.to_string(),
                    })
                }
            },
        )
    }

    /// GET que devuelve el cuerpo como texto.
    pub fn obtener_texto(&self, url: &str) -> Result<String, ErrorHttp> {
        self.traer(url)?.text().map_err(ErrorHttp::Cuerpo)
    }

    /// GET que deserializa el cuerpo JSON en `T`.
    pub fn obtener_json<T: DeserializeOwned>(&self, url: &str) -> Result<T, ErrorHttp> {
        self.traer(url)?.json::<T>().map_err(ErrorHttp::Cuerpo)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn clasifica_estado_reintentable_o_no() {
        // POSITIVO: 5xx y 429 se reintentan.
        assert!(ErrorHttp::Estado { codigo: 503, url: "x".into() }.reintentable());
        assert!(ErrorHttp::Estado { codigo: 500, url: "x".into() }.reintentable());
        assert!(ErrorHttp::Estado { codigo: 429, url: "x".into() }.reintentable());
        // ROJO del mismo control: 404 y 400 NO — reintentarlos gira en falso.
        assert!(!ErrorHttp::Estado { codigo: 404, url: "x".into() }.reintentable());
        assert!(!ErrorHttp::Estado { codigo: 400, url: "x".into() }.reintentable());
    }

    /// Servidor HTTP mínimo que sirve `respuestas` (una por conexión) y termina.
    /// Cruza la frontera del socket (D28) sin salir a la red externa.
    fn servidor(respuestas: Vec<(u16, String)>) -> (String, thread::JoinHandle<usize>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            let mut servidas = 0usize;
            for (codigo, cuerpo) in respuestas {
                let (mut s, _) = listener.accept().unwrap();
                let mut buf = [0u8; 2048];
                let _ = s.read(&mut buf); // consumir la petición
                let razon = match codigo {
                    200 => "OK",
                    404 => "Not Found",
                    503 => "Service Unavailable",
                    _ => "Status",
                };
                let resp = format!(
                    "HTTP/1.1 {codigo} {razon}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{cuerpo}",
                    cuerpo.len()
                );
                let _ = s.write_all(resp.as_bytes());
                servidas += 1;
            }
            servidas
        });
        (url, handle)
    }

    fn cliente() -> Cliente {
        Cliente::nuevo(
            Duration::from_secs(5),
            Duration::ZERO,
            Politica::instantanea(5),
        )
        .unwrap()
    }

    #[test]
    fn trae_texto_y_json_de_un_200() {
        let (url, h) = servidor(vec![(200, r#"{"n":42}"#.into())]);
        #[derive(serde::Deserialize)]
        struct R {
            n: u32,
        }
        let r: R = cliente().obtener_json(&url).unwrap();
        assert_eq!(r.n, 42);
        assert_eq!(h.join().unwrap(), 1);
    }

    #[test]
    fn reintenta_un_5xx_y_luego_acierta() {
        // 503, 503, 200 → el cliente insiste y termina en Ok con 3 conexiones.
        let (url, h) = servidor(vec![
            (503, "caído".into()),
            (503, "caído".into()),
            (200, "ok".into()),
        ]);
        let cuerpo = cliente().obtener_texto(&url).unwrap();
        assert_eq!(cuerpo, "ok");
        assert_eq!(h.join().unwrap(), 3, "debió reconectar 3 veces");
    }

    #[test]
    fn no_reintenta_un_404() {
        // ROJO por la vía real: un 404 corta al instante — una sola conexión — y
        // devuelve ErrorHttp::Estado. Si reintentara, el servidor (que solo sirve 1)
        // dejaría al cliente colgado esperando y el join no daría 1.
        let (url, h) = servidor(vec![(404, "no está".into())]);
        let err = cliente().obtener_texto(&url).unwrap_err();
        match err {
            ErrorHttp::Estado { codigo: 404, .. } => {}
            otro => panic!("esperaba Estado 404, vino {otro}"),
        }
        assert_eq!(h.join().unwrap(), 1, "no debió reintentar");
    }
}
